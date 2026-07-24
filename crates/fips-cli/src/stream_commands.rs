use crate::io_helpers::{write_bytes, write_json};
use anyhow::{Context, Result};
use fips_engine::IndividualEngine;
use serde::Serialize;
use serde_json::json;
use std::io::{BufWriter, Write};
use std::path::Path;

pub const EVENT_STREAM_VERSION: &str = "experiments.fips.network/event-stream/v1alpha1";

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    api_version: &'static str,
    kind: &'static str,
    payload: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<&'a str>,
}

pub fn run(campaign: &Path, output: Option<&Path>) -> Result<()> {
    let plan = fips_model::normalize_path(campaign)
        .with_context(|| format!("normalization failed for {}", campaign.display()))?;
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    write_line(
        &mut writer,
        &Envelope {
            api_version: EVENT_STREAM_VERSION,
            kind: "stream-start",
            payload: json!({"normalized_plan": plan}),
            run_id: None,
        },
    )?;
    let run = IndividualEngine
        .run_plan_streaming(&plan, &mut |event| {
            write_line(
                &mut writer,
                &Envelope {
                    api_version: EVENT_STREAM_VERSION,
                    kind: "event",
                    payload: event,
                    run_id: None,
                },
            )
            .map_err(|error| error.to_string())
        })
        .with_context(|| format!("run failed for {}", campaign.display()))?;
    if let Some(directory) = output {
        write_evidence(directory, &run)?;
    }
    write_line(
        &mut writer,
        &Envelope {
            api_version: EVENT_STREAM_VERSION,
            kind: "stream-complete",
            payload: json!({
                "artifact_id": run.artifact.manifest.artifact_id,
                "outcome": if run.report.assertions.iter().all(|item| item.outcome == "pass") {
                    "pass"
                } else {
                    "fail"
                },
                "fidelity": run.artifact.manifest.fidelity,
                "report": run.report,
                "evidence_path": output.map(Path::to_path_buf),
            }),
            run_id: Some(&run.artifact.manifest.run_id),
        },
    )
}

fn write_evidence(directory: &Path, run: &fips_engine::IndividualRun) -> Result<()> {
    write_bytes(
        &directory.join("artifact.json"),
        &run.artifact.to_canonical_json()?,
    )?;
    write_bytes(
        &directory.join("reproduction.json"),
        &run.reproduction.to_canonical_json()?,
    )?;
    write_json(&directory.join("report.json"), &run.report)?;
    if let Some(recovery) = &run.recovery_report {
        write_json(&directory.join("recovery-report.json"), recovery)?;
    }
    Ok(())
}

fn write_line(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn stream_version_is_namespaced() {
        assert_eq!(
            EVENT_STREAM_VERSION,
            "experiments.fips.network/event-stream/v1alpha1"
        );
    }

    #[test]
    fn output_path_remains_optional() {
        let path = Some(PathBuf::from("run"));
        assert_eq!(path.as_deref(), Some(Path::new("run")));
    }
}
