use super::budget::Deadline;
use anyhow::{ensure, Context, Result};
use domain::run_carry_forward::ContentDigest;
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::Path,
};

pub(super) fn validate_relative(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && !path.starts_with('/')
            && !path.contains('\0')
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "unsafe_path: invalid relative path"
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EntryInfo {
    pub mode: u32,
    pub size: u64,
    pub dev: u64,
    pub ino: u64,
}

pub(super) struct SafeRoot {
    pub file: File,
}

impl SafeRoot {
    pub fn open(path: &Path) -> Result<Self> {
        ensure!(
            path.is_absolute(),
            "unsafe_path: directory must be absolute"
        );
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open("/")?;
        for component in path.components() {
            match component {
                std::path::Component::RootDir => continue,
                std::path::Component::Normal(name) => {
                    let name = CString::new(name.as_bytes())?;
                    let fd = unsafe {
                        libc::openat(
                            file.as_raw_fd(),
                            name.as_ptr(),
                            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                        )
                    };
                    if fd < 0 {
                        return Err(io::Error::last_os_error())
                            .context("unsafe_path: directory ancestor");
                    }
                    file = unsafe { File::from_raw_fd(fd) };
                }
                _ => anyhow::bail!("unsafe_path: directory component"),
            }
        }
        Ok(Self { file })
    }

    pub fn verify_path(&self, path: &Path) -> Result<()> {
        let current = Self::open(path)?.file.metadata()?;
        let held = self.file.metadata()?;
        ensure!(
            (current.dev(), current.ino()) == (held.dev(), held.ino()),
            "unsafe_path: directory identity changed"
        );
        Ok(())
    }

    pub fn create_directory(&self, relative: &str) -> Result<Self> {
        let (parent, leaf) = self.parent(relative)?;
        if unsafe { libc::mkdirat(parent.as_raw_fd(), leaf.as_ptr(), 0o700) } < 0 {
            return Err(io::Error::last_os_error()).context("destination_conflict: directory");
        }
        parent.sync_all()?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error()).context("unsafe_path: created directory");
        }
        Ok(Self {
            file: unsafe { File::from_raw_fd(fd) },
        })
    }

    pub fn parent(&self, relative: &str) -> Result<(File, CString)> {
        validate_relative(relative)?;
        let mut parts: Vec<_> = relative.split('/').collect();
        let leaf = CString::new(parts.pop().context("unsafe_path")?)?;
        let mut parent = self.file.try_clone()?;
        for part in parts {
            let name = CString::new(part)?;
            // Each component is opened relative to the held parent, never followed through a symlink.
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error()).context("unsafe_path: parent");
            }
            parent = unsafe { File::from_raw_fd(fd) };
        }
        Ok((parent, leaf))
    }

    pub fn inspect(&self, relative: &str) -> Result<Option<EntryInfo>> {
        let (parent, leaf) = match self.parent(relative) {
            Ok(value) => value,
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                return Ok(None)
            }
            Err(error) => return Err(error),
        };
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        let result = unsafe {
            libc::fstatat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(error.into());
        }
        let stat = unsafe { stat.assume_init() };
        ensure!(stat.st_size >= 0, "unsafe_path: negative size");
        Ok(Some(EntryInfo {
            mode: stat.st_mode as u32,
            size: stat.st_size as u64,
            dev: stat.st_dev as u64,
            ino: stat.st_ino,
        }))
    }

    pub fn read_link(&self, relative: &str) -> Result<String> {
        let (parent, leaf) = self.parent(relative)?;
        let mut bytes = vec![0u8; 4096];
        let n = unsafe {
            libc::readlinkat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        };
        if n < 0 {
            return Err(io::Error::last_os_error().into());
        }
        ensure!(
            (n as usize) < bytes.len(),
            "continuation_budget_exceeded: symlink target"
        );
        bytes.truncate(n as usize);
        Ok(String::from_utf8(bytes).context("unsafe_path: non-UTF8 link")?)
    }

    pub fn open_file(&self, relative: &str) -> Result<File> {
        let (parent, leaf) = self.parent(relative)?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error()).context("unsafe_path: file");
        }
        let file = unsafe { File::from_raw_fd(fd) };
        ensure!(
            file.metadata()?.is_file(),
            "unsafe_path: not a regular file"
        );
        Ok(file)
    }

    fn create_parent(&self, relative: &str) -> Result<(File, CString)> {
        validate_relative(relative)?;
        let mut parts: Vec<_> = relative.split('/').collect();
        let leaf = CString::new(parts.pop().context("unsafe_path")?)?;
        let mut parent = self.file.try_clone()?;
        for part in parts {
            let name = CString::new(part)?;
            let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
            if result < 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                return Err(io::Error::last_os_error().into());
            }
            parent.sync_all()?;
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error()).context("unsafe_path: destination parent");
            }
            parent = unsafe { File::from_raw_fd(fd) };
        }
        Ok((parent, leaf))
    }

    pub fn create_file(&self, relative: &str) -> Result<File> {
        let (parent, leaf) = self.create_parent(relative)?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error()).context("destination_conflict: file");
        }
        let file = unsafe { File::from_raw_fd(fd) };
        parent.sync_all()?;
        Ok(file)
    }

    pub fn create_link(&self, relative: &str, target: &str) -> Result<()> {
        let (parent, leaf) = self.create_parent(relative)?;
        let target = CString::new(target)?;
        let result = unsafe { libc::symlinkat(target.as_ptr(), parent.as_raw_fd(), leaf.as_ptr()) };
        if result < 0 {
            return Err(io::Error::last_os_error()).context("destination_conflict: link");
        }
        parent.sync_all()?;
        Ok(())
    }

    pub fn publish(&self, name: &str, bytes: &[u8], deadline: Deadline) -> Result<()> {
        validate_relative(name)?;
        ensure!(!name.contains('/'), "unsafe_path: publication name");
        let pending = self.prepare_publication(bytes, deadline)?;
        self.publish_prepared(pending, name, deadline)
    }

    fn prepare_publication(&self, bytes: &[u8], deadline: Deadline) -> Result<CString> {
        use std::io::Write;
        deadline.check()?;
        let pending = format!(".pending-{}", uuid::Uuid::new_v4());
        let mut file = self.create_file(&pending)?;
        deadline.check()?;
        file.write_all(bytes)?;
        deadline.check()?;
        file.sync_all()?;
        Ok(CString::new(pending)?)
    }

    fn publish_prepared(&self, from: CString, name: &str, deadline: Deadline) -> Result<()> {
        let to = CString::new(name)?;
        deadline.check()?;
        // linkat provides atomic publication without replacing an existing receipt.
        let result = unsafe {
            libc::linkat(
                self.file.as_raw_fd(),
                from.as_ptr(),
                self.file.as_raw_fd(),
                to.as_ptr(),
                0,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error()).context("destination_conflict: publication");
        }
        self.file.sync_all()?;
        deadline.check()?;
        let result = unsafe { libc::unlinkat(self.file.as_raw_fd(), from.as_ptr(), 0) };
        if result < 0 {
            return Err(io::Error::last_os_error().into());
        }
        self.file.sync_all()?;
        deadline.check()?;
        Ok(())
    }
}

pub(super) fn identity(meta: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

pub(super) fn hash_file(mut file: File, limit: u64, deadline: Deadline) -> Result<ContentDigest> {
    deadline.check()?;
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.len() <= limit,
        "continuation_budget_exceeded: file"
    );
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        deadline.check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(total <= limit, "continuation_budget_exceeded: growing file");
        hash.update(&buffer[..n]);
    }
    ensure!(
        identity(&before) == identity(&file.metadata()?),
        "source_changed: file during read"
    );
    deadline.check()?;
    ContentDigest::from_sha256_hex(&format!("{:x}", hash.finalize())).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn held_directory_does_not_follow_a_replaced_ancestor() {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let parent = base.join("parent");
        let moved = base.join("moved");
        let replacement = base.join("replacement");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&replacement).unwrap();
        let held = SafeRoot::open(&parent).unwrap();
        fs::rename(&parent, &moved).unwrap();
        symlink(&replacement, &parent).unwrap();
        held.create_directory("owned").unwrap();
        assert!(moved.join("owned").is_dir());
        assert!(!replacement.join("owned").exists());
        assert!(held.verify_path(&parent).is_err());
        fs::create_dir(replacement.join("child")).unwrap();
        assert!(SafeRoot::open(&parent.join("child")).is_err());
    }

    #[test]
    fn expired_hash_stops_before_reading_contents() {
        let mut file = tempfile::tempfile().unwrap();
        use std::io::{Seek, Write};
        file.write_all(b"fixture").unwrap();
        file.rewind().unwrap();
        let error = hash_file(file, 1024, Deadline::after(std::time::Duration::ZERO)).unwrap_err();
        assert_eq!(error.to_string(), "continuation_budget_exceeded: deadline");
    }

    #[test]
    fn expiry_after_receipt_fsync_prevents_publication() {
        let temp = tempfile::tempdir().unwrap();
        let root = SafeRoot::open(&fs::canonicalize(temp.path()).unwrap()).unwrap();
        let pending = root
            .prepare_publication(
                b"verified",
                Deadline::after(std::time::Duration::from_secs(30)),
            )
            .unwrap();
        let error = root
            .publish_prepared(
                pending,
                "receipt.json",
                Deadline::after(std::time::Duration::ZERO),
            )
            .unwrap_err();
        assert_eq!(error.to_string(), "continuation_budget_exceeded: deadline");
        assert!(!temp.path().join("receipt.json").exists());
    }
}
