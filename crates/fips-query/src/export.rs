use crate::document::{AnalysisError, analyze};
use fips_artifact::RunArtifact;
use serde_json::{Value, to_vec_pretty};
use std::fs;
use std::path::{Component, Path};

const INDEX_HTML: &str = include_str!("../../../web/index.html");
const APP_JS: &str = include_str!("../../../web/app.js");
const WORKER_JS: &str = include_str!("../../../web/worker.js");
const STYLES_CSS: &str = include_str!("../../../web/styles.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportLimits {
    pub maximum_artifact_bytes: u64,
}

impl Default for ExportLimits {
    fn default() -> Self {
        Self {
            maximum_artifact_bytes: 64 * 1024 * 1024,
        }
    }
}

pub fn export_static(
    artifact: &RunArtifact,
    output: &Path,
    limits: ExportLimits,
) -> Result<(), AnalysisError> {
    ensure_safe(output)?;
    let analysis = analyze(artifact)?;
    let mut artifact_value = serde_json::to_value(artifact)?;
    redact(&mut artifact_value);
    let mut artifact_bytes = to_vec_pretty(&artifact_value)?;
    artifact_bytes.push(b'\n');
    if artifact_bytes.len() as u64 > limits.maximum_artifact_bytes {
        return Err(AnalysisError::SizeLimit {
            limit: limits.maximum_artifact_bytes,
            actual: artifact_bytes.len() as u64,
        });
    }
    fs::create_dir_all(output).map_err(|source| io_error(output, source))?;
    write(output.join("index.html"), INDEX_HTML.as_bytes())?;
    write(output.join("app.js"), APP_JS.as_bytes())?;
    write(output.join("worker.js"), WORKER_JS.as_bytes())?;
    write(output.join("styles.css"), STYLES_CSS.as_bytes())?;
    write(output.join("artifact.json"), &artifact_bytes)?;
    let mut analysis_value = serde_json::to_value(&analysis)?;
    redact(&mut analysis_value);
    let mut bytes = to_vec_pretty(&analysis_value)?;
    bytes.push(b'\n');
    write(output.join("analysis.json"), &bytes)?;
    let json = serde_json::to_string(&analysis_value)?;
    write(
        output.join("data.js"),
        format!("window.__FIPS_ANALYSIS__ = {json};\n").as_bytes(),
    )
}

fn redact(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let key = key.to_ascii_lowercase();
                if [
                    "private_key",
                    "secret",
                    "credential",
                    "access_token",
                    "refresh_token",
                ]
                .iter()
                .any(|needle| key.contains(needle))
                {
                    *child = Value::String("[redacted]".to_owned());
                } else {
                    redact(child);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact),
        Value::String(text) => {
            if let Some(index) = text.find("/Users/").or_else(|| text.find("/home/")) {
                let suffix = text[index..]
                    .splitn(4, '/')
                    .skip(3)
                    .collect::<Vec<_>>()
                    .join("/");
                *text = if suffix.is_empty() {
                    "$HOME".to_owned()
                } else {
                    format!("$HOME/{suffix}")
                };
            }
        }
        _ => {}
    }
}

fn ensure_safe(path: &Path) -> Result<(), AnalysisError> {
    if path.as_os_str().is_empty()
        || path.parent().is_none()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(AnalysisError::UnsafePath(path.display().to_string()));
    }
    Ok(())
}

fn write(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), AnalysisError> {
    let path = path.as_ref();
    fs::write(path, bytes).map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: std::io::Error) -> AnalysisError {
    AnalysisError::Io {
        path: path.display().to_string(),
        source,
    }
}
