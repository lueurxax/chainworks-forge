use super::{
    host::ServiceGeneration,
    project::{digest, private_dir, write_new, FixtureProject},
    tools::OpenResult,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deviation {
    pub category: String,
    pub payload_sha256: String,
    pub shape: Value,
}
#[derive(Debug)]
pub struct DeviationError(pub Deviation);
impl std::fmt::Display for DeviationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.category)
    }
}
impl std::error::Error for DeviationError {}
pub fn retain(error: anyhow::Error, payload: &Value) -> anyhow::Error {
    if error.downcast_ref::<DeviationError>().is_some() {
        return error;
    }
    fn shape(v: &Value, depth: usize) -> Value {
        let kind = match v {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        if depth == 0 || !v.is_object() {
            return Value::String(kind.into());
        }
        let mut out = serde_json::Map::new();
        out.insert("_type".into(), Value::String(kind.into()));
        out.insert(
            "_field_count".into(),
            Value::from(v.as_object().unwrap().len()),
        );
        for key in [
            "structuredContent",
            "content",
            "isError",
            "workspaceIdentifier",
            "workspacePath",
            "filePath",
            "fileSize",
            "totalLines",
            "linesRead",
            "startLine",
            "id",
            "path",
            "error",
            "result",
        ] {
            if let Some(value) = v.get(key) {
                out.insert(key.into(), shape(value, depth - 1));
            }
        }
        Value::Object(out)
    }
    DeviationError(Deviation {
        category: super::controller::category(&error).into(),
        payload_sha256: digest(&serde_json::to_vec(payload).unwrap_or_default()),
        shape: shape(payload, 3),
    })
    .into()
}

pub trait Recorder: Send {
    fn deviation(&mut self, _deviation: &Deviation) -> Result<()> {
        Ok(())
    }
    fn begin_open(
        &mut self,
        project: &FixtureProject,
        generation: &ServiceGeneration,
    ) -> Result<()>;
    fn finish_open(&mut self, label: &str, result: &OpenResult) -> Result<()>;
    fn unknown_open(&mut self, label: &str) -> Result<()>;
}
pub struct LocalRecorder {
    dir: PathBuf,
    _lock: Option<File>,
    #[cfg(test)]
    pub sync_fault: Option<&'static str>,
}
impl LocalRecorder {
    pub fn create(dir: &Path) -> Result<Self> {
        private_dir(dir)?;
        File::open(
            dir.parent()
                .ok_or_else(|| anyhow::anyhow!("i1_record_parent"))?,
        )?
        .sync_all()?;
        write_new(&dir.join("lock"), b"I1 exclusive attempt\n")?;
        let lock = File::open(dir.join("lock"))?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            ensure!(
                unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
                "i1_record_locked"
            );
        }
        Ok(Self {
            dir: dir.into(),
            _lock: Some(lock),
            #[cfg(test)]
            sync_fault: None,
        })
    }
    pub fn unresolved(dir: &Path) -> Result<bool> {
        for label in ["A", "B"] {
            if dir.join(format!("{label}.intent.json")).exists() {
                let terminal = dir.join(format!("{label}.result.json"));
                let valid = std::fs::read(terminal)
                    .ok()
                    .filter(|b| {
                        std::fs::read_to_string(dir.join(format!("{label}.committed.json")))
                            .ok()
                            .as_deref()
                            == Some(digest(b).as_str())
                    })
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .is_some_and(|v| {
                        v["version"] == 1
                            && v["label"] == label
                            && serde_json::from_value::<OpenResult>(v["result"].clone()).is_ok()
                    });
                if !valid {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn path(&self, label: &str, kind: &str) -> Result<PathBuf> {
        ensure!(matches!(label, "A" | "B"), "i1_fixture_label");
        Ok(self.dir.join(format!("{label}.{kind}.json")))
    }
    fn persist(&self, path: &Path, bytes: &[u8], phase: &str) -> Result<()> {
        super::project::write_new_with_sync(path, bytes, |point| {
            #[cfg(test)]
            {
                if self.sync_fault == Some(format!("{phase}_{point}").as_str()) {
                    return Err(std::io::Error::from_raw_os_error(libc::EIO).into());
                }
            }
            let _ = (phase, point);
            Ok(())
        })
    }
}
impl Recorder for LocalRecorder {
    fn deviation(&mut self, deviation: &Deviation) -> Result<()> {
        write_new(
            &self.dir.join("deviation.json"),
            &serde_json::to_vec_pretty(deviation)?,
        )
    }
    fn begin_open(&mut self, p: &FixtureProject, g: &ServiceGeneration) -> Result<()> {
        let request =
            serde_json::json!({"name":"XcodeOpenWorkspace","arguments":{"path":p.package}});
        let v = serde_json::json!({"version":1,"label":p.label,"package":p.package,"generation":g,
            "request_sha256":digest(&serde_json::to_vec(&request)?),"at":chrono::Utc::now().to_rfc3339()});
        self.persist(
            &self.path(&p.label, "intent")?,
            &serde_json::to_vec_pretty(&v)?,
            "intent",
        )
    }
    fn finish_open(&mut self, label: &str, r: &OpenResult) -> Result<()> {
        ensure!(self.path(label, "intent")?.is_file(), "i1_missing_intent");
        let v = serde_json::json!({"version":1,"label":label,"result":r,"at":chrono::Utc::now().to_rfc3339()});
        let bytes = serde_json::to_vec_pretty(&v)?;
        self.persist(&self.path(label, "result")?, &bytes, "result")?;
        write_new(&self.path(label, "committed")?, digest(&bytes).as_bytes())
    }
    fn unknown_open(&mut self, label: &str) -> Result<()> {
        write_new(
            &self.path(label, "unknown")?,
            b"{\"version\":1,\"state\":\"uncertain\"}\n",
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn i1_intent_cannot_be_reissued_or_resumed() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("record");
        let mut record = LocalRecorder::create(&dir).unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        record
            .begin_open(
                &pair.projects[0],
                &super::super::host::fixture_facts().generation,
            )
            .unwrap();
        assert!(LocalRecorder::unresolved(&dir).unwrap());
        assert!(record
            .begin_open(
                &pair.projects[0],
                &super::super::host::fixture_facts().generation
            )
            .is_err());
        drop(record);
        assert!(LocalRecorder::create(&dir).is_err());
    }
    #[test]
    fn i1_terminal_evidence_is_exclusive_and_private() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("record");
        let mut record = LocalRecorder::create(&dir).unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        record
            .begin_open(
                &pair.projects[0],
                &super::super::host::fixture_facts().generation,
            )
            .unwrap();
        let opened = OpenResult {
            id: "a".into(),
            path: None,
        };
        record.finish_open("A", &opened).unwrap();
        assert!(!LocalRecorder::unresolved(&dir).unwrap());
        assert!(record.finish_open("A", &opened).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(dir.join("A.intent.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
