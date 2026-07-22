//! Deterministic individual-node simulation for the M1/M2 engine path.
//!
//! The implementation is deliberately headless. It consumes normalized cases,
//! advances only injected virtual time, and emits immutable artifacts.

mod bloom;
mod cache;
mod engine;
mod graph;
mod lookup;
mod network;
mod recovery;
mod resources;
mod scheduler;
mod traffic;

pub use bloom::*;
pub use cache::*;
pub use engine::{IndividualEngine, IndividualRun, RootRatchetReport, TreeAnnounceCounters};
pub use graph::{
    AttachmentSelector, EdgeId, GraphError, GraphMemoryFootprint, GraphStore, NodeAddress, NodeId,
    TopologyKind,
};
pub use lookup::*;
pub use network::{
    Delivery, EnqueueRequest, LinkClass, LinkConfig, LinkCounters, LinkError, LinkOrdering,
    LinkService,
};
pub use recovery::*;
pub use resources::*;
pub use scheduler::{CausalEvent, EventId, ScheduleError, Scheduler, SchedulerDiagnostics};
pub use traffic::*;
