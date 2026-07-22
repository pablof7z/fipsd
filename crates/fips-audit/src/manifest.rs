use crate::AuditError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    pub api_version: String,
    pub release: String,
    pub files: Vec<PackageFile>,
    pub total_bytes: u64,
    pub compatibility: Vec<String>,
    pub signing: String,
    pub upgrade_policy: String,
}

pub fn build_manifest(root: &Path) -> Result<PackageManifest, AuditError> {
    let root = safe_root(root)?;
    let mut paths = Vec::new();
    collect(&root, &root, &mut paths)?;
    paths.sort();
    let mut total_bytes = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for relative in paths {
        let bytes =
            fs::read(root.join(&relative)).map_err(|source| io(root.join(&relative), source))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(AuditError::SizeLimit(relative.display().to_string()));
        }
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        if total_bytes > MAX_PACKAGE_BYTES {
            return Err(AuditError::SizeLimit(root.display().to_string()));
        }
        files.push(PackageFile {
            path: normalized(&relative)?,
            size_bytes: bytes.len() as u64,
            sha256: hash(&bytes),
        });
    }
    Ok(PackageManifest {
        api_version: "experiments.fips.network/release-manifest/v1alpha1".to_owned(),
        release: "0.1.0".to_owned(),
        files,
        total_bytes,
        compatibility: vec![
            "run-artifact/v1alpha1".to_owned(),
            "campaign/v1alpha1".to_owned(),
            "analysis/v1alpha1".to_owned(),
            "qualification-atlas/v1alpha1".to_owned(),
        ],
        signing: "local packages carry SHA-256; hosted release requires GitHub artifact attestation".to_owned(),
        upgrade_policy: "v0.1 readers reject unknown major schema versions; immutable source artifacts are never rewritten".to_owned(),
    })
}

pub fn verify_manifest(root: &Path, manifest: &PackageManifest) -> Result<(), AuditError> {
    let expected = build_manifest(root)?;
    if expected != *manifest {
        return Err(AuditError::Checksum(
            "file inventory, size, or SHA-256 differs".to_owned(),
        ));
    }
    Ok(())
}

fn collect(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), AuditError> {
    let entries = fs::read_dir(directory).map_err(|source| io(directory, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| io(directory, source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| io(entry.path(), source))?;
        if file_type.is_symlink() {
            return Err(AuditError::UnsafePath(entry.path().display().to_string()));
        }
        if file_type.is_dir() {
            collect(root, &entry.path(), output)?;
        } else if file_type.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("descendant")
                .to_owned();
            if !matches!(
                relative.to_str(),
                Some("release-manifest.json" | "checksums.sha256" | "checksums.sha256.sig")
            ) {
                output.push(relative);
            }
        }
    }
    Ok(())
}

fn safe_root(root: &Path) -> Result<PathBuf, AuditError> {
    if root.as_os_str().is_empty() || root.parent().is_none() {
        return Err(AuditError::UnsafePath(root.display().to_string()));
    }
    let canonical = root.canonicalize().map_err(|source| io(root, source))?;
    if !canonical.is_dir() {
        return Err(AuditError::UnsafePath(root.display().to_string()));
    }
    Ok(canonical)
}

fn normalized(path: &Path) -> Result<String, AuditError> {
    if path.components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(AuditError::UnsafePath(path.display().to_string()));
    }
    Ok(path.to_string_lossy().replace('\\', "/"))
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn io(path: impl AsRef<Path>, source: std::io::Error) -> AuditError {
    AuditError::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}
