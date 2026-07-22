//! Read-only, deterministic analysis of immutable FIPS artifacts.

mod causal;
mod compare;
mod document;
mod export;
mod network;
mod query;
mod root_wave;

pub use compare::{Comparison, MetricDelta, compare};
pub use document::{
    AnalysisDocument, AnalysisError, FidelityLabel, MetricSummary, QuiescenceSummary,
    Representation, SourceRange, analyze,
};
pub use export::{ExportLimits, export_static};
pub use network::{NetworkEdge, NetworkNode, NetworkView};
pub use query::{EventQuery, EventQueryResult, query_events};
pub use root_wave::{RootWave, RootWavePoint};
