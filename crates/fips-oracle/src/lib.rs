//! Pinned real-daemon harness import, telemetry, provenance, and differential oracle.

mod compiler;
mod differential;
mod fuzz;
mod importer;
mod oracle;
mod process_backend;
mod provenance;
mod suites;
mod telemetry;

pub use compiler::*;
pub use differential::*;
pub use fuzz::*;
pub use importer::*;
pub use oracle::*;
pub use process_backend::*;
pub use provenance::*;
pub use suites::*;
pub use telemetry::*;

pub const PINNED_FIPS_COMMIT: &str = "80c956a6fdb85dde1450969a21891c1158e43267";
pub const CHAOS_ADAPTER_VERSION: &str = "fips-chaos-80c956a/v1";
