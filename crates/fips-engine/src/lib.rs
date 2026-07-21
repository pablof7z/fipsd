//! Deterministic individual-node simulation for the M1/M2 engine path.
//!
//! The implementation is deliberately headless. It consumes normalized cases,
//! advances only injected virtual time, and emits immutable artifacts.

mod engine;
mod graph;
mod network;
mod scheduler;

pub use engine::{IndividualEngine, IndividualRun, RootRatchetReport, TreeAnnounceCounters};
pub use graph::{
    AttachmentSelector, EdgeId, GraphError, GraphMemoryFootprint, GraphStore, NodeAddress, NodeId,
    TopologyKind,
};
pub use network::{
    Delivery, EnqueueRequest, LinkClass, LinkConfig, LinkCounters, LinkError, LinkOrdering,
    LinkService,
};
pub use scheduler::{CausalEvent, EventId, ScheduleError, Scheduler, SchedulerDiagnostics};
