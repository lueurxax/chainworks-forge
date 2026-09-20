use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TEMPLATE: &str = include_str!("../../tests/fixtures/xcode_headless_i1/project.pbxproj");
const SCHEME: &str = include_str!("../../tests/fixtures/xcode_headless_i1/Fixture.xcscheme");
const MAIN: &str = include_str!("../../tests/fixtures/xcode_headless_i1/main.c");
pub const SEED_PATHS: [&str; 4] = [
    "Fixture.xcodeproj/project.pbxproj",
    "Fixture.xcodeproj/xcshareddata/xcschemes/Fixture.xcscheme",
    "Sources/main.c",
    "Sources/Sentinel.txt",
];

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    write_new_with_sync(path, bytes, |_| Ok(()))
}
pub fn write_new_with_sync(
    path: &Path,
    bytes: &[u8],
    mut before_sync: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    before_sync("file")?;
    file.sync_all()?;
    before_sync("dir")?;
    File::open(
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("i1_missing_parent"))?,
    )?
    .sync_all()?;
    Ok(())
}

pub fn no_links_below(root: &Path, path: &Path) -> Result<()> {
    let relative = path.strip_prefix(root)?;
    let mut current = root.to_owned();
    for component in relative.components() {
        ensure!(
            matches!(component, std::path::Component::Normal(_)),
            "i1_invalid_component"
        );
        current.push(component);
        ensure!(
            !fs::symlink_metadata(&current)?.file_type().is_symlink(),
            "i1_symlink"
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FixtureProject {
    pub label: String,
    pub root: PathBuf,
    pub package: PathBuf,
    pub read_path: String,
    pub marker: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FixturePair {
    pub projects: [FixtureProject; 2],
}

pub fn resolve_execution_root(
    repository: &Path,
    worktree: Option<&Path>,
    write_enabled: bool,
    strategy: Option<&str>,
) -> Result<PathBuf> {
    ensure!(
        matches!(
            strategy,
            None | Some("") | Some("dedicated") | Some("shared_implementation_worktree")
        ),
        "i1_unknown_strategy"
    );
    let required = matches!(
        strategy,
        Some("dedicated") | Some("shared_implementation_worktree")
    );
    ensure!(!required || worktree.is_some(), "i1_missing_worktree");
    let candidate = if required || write_enabled {
        worktree.unwrap_or(repository)
    } else {
        repository
    };
    let root = candidate.canonicalize()?;
    ensure!(root.is_dir(), "i1_missing_root");
    Ok(root)
}

pub fn select_project(root: &Path, selector: Option<&Path>) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let is_package = |p: &Path| {
        matches!(
            p.extension().and_then(|v| v.to_str()),
            Some("xcodeproj" | "xcworkspace")
        )
    };
    let candidate = if let Some(selector) = selector {
        root.join(selector)
    } else {
        let candidates = fs::read_dir(&root)?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        let candidates: Vec<_> = candidates.into_iter().filter(|p| is_package(p)).collect();
        ensure!(candidates.len() == 1, "i1_project_count");
        candidates[0].clone()
    };
    no_links_below(&root, &candidate)?;
    let package = candidate.canonicalize()?;
    ensure!(
        package != root && package.starts_with(&root) && package.is_dir() && is_package(&package),
        "i1_invalid_project"
    );
    Ok(package)
}

pub fn create_pair(fresh_root: &Path) -> Result<FixturePair> {
    let parent = fresh_root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("i1_missing_parent"))?
        .canonicalize()?;
    let root = parent.join(
        fresh_root
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("i1_missing_name"))?,
    );
    private_dir(&root)?;
    let mut projects = Vec::new();
    for label in ["A", "B"] {
        let project_root = root.join(label);
        private_dir(&project_root)?;
        let project_root = resolve_execution_root(&project_root, None, false, None)?;
        let package = project_root.join("Fixture.xcodeproj");
        private_dir(&package)?;
        private_dir(&package.join("xcshareddata"))?;
        private_dir(&package.join("xcshareddata/xcschemes"))?;
        private_dir(&project_root.join("Sources"))?;
        let marker = format!("CW_I1_{label}");
        write_new(&package.join("project.pbxproj"), TEMPLATE.as_bytes())?;
        write_new(&project_root.join(SEED_PATHS[1]), SCHEME.as_bytes())?;
        write_new(&project_root.join(SEED_PATHS[2]), MAIN.as_bytes())?;
        write_new(
            &project_root.join("Sources/Sentinel.txt"),
            format!("{marker}\n").as_bytes(),
        )?;
        projects.push(FixtureProject {
            label: label.into(),
            root: project_root,
            package,
            read_path: "Fixture/Sources/Sentinel.txt".into(),
            marker,
        });
    }
    Ok(FixturePair {
        projects: projects.try_into().unwrap(),
    })
}

pub fn seed_digests(pair: &FixturePair) -> Result<BTreeMap<String, String>> {
    let mut digests = BTreeMap::new();
    for project in &pair.projects {
        ensure!(
            project.root.canonicalize()? == project.root,
            "i1_noncanonical_root"
        );
        ensure!(
            select_project(&project.root, Some(&project.package))? == project.package,
            "i1_changed_project"
        );
        for path in SEED_PATHS {
            let file = project.root.join(path);
            no_links_below(&project.root, &file)?;
            ensure!(fs::metadata(&file)?.len() <= 1024 * 1024, "i1_seed_size");
            digests.insert(
                format!("{}/{path}", project.label),
                digest(&fs::read(file)?),
            );
        }
    }
    ensure!(digests.len() == 2 * SEED_PATHS.len(), "i1_seed_count");
    Ok(digests)
}

pub fn validate_fixed_pair(root: &Path, pair: &FixturePair) -> Result<()> {
    for (p, label) in pair.projects.iter().zip(["A", "B"]) {
        let expected_root = root.join(label);
        ensure!(
            p.label == label
                && p.root == expected_root
                && p.package == expected_root.join("Fixture.xcodeproj")
                && p.read_path == "Fixture/Sources/Sentinel.txt"
                && p.marker == format!("CW_I1_{label}"),
            "i1_fixture_layout"
        );
        ensure!(
            fs::read(p.package.join("project.pbxproj"))? == TEMPLATE.as_bytes()
                && fs::read(p.root.join(SEED_PATHS[1]))? == SCHEME.as_bytes()
                && fs::read(p.root.join(SEED_PATHS[2]))? == MAIN.as_bytes()
                && fs::read(p.root.join("Sources/Sentinel.txt"))?
                    == format!("CW_I1_{label}\n").as_bytes(),
            "i1_fixture_content"
        );
    }
    Ok(())
}

pub fn added_metadata(pair: &FixturePair) -> Result<BTreeMap<String, String>> {
    let mut output = BTreeMap::new();
    let mut count = 0;
    let mut bytes = 0;
    for p in &pair.projects {
        let mut pending = vec![p.root.clone()];
        while let Some(path) = pending.pop() {
            count += 1;
            ensure!(count <= 512, "i1_metadata_count");
            let m = fs::symlink_metadata(&path)?;
            ensure!(!m.file_type().is_symlink(), "i1_metadata_symlink");
            if m.is_dir() {
                for entry in fs::read_dir(&path)? {
                    pending.push(entry?.path());
                }
            } else {
                ensure!(m.is_file(), "i1_metadata_kind");
                let relative = path.strip_prefix(&p.root)?;
                if relative
                    .to_str()
                    .is_some_and(|path| SEED_PATHS.contains(&path))
                {
                    continue;
                }
                bytes += m.len();
                ensure!(bytes <= 8 * 1_048_576, "i1_metadata_size");
                output.insert(
                    format!(
                        "{}/{}",
                        p.label,
                        digest(relative.as_os_str().as_encoded_bytes())
                    ),
                    digest(&fs::read(&path)?),
                );
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn i1_fixture_has_a_macos_target_and_protects_all_seed_files() {
        use std::ffi::OsStr;
        let temp = tempfile::tempdir().unwrap();
        let fresh = temp.path().join("pair");
        let pair = create_pair(&fresh).unwrap();
        let project = &pair.projects[0];
        let (ok, out, _) = super::super::host::metadata(
            Path::new("/usr/bin/plutil"),
            &[
                OsStr::new("-convert"),
                OsStr::new("json"),
                OsStr::new("-o"),
                OsStr::new("-"),
                project.package.join("project.pbxproj").as_os_str(),
            ],
        )
        .await
        .unwrap();
        assert!(ok);
        let pbx: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let objects = &pbx["objects"];
        let root = &objects[pbx["rootObject"].as_str().unwrap()];
        let targets = root["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 1);
        let target = &objects[targets[0].as_str().unwrap()];
        assert_eq!(target["isa"], "PBXNativeTarget");
        assert_eq!(target["productType"], "com.apple.product-type.tool");
        let config_list = &objects[target["buildConfigurationList"].as_str().unwrap()];
        let config = &objects[config_list["buildConfigurations"][0].as_str().unwrap()];
        assert_eq!(config["buildSettings"]["SDKROOT"], "macosx");
        assert_eq!(config["buildSettings"]["SUPPORTED_PLATFORMS"], "macosx");
        assert!(target["buildPhases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|id| objects[id.as_str().unwrap()]["isa"] != "PBXShellScriptBuildPhase"));
        let before = seed_digests(&pair).unwrap();
        assert_eq!(before.len(), 8);
        assert!(added_metadata(&pair).unwrap().is_empty());
        let scheme = project
            .package
            .join("xcshareddata/xcschemes/Fixture.xcscheme");
        let original = fs::read(&scheme).unwrap();
        fs::write(&scheme, b"changed").unwrap();
        assert!(validate_fixed_pair(fresh.canonicalize().unwrap().as_path(), &pair).is_err());
        assert_ne!(seed_digests(&pair).unwrap(), before);
        fs::write(scheme, original).unwrap();
        fs::write(project.root.join("Sources/main.c"), b"changed").unwrap();
        assert!(validate_fixed_pair(fresh.canonicalize().unwrap().as_path(), &pair).is_err());
        assert_ne!(seed_digests(&pair).unwrap(), before);
    }

    #[test]
    fn i1_shared_read_only_uses_worktree() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let worktree = temp.path().join("worktree");
        fs::create_dir(&repo).unwrap();
        fs::create_dir(&worktree).unwrap();
        for strategy in ["dedicated", "shared_implementation_worktree"] {
            assert_eq!(
                resolve_execution_root(&repo, Some(&worktree), false, Some(strategy)).unwrap(),
                worktree.canonicalize().unwrap()
            );
            assert!(resolve_execution_root(&repo, None, false, Some(strategy)).is_err());
        }
    }

    #[test]
    fn i1_legacy_root_and_missing_roots() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        assert_eq!(
            resolve_execution_root(repo, None, true, None).unwrap(),
            repo.canonicalize().unwrap()
        );
        assert_eq!(
            resolve_execution_root(repo, Some(Path::new("/missing")), false, None).unwrap(),
            repo.canonicalize().unwrap()
        );
        assert!(resolve_execution_root(repo, Some(Path::new("/missing")), true, None).is_err());
        assert!(resolve_execution_root(repo, None, false, Some("future")).is_err());
        assert!(resolve_execution_root(&repo.join("missing"), None, false, None).is_err());
    }

    #[test]
    fn i1_exact_immediate_project_and_ambiguity() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        assert!(select_project(&root, None).is_err());
        let a = root.join("A.xcodeproj");
        fs::create_dir(&a).unwrap();
        assert_eq!(select_project(&root, None).unwrap(), a);
        fs::create_dir(root.join("B.xcworkspace")).unwrap();
        assert!(select_project(&root, None).is_err());
        assert_eq!(
            select_project(&root, Some(Path::new("A.xcodeproj"))).unwrap(),
            a
        );
        assert!(select_project(&root, Some(Path::new("../outside.xcodeproj"))).is_err());
        assert!(select_project(&root, Some(&root)).is_err());
    }

    #[test]
    fn i1_fixture_pair_has_distinct_markers_and_cannot_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let fresh = temp.path().join("pair");
        let pair = create_pair(&fresh).unwrap();
        assert_eq!(
            pair.projects[0].package.file_name().unwrap(),
            "Fixture.xcodeproj"
        );
        assert_eq!(pair.projects[1].read_path, "Fixture/Sources/Sentinel.txt");
        assert_eq!(
            fs::read(pair.projects[0].root.join("Sources/Sentinel.txt")).unwrap(),
            b"CW_I1_A\n"
        );
        assert_eq!(
            fs::read(pair.projects[1].root.join("Sources/Sentinel.txt")).unwrap(),
            b"CW_I1_B\n"
        );
        assert!(create_pair(&fresh).is_err());
        let before = seed_digests(&pair).unwrap();
        assert_eq!(before.len(), 8);
        fs::write(
            pair.projects[0].root.join("Sources/Sentinel.txt"),
            "changed",
        )
        .unwrap();
        assert_ne!(seed_digests(&pair).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn i1_selection_rejects_linked_package_and_parent() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(outside.path().join("Target.xcodeproj")).unwrap();
        symlink(
            outside.path().join("Target.xcodeproj"),
            root.join("Link.xcodeproj"),
        )
        .unwrap();
        assert!(select_project(&root, None).is_err());
        assert!(select_project(&root, Some(Path::new("Link.xcodeproj"))).is_err());
        symlink(outside.path(), root.join("nested")).unwrap();
        assert!(select_project(&root, Some(Path::new("nested/Target.xcodeproj"))).is_err());
    }
}
