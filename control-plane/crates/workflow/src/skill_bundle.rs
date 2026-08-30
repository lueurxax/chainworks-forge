use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

pub(crate) const MAX_SKILL_BYTES: u64 = 65_536;
const MAX_SKILL_BODY_LINES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmbeddedSkillBundle {
    pub source_encoding: String,
    pub skill_md: String,
    pub skill_bundle_sha256: String,
}

#[derive(Debug)]
pub(crate) struct LoadedSkillBundle {
    pub embedded: EmbeddedSkillBundle,
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct SkillFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

pub(crate) fn load_skill_bundle(
    catalog_base: &Path,
    relative_path: &str,
) -> Result<LoadedSkillBundle> {
    load_skill_bundle_with_hook(catalog_base, relative_path, || {})
}

fn load_skill_bundle_with_hook<F>(
    catalog_base: &Path,
    relative_path: &str,
    after_file_open: F,
) -> Result<LoadedSkillBundle>
where
    F: FnOnce(),
{
    validate_relative_path(relative_path)?;
    let expected_name = Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("external skill path must end in a UTF-8 directory name"))?;

    let dir = open_bundle_directory(catalog_base, relative_path)?;

    let before_entries = enumerate_directory(dir.as_raw_fd())?;
    if before_entries != [b"SKILL.md".to_vec()] {
        anyhow::bail!("external skill bundle must contain exactly one regular SKILL.md entry");
    }

    let file_fd = open_regular_file(dir.as_raw_fd(), b"SKILL.md")?;
    after_file_open();
    let mut file = File::from(file_fd);
    let before = file.metadata().context("reading SKILL.md metadata")?;
    require_regular_bounded_file(&before)?;

    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(MAX_SKILL_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("reading bounded SKILL.md bytes")?;
    if bytes.len() as u64 > MAX_SKILL_BYTES {
        anyhow::bail!("SKILL.md exceeds {MAX_SKILL_BYTES} bytes");
    }
    if bytes.len() as u64 != before.len() {
        anyhow::bail!("SKILL.md changed size while it was being read");
    }

    let after = file.metadata().context("re-reading SKILL.md metadata")?;
    if metadata_identity(&before) != metadata_identity(&after) {
        anyhow::bail!("SKILL.md changed while it was being read");
    }
    if enumerate_directory(dir.as_raw_fd())? != before_entries {
        anyhow::bail!("external skill bundle entries changed while SKILL.md was read");
    }
    let rebound_dir = open_bundle_directory(catalog_base, relative_path)
        .context("revalidating external skill bundle path")?;
    if descriptor_identity(dir.as_raw_fd())? != descriptor_identity(rebound_dir.as_raw_fd())? {
        anyhow::bail!("external skill bundle path changed while SKILL.md was read");
    }
    if enumerate_directory(rebound_dir.as_raw_fd())? != before_entries {
        anyhow::bail!("external skill bundle entries changed while SKILL.md was read");
    }
    let rebound_file = File::from(
        open_regular_file(rebound_dir.as_raw_fd(), b"SKILL.md")
            .context("revalidating SKILL.md path")?,
    );
    let rebound_metadata = rebound_file
        .metadata()
        .context("revalidating SKILL.md metadata")?;
    if metadata_identity(&after) != metadata_identity(&rebound_metadata) {
        anyhow::bail!("SKILL.md path changed while it was being read");
    }

    let skill_md = String::from_utf8(bytes).context("SKILL.md must be valid UTF-8")?;
    let body = parse_and_validate_skill_document(&skill_md, expected_name)?;
    let skill_bundle_sha256 = format!("{:x}", Sha256::digest(skill_md.as_bytes()));
    Ok(LoadedSkillBundle {
        embedded: EmbeddedSkillBundle {
            source_encoding: "utf-8".to_string(),
            skill_md,
            skill_bundle_sha256,
        },
        body,
    })
}

pub(crate) fn validate_embedded_skill_bundle(
    bundle: &EmbeddedSkillBundle,
    relative_path: &str,
) -> Result<String> {
    validate_relative_path(relative_path)?;
    if bundle.source_encoding != "utf-8" {
        anyhow::bail!("embedded skill source_encoding must be utf-8");
    }
    if bundle.skill_md.len() as u64 > MAX_SKILL_BYTES {
        anyhow::bail!("embedded SKILL.md exceeds {MAX_SKILL_BYTES} bytes");
    }
    let actual_hash = format!("{:x}", Sha256::digest(bundle.skill_md.as_bytes()));
    if bundle.skill_bundle_sha256 != actual_hash {
        anyhow::bail!("embedded skill bundle digest mismatch");
    }
    let expected_name = Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("external skill path must end in a UTF-8 directory name"))?;
    parse_and_validate_skill_document(&bundle.skill_md, expected_name)
}

fn parse_and_validate_skill_document(content: &str, expected_name: &str) -> Result<String> {
    let mut offset = 0usize;
    let mut lines = content.split_inclusive('\n');
    let first = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("SKILL.md is empty"))?;
    offset += first.len();
    if first.trim_end_matches(['\r', '\n']) != "---" {
        anyhow::bail!("SKILL.md must start with YAML frontmatter");
    }

    let mut frontmatter = String::new();
    let mut closed = false;
    for line in lines {
        offset += line.len();
        if line.trim_end_matches(['\r', '\n']) == "---" {
            closed = true;
            break;
        }
        frontmatter.push_str(line);
    }
    if !closed {
        anyhow::bail!("SKILL.md frontmatter is not closed");
    }

    let frontmatter: SkillFrontmatter =
        serde_yaml::from_str(&frontmatter).context("parsing SKILL.md frontmatter")?;
    validate_skill_name(&frontmatter.name)?;
    if frontmatter.name != expected_name {
        anyhow::bail!(
            "SKILL.md name '{}' must match bundle directory '{}'",
            frontmatter.name,
            expected_name
        );
    }
    let description = frontmatter.description.trim();
    if description.is_empty() || description.chars().count() > 1024 {
        anyhow::bail!("SKILL.md description must contain 1..=1024 characters");
    }
    if frontmatter
        .compatibility
        .as_ref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        anyhow::bail!("SKILL.md compatibility exceeds 500 characters");
    }
    let _ = frontmatter.metadata;

    let body = &content[offset..];
    if body.trim().is_empty() {
        anyhow::bail!("SKILL.md body must not be empty");
    }
    if body.lines().count() > MAX_SKILL_BODY_LINES {
        anyhow::bail!("SKILL.md body exceeds {MAX_SKILL_BODY_LINES} lines");
    }
    Ok(body.to_string())
}

fn validate_skill_name(name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        anyhow::bail!("SKILL.md name must contain 1..=64 ASCII characters");
    }
    if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' || name.contains("--") {
        anyhow::bail!("SKILL.md name must use single, non-edge hyphens");
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        anyhow::bail!("SKILL.md name must use lowercase ASCII letters, digits, and hyphens");
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        anyhow::bail!("external skill path must be a non-empty relative path");
    }
    if !path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("external skill path may not contain '.', '..', or root components");
    }
    Ok(())
}

fn open_directory(path: &Path) -> io::Result<OwnedFd> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    owned_fd(unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    })
}

fn open_bundle_directory(catalog_base: &Path, relative_path: &str) -> Result<OwnedFd> {
    let mut dir = open_directory_tree(catalog_base)?;
    for component in Path::new(relative_path).components() {
        let Component::Normal(name) = component else {
            anyhow::bail!("external skill path contains an invalid component");
        };
        dir = open_child_directory(dir.as_raw_fd(), name.as_bytes())
            .with_context(|| format!("opening external skill path component {:?}", name))?;
    }
    Ok(dir)
}

fn open_directory_tree(path: &Path) -> Result<OwnedFd> {
    let mut dir = if path.is_absolute() {
        open_directory(Path::new("/"))?
    } else {
        open_directory(Path::new("."))?
    };
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                dir =
                    open_child_directory(dir.as_raw_fd(), name.as_bytes()).with_context(|| {
                        format!("opening agent catalog parent component {:?}", name)
                    })?;
            }
            Component::ParentDir => {
                dir = open_child_directory(dir.as_raw_fd(), b"..").context(
                    "opening agent catalog parent component '..' without following symlinks",
                )?;
            }
            Component::Prefix(_) => {
                anyhow::bail!("agent catalog parent must not contain platform-prefix components");
            }
        }
    }
    Ok(dir)
}

fn open_child_directory(parent: RawFd, name: &[u8]) -> io::Result<OwnedFd> {
    let name =
        CString::new(name).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    owned_fd(unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK,
        )
    })
}

fn open_regular_file(parent: RawFd, name: &[u8]) -> io::Result<OwnedFd> {
    let name =
        CString::new(name).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    owned_fd(unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    })
}

fn owned_fd(fd: RawFd) -> io::Result<OwnedFd> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn enumerate_directory(fd: RawFd) -> io::Result<Vec<Vec<u8>>> {
    let current = CString::new(".").expect("literal contains no nul");
    let stream_fd = unsafe {
        libc::openat(
            fd,
            current.as_ptr(),
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK,
        )
    };
    if stream_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let directory = unsafe { libc::fdopendir(stream_fd) };
    if directory.is_null() {
        unsafe { libc::close(stream_fd) };
        return Err(io::Error::last_os_error());
    }

    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(directory) };
        if entry.is_null() {
            break;
        }
        let bytes = unsafe {
            let start = (*entry).d_name.as_ptr() as *const u8;
            let mut len = 0usize;
            while *start.add(len) != 0 {
                len += 1;
            }
            std::slice::from_raw_parts(start, len).to_vec()
        };
        if bytes != b"." && bytes != b".." {
            names.push(bytes);
        }
    }
    unsafe { libc::closedir(directory) };
    names.sort();
    Ok(names)
}

fn require_regular_bounded_file(metadata: &Metadata) -> Result<()> {
    if !metadata.is_file() {
        anyhow::bail!("SKILL.md must be a regular file");
    }
    if metadata.len() > MAX_SKILL_BYTES {
        anyhow::bail!("SKILL.md exceeds {MAX_SKILL_BYTES} bytes");
    }
    Ok(())
}

fn metadata_identity(metadata: &Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn descriptor_identity(fd: RawFd) -> io::Result<(u64, u64, u32)> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    Ok((stat.st_dev as u64, stat.st_ino as u64, stat.st_mode as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "chainworks_skill_bundle_unit_{}_{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }

        fn write_skill(&self, root: &str, body: &str) {
            let directory = self.0.join(root).join("test-skill");
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                directory.join("SKILL.md"),
                format!("---\nname: test-skill\ndescription: Stable test skill.\n---\n{body}\n"),
            )
            .unwrap();
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn final_entry_rename_swap_fails_closed() {
        let root = TestDir::new();
        root.write_skill("skills", "original bytes");
        let directory = root.0.join("skills/test-skill");

        let result = load_skill_bundle_with_hook(&root.0, "skills/test-skill", || {
            fs::rename(directory.join("SKILL.md"), directory.join("SKILL.old")).unwrap();
            fs::write(
                directory.join("SKILL.md"),
                "---\nname: test-skill\ndescription: Replacement skill.\n---\nreplacement bytes\n",
            )
            .unwrap();
            fs::remove_file(directory.join("SKILL.old")).unwrap();
        });

        assert!(result.is_err(), "renamed final entry must fail closed");
    }

    #[cfg(unix)]
    #[test]
    fn intermediate_directory_symlink_swap_fails_closed() {
        use std::os::unix::fs::symlink;

        let root = TestDir::new();
        root.write_skill("skills", "original bytes");
        root.write_skill("outside", "outside bytes");
        let skills = root.0.join("skills");
        let original = root.0.join("skills-original");
        let outside = root.0.join("outside");

        let result = load_skill_bundle_with_hook(&root.0, "skills/test-skill", || {
            fs::rename(&skills, &original).unwrap();
            symlink(&outside, &skills).unwrap();
        });

        assert!(
            result.is_err(),
            "swapped intermediate directory must fail closed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn catalog_parent_symlink_is_rejected_without_following() {
        use std::os::unix::fs::symlink;

        let root = TestDir::new();
        root.write_skill("real-catalog/skills", "original bytes");
        let alias = root.0.join("catalog-alias");
        symlink(root.0.join("real-catalog"), &alias).unwrap();

        let result = load_skill_bundle(&alias, "skills/test-skill");
        assert!(result.is_err(), "catalog parent symlink must fail closed");
    }
}
