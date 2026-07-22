use anyhow::{Context, Result, bail};
use clap::Subcommand;
use fips_audit::{
    PackageManifest, ReleaseAudit, audit_release, benchmark, build_manifest, verify_manifest,
};
use serde::{Serialize, de::DeserializeOwned};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum ReleaseCommand {
    /// Generate the deterministic release-readiness audit.
    Audit {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Verify a checked-in release audit against live deterministic checks.
    VerifyAudit { audit: PathBuf },
    /// Measure local performance without extrapolating to other hosts.
    Benchmark {
        #[arg(long, default_value_t = 20)]
        iterations: usize,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Inventory and checksum a staged release directory.
    Manifest {
        root: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Verify every staged release file against its manifest.
    VerifyPackage { root: PathBuf, manifest: PathBuf },
}

pub fn execute(command: ReleaseCommand) -> Result<()> {
    match command {
        ReleaseCommand::Audit { output } => {
            let audit = audit_release()?;
            if !audit.ready {
                bail!("release audit did not pass");
            }
            write_json(&output, &audit)?;
            println!("release audit: {}", output.display());
        }
        ReleaseCommand::VerifyAudit { audit } => {
            let actual: ReleaseAudit = read_json(&audit)?;
            let expected = audit_release()?;
            if actual != expected || !actual.ready {
                bail!("release audit differs from live deterministic checks");
            }
            println!("verified release audit for {}", actual.release);
        }
        ReleaseCommand::Benchmark { iterations, output } => {
            let report = benchmark(iterations)?;
            write_json(&output, &report)?;
            println!("measured {} benchmark cases", report.cases.len());
        }
        ReleaseCommand::Manifest { root, output } => {
            let manifest = build_manifest(&root)?;
            write_json(&output, &manifest)?;
            write_checksums(&root, &manifest)?;
            println!("release manifest: {} files", manifest.files.len());
        }
        ReleaseCommand::VerifyPackage { root, manifest } => {
            let manifest: PackageManifest = read_json(&manifest)?;
            verify_manifest(&root, &manifest)?;
            println!("verified {} package files", manifest.files.len());
        }
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("cannot read {}", path.display()))?,
    )
    .with_context(|| format!("invalid JSON {}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}

fn write_checksums(root: &Path, manifest: &PackageManifest) -> Result<()> {
    let mut output = String::new();
    for file in &manifest.files {
        output.push_str(&format!("{}  {}\n", file.sha256, file.path));
    }
    let path = root.join("checksums.sha256");
    fs::write(&path, output).with_context(|| format!("cannot write {}", path.display()))
}
