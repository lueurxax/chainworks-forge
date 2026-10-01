//! Read-only APFS persistent identity. Mount device numbers are observations, not
//! durable volume identities. No NSURL resource identifiers or allocating legacy
//! OBJPERMANENTID calls are used. Unknown platforms/capabilities fail closed.
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityKind {
    PrivateFile,
    Directory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    Unsupported,
    Unavailable,
    Insecure,
    Changed,
    Invalid,
}
impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "persistent_identity_unsupported",
            Self::Unavailable => "persistent_identity_unavailable",
            Self::Insecure => "persistent_identity_insecure",
            Self::Changed => "persistent_identity_changed",
            Self::Invalid => "persistent_identity_invalid",
        })
    }
}
impl std::error::Error for IdentityError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistentObjectIdentity {
    pub version: u32,
    pub filesystem: String,
    pub volume_uuid: String,
    pub object_id: String,
}
impl PersistentObjectIdentity {
    pub fn validate(&self) -> Result<(), IdentityError> {
        let id = self
            .object_id
            .parse::<u64>()
            .map_err(|_| IdentityError::Invalid)?;
        if self.version != 1
            || self.filesystem != "apfs"
            || id == 0
            || self.object_id != id.to_string()
            || !valid_uuid(&self.volume_uuid)
        {
            return Err(IdentityError::Invalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemObservation {
    pub canonical_path: String,
    pub uid: u32,
    pub device: u64,
    pub inode: u64,
    pub birth_sec: i64,
    pub birth_nsec: i64,
    pub boot_session_uuid: String,
    pub persistent: PersistentObjectIdentity,
}
impl FilesystemObservation {
    pub fn validate(&self) -> Result<(), IdentityError> {
        self.persistent.validate()?;
        let path = &self.canonical_path;
        if path.len() > 4096
            || !path.starts_with('/')
            || path.ends_with('/')
            || path.chars().any(char::is_control)
            || path[1..]
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
            || self.inode == 0
            || self.device == 0
            || self.birth_sec < 0
            || !(0..1_000_000_000).contains(&self.birth_nsec)
            || self.persistent.object_id != self.inode.to_string()
            || !valid_uuid(&self.boot_session_uuid)
        {
            return Err(IdentityError::Invalid);
        }
        Ok(())
    }
}
fn valid_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .is_ok_and(|id| !id.is_nil() && !id.is_max() && id.to_string() == value)
}

/// Cross-boot acceptance is possible only with an already pinned persistent
/// identity. This function does not upgrade a legacy (device,inode) record.
pub fn accepts_observation(
    pinned: &FilesystemObservation,
    current: &FilesystemObservation,
) -> Result<(), IdentityError> {
    pinned.validate()?;
    current.validate()?;
    if pinned.canonical_path != current.canonical_path
        || pinned.uid != current.uid
        || pinned.inode != current.inode
        || pinned.birth_sec != current.birth_sec
        || pinned.birth_nsec != current.birth_nsec
        || pinned.persistent != current.persistent
        || (pinned.device != current.device
            && pinned.boot_session_uuid == current.boot_session_uuid)
    {
        return Err(IdentityError::Changed);
    }
    Ok(())
}

pub fn observe(path: &Path, kind: IdentityKind) -> Result<FilesystemObservation, IdentityError> {
    let canonical = fs::canonicalize(path).map_err(|_| IdentityError::Unavailable)?;
    // Refuse both final symlinks and aliasing ancestors at this boundary. Callers
    // must resolve their exact canonical target before requesting enrollment.
    if !path.is_absolute() || canonical != path {
        return Err(IdentityError::Insecure);
    }
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(
        libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | libc::O_CLOEXEC
            | if kind == IdentityKind::Directory {
                libc::O_DIRECTORY
            } else {
                0
            },
    );
    let file = options.open(path).map_err(|_| IdentityError::Unavailable)?;
    let before = checked_metadata(path, &file, kind)?;
    let boot = native::boot_uuid()?;
    let persistent = native::identity(&file)?;
    let after = checked_metadata(path, &file, kind)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || birth(&before)? != birth(&after)?
        || boot != native::boot_uuid()?
        || persistent != native::identity(&file)?
    {
        return Err(IdentityError::Changed);
    }
    let (birth_sec, birth_nsec) = birth(&after)?;
    let observation = FilesystemObservation {
        canonical_path: canonical.to_str().ok_or(IdentityError::Invalid)?.to_owned(),
        uid: after.uid(),
        device: after.dev(),
        inode: after.ino(),
        birth_sec,
        birth_nsec,
        boot_session_uuid: boot,
        persistent,
    };
    observation.validate()?;
    Ok(observation)
}
fn birth(metadata: &fs::Metadata) -> Result<(i64, i64), IdentityError> {
    let time = metadata
        .created()
        .map_err(|_| IdentityError::Unsupported)?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| IdentityError::Invalid)?;
    Ok((
        i64::try_from(time.as_secs()).map_err(|_| IdentityError::Invalid)?,
        i64::from(time.subsec_nanos()),
    ))
}
fn checked_metadata(
    path: &Path,
    file: &File,
    kind: IdentityKind,
) -> Result<fs::Metadata, IdentityError> {
    let named = fs::symlink_metadata(path).map_err(|_| IdentityError::Unavailable)?;
    let opened = file.metadata().map_err(|_| IdentityError::Unavailable)?;
    for metadata in [&named, &opened] {
        let safe = match kind {
            IdentityKind::PrivateFile => {
                metadata.is_file() && metadata.nlink() == 1 && metadata.mode() & 0o7777 == 0o600
            }
            IdentityKind::Directory => metadata.is_dir() && metadata.mode() & 0o7022 == 0,
        };
        if !safe || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(IdentityError::Insecure);
        }
    }
    if named.dev() != opened.dev()
        || named.ino() != opened.ino()
        || fs::canonicalize(path).map_err(|_| IdentityError::Unavailable)? != path
    {
        return Err(IdentityError::Changed);
    }
    Ok(opened)
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;
    pub(super) fn boot_uuid() -> Result<String, IdentityError> {
        Err(IdentityError::Unsupported)
    }
    pub(super) fn identity(_: &File) -> Result<PersistentObjectIdentity, IdentityError> {
        Err(IdentityError::Unsupported)
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::ffi::CStr;
    use std::os::fd::AsRawFd;
    pub(super) fn boot_uuid() -> Result<String, IdentityError> {
        let mut bytes = [0u8; 128];
        let mut length = bytes.len();
        let result = unsafe {
            libc::sysctlbyname(
                c"kern.bootsessionuuid".as_ptr(),
                bytes.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        if result != 0 || length < 2 || length > bytes.len() {
            return Err(IdentityError::Unavailable);
        }
        let value =
            CStr::from_bytes_with_nul(&bytes[..length]).map_err(|_| IdentityError::Invalid)?;
        let id = uuid::Uuid::parse_str(value.to_str().map_err(|_| IdentityError::Invalid)?)
            .map_err(|_| IdentityError::Invalid)?;
        if id.is_nil() || id.is_max() {
            return Err(IdentityError::Invalid);
        }
        Ok(id.to_string())
    }
    fn attributes(
        file: &File,
        common: u32,
        volume: u32,
        expected: usize,
    ) -> Result<Vec<u8>, IdentityError> {
        let mut request = libc::attrlist {
            bitmapcount: 5,
            reserved: 0,
            commonattr: common | libc::ATTR_CMN_RETURNED_ATTRS,
            volattr: volume,
            dirattr: 0,
            fileattr: 0,
            forkattr: 0,
        };
        let mut result = vec![0u8; expected];
        let code = unsafe {
            libc::fgetattrlist(
                file.as_raw_fd(),
                (&mut request as *mut libc::attrlist).cast(),
                result.as_mut_ptr().cast(),
                result.len(),
                0,
            )
        };
        if code != 0 {
            return Err(IdentityError::Unsupported);
        }
        validate_attribute_response(&result, request.commonattr, volume, expected)?;
        Ok(result)
    }
    // Separate the existing fixed-layout checks so malformed native responses
    // and missing capability bits can be tested without changing host mounts.
    fn validate_attribute_response(
        result: &[u8],
        commonattr: u32,
        volume: u32,
        expected: usize,
    ) -> Result<(), IdentityError> {
        if word(result, 0)? as usize != expected
            || word(result, 4)? != commonattr
            || word(result, 8)? != (volume & !libc::ATTR_VOL_INFO)
            || [12, 16, 20].iter().any(|&i| word(result, i) != Ok(0))
        {
            return Err(IdentityError::Unsupported);
        }
        Ok(())
    }
    fn validate_capabilities(capabilities: u32, valid: u32) -> Result<(), IdentityError> {
        let required = libc::VOL_CAP_FMT_PERSISTENTOBJECTIDS | libc::VOL_CAP_FMT_64BIT_OBJECT_IDS;
        if capabilities & required != required || valid & required != required {
            return Err(IdentityError::Unsupported);
        }
        Ok(())
    }
    fn word(bytes: &[u8], offset: usize) -> Result<u32, IdentityError> {
        Ok(u32::from_ne_bytes(
            bytes
                .get(offset..offset + 4)
                .ok_or(IdentityError::Invalid)?
                .try_into()
                .map_err(|_| IdentityError::Invalid)?,
        ))
    }
    pub(super) fn identity(file: &File) -> Result<PersistentObjectIdentity, IdentityError> {
        let mut filesystem: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatfs(file.as_raw_fd(), &mut filesystem) } != 0 {
            return Err(IdentityError::Unavailable);
        }
        let name = unsafe { CStr::from_ptr(filesystem.f_fstypename.as_ptr()) };
        if name.to_bytes() != b"apfs" || filesystem.f_flags & libc::MNT_LOCAL as u32 == 0 {
            return Err(IdentityError::Unsupported);
        }
        // Ordered fixed-size output: length, returned mask, capabilities[4],
        // valid[4], UUID. Capabilities explicitly promise IDs survive remount;
        // 64-bit volumes require FILEID rather than legacy 32-bit OBJPERMANENTID.
        let volume = attributes(
            file,
            0,
            libc::ATTR_VOL_INFO | libc::ATTR_VOL_CAPABILITIES | libc::ATTR_VOL_UUID,
            72,
        )?;
        validate_capabilities(word(&volume, 24)?, word(&volume, 40)?)?;
        let volume_uuid = uuid::Uuid::from_bytes(
            volume[56..72]
                .try_into()
                .map_err(|_| IdentityError::Invalid)?,
        );
        if volume_uuid.is_nil() || volume_uuid.is_max() {
            return Err(IdentityError::Invalid);
        }
        let object = attributes(file, libc::ATTR_CMN_FILEID, 0, 32)?;
        let object_id = u64::from_ne_bytes(
            object[24..32]
                .try_into()
                .map_err(|_| IdentityError::Invalid)?,
        );
        let identity = PersistentObjectIdentity {
            version: 1,
            filesystem: "apfs".into(),
            volume_uuid: volume_uuid.to_string(),
            object_id: object_id.to_string(),
        };
        identity.validate()?;
        Ok(identity)
    }
    #[cfg(test)]
    mod native_decoding_tests {
        use super::*;

        #[test]
        fn either_missing_capability_or_missing_valid_bit_denies_persistence() {
            let required =
                libc::VOL_CAP_FMT_PERSISTENTOBJECTIDS | libc::VOL_CAP_FMT_64BIT_OBJECT_IDS;
            validate_capabilities(required, required).unwrap();
            for missing in [
                libc::VOL_CAP_FMT_PERSISTENTOBJECTIDS,
                libc::VOL_CAP_FMT_64BIT_OBJECT_IDS,
            ] {
                assert_eq!(
                    validate_capabilities(required & !missing, required),
                    Err(IdentityError::Unsupported)
                );
                assert_eq!(
                    validate_capabilities(required, required & !missing),
                    Err(IdentityError::Unsupported)
                );
            }
            assert_eq!(validate_capabilities(0, 0), Err(IdentityError::Unsupported));
        }

        #[test]
        fn omitted_unexpected_or_truncated_native_attributes_fail_closed() {
            let common = libc::ATTR_CMN_RETURNED_ATTRS;
            let returned_volume = libc::ATTR_VOL_CAPABILITIES | libc::ATTR_VOL_UUID;
            let request_volume = libc::ATTR_VOL_INFO | returned_volume;
            let mut valid = [0u8; 72];
            valid[0..4].copy_from_slice(&72u32.to_ne_bytes());
            valid[4..8].copy_from_slice(&common.to_ne_bytes());
            valid[8..12].copy_from_slice(&returned_volume.to_ne_bytes());
            validate_attribute_response(&valid, common, request_volume, 72).unwrap();
            for (offset, value) in [
                (0, 76u32),
                (4, 0),
                (8, libc::ATTR_VOL_CAPABILITIES),
                (12, 1),
                (16, 1),
                (20, 1),
            ] {
                let mut malformed = valid;
                malformed[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
                assert_eq!(
                    validate_attribute_response(&malformed, common, request_volume, 72),
                    Err(IdentityError::Unsupported)
                );
            }
            assert_eq!(
                validate_attribute_response(&[], common, request_volume, 72),
                Err(IdentityError::Invalid)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation() -> FilesystemObservation {
        FilesystemObservation {
            canonical_path: "/fixture/database.sqlite".into(),
            uid: 501,
            device: 7,
            inode: 42,
            birth_sec: 100,
            birth_nsec: 0,
            boot_session_uuid: "00000000-0000-4000-8000-000000000001".into(),
            persistent: PersistentObjectIdentity {
                version: 1,
                filesystem: "apfs".into(),
                volume_uuid: "00000000-0000-4000-8000-000000000002".into(),
                object_id: "42".into(),
            },
        }
    }
    #[test]
    fn remount_requires_both_new_boot_and_same_persistent_identity() {
        let pinned = observation();
        let mut current = pinned.clone();
        current.device += 1;
        assert_eq!(
            accepts_observation(&pinned, &current),
            Err(IdentityError::Changed)
        );
        current.boot_session_uuid = "00000000-0000-4000-8000-000000000003".into();
        accepts_observation(&pinned, &current).unwrap();
        current.persistent.volume_uuid = "00000000-0000-4000-8000-000000000004".into();
        assert_eq!(
            accepts_observation(&pinned, &current),
            Err(IdentityError::Changed)
        );
    }
    #[test]
    fn changed_object_path_owner_or_unsupported_identity_never_matches() {
        let pinned = observation();
        for mutation in 0..7 {
            let mut current = pinned.clone();
            match mutation {
                0 => {
                    current.inode += 1;
                    current.persistent.object_id = current.inode.to_string();
                }
                1 => current.canonical_path = "/fixture/replacement.sqlite".into(),
                2 => current.uid += 1,
                3 => current.persistent.filesystem = "unknown".into(),
                4 => current.persistent.volume_uuid.clear(),
                5 => current.boot_session_uuid.clear(),
                _ => current.persistent.object_id = "042".into(),
            };
            assert!(accepts_observation(&pinned, &current).is_err());
        }
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn native_apfs_probe_is_read_only_and_rejects_symlinks_hardlinks_and_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("database.sqlite");
        fs::write(&path, b"fixture").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let before = fs::read(&path).unwrap();
        let first = observe(&path, IdentityKind::PrivateFile).unwrap();
        assert_eq!(observe(&path, IdentityKind::PrivateFile).unwrap(), first);
        assert_eq!(fs::read(&path).unwrap(), before);
        let alias = root.join("alias");
        symlink(&path, &alias).unwrap();
        assert!(observe(&alias, IdentityKind::PrivateFile).is_err());
        fs::remove_file(&alias).unwrap();
        fs::hard_link(&path, &alias).unwrap();
        assert!(observe(&path, IdentityKind::PrivateFile).is_err());
        fs::remove_file(&alias).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            observe(&path, IdentityKind::PrivateFile),
            Err(IdentityError::Insecure)
        );
        observe(&root, IdentityKind::Directory).unwrap();
    }
}
