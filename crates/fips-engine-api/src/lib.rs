//! Engine contracts only; M0 deliberately ships no simulation engine.

use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Stable engine identity recorded in provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineIdentity {
    /// Engine implementation name.
    pub name: String,
    /// Semantic implementation version.
    pub version: String,
    /// Source revision used to build it.
    pub source_revision: String,
}

/// Deterministic request passed to an engine implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineRequest {
    /// Normalized plan.
    pub plan: NormalizedPlan,
    /// Protocol variant identifier.
    pub variant: String,
}

/// Ordered effect emitted by a future engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineEffect {
    /// Stable causal identifier.
    pub causal_id: String,
    /// Total-order ordinal at one virtual timestamp.
    pub ordinal: u64,
    /// Versioned effect kind.
    pub kind: String,
    /// Effect payload.
    pub payload: Value,
    /// Injected virtual time in integer nanoseconds.
    pub virtual_time_ns: u64,
}

/// Engine failures cannot silently degrade fidelity.
#[derive(Debug, Error)]
pub enum EngineError {
    /// The requested fidelity/variant is unsupported.
    #[error("unsupported engine request: {0}")]
    Unsupported(String),
    /// An invariant failed.
    #[error("engine invariant failed: {0}")]
    Invariant(String),
}

/// Pluggable deterministic engine seam.
pub trait Engine {
    /// Stable identity for provenance.
    fn identity(&self) -> EngineIdentity;
    /// Validate support without changing the request or degrading fidelity.
    fn validate(&self, request: &EngineRequest) -> Result<(), EngineError>;
    /// Execute and emit effects in total order.
    fn run(&self, request: &EngineRequest) -> Result<Vec<EngineEffect>, EngineError>;
}
