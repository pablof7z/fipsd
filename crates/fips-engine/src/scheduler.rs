//! Stable virtual time and total event ordering.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Stable scheduler event identifier.
pub type EventId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EventKey {
    virtual_time_ns: u64,
    ordinal: u64,
}

/// An event together with its stable causal metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalEvent<T> {
    /// Stable event identifier assigned at schedule time.
    pub id: EventId,
    /// Parent event, if this event was caused by another event.
    pub parent: Option<EventId>,
    /// Virtual execution time.
    pub virtual_time_ns: u64,
    /// Total-order tie breaker independent of hash iteration or wall time.
    pub ordinal: u64,
    /// Caller-owned event payload.
    pub payload: T,
}

/// Scheduler counters included in diagnostics and run reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerDiagnostics {
    /// Events accepted by the scheduler.
    pub scheduled: u64,
    /// Events explicitly cancelled, including cancelled descendants.
    pub cancelled: u64,
    /// Events replaced through a coalescing key.
    pub superseded: u64,
    /// Events returned for execution.
    pub executed: u64,
    /// Maximum pending-event count.
    pub peak_pending: u64,
}

/// Virtual scheduler failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    /// The bounded pending-event capacity was exhausted.
    #[error("scheduler pending-event limit {limit} exceeded")]
    Capacity {
        /// Configured pending-event limit.
        limit: usize,
    },
    /// Time arithmetic overflowed.
    #[error("virtual-time overflow")]
    TimeOverflow,
}

/// Single-threaded deterministic scheduler with stable tie breaking.
#[derive(Debug, Clone)]
pub struct Scheduler<T> {
    now_ns: u64,
    next_id: EventId,
    next_ordinal: u64,
    max_pending: usize,
    queue: BTreeMap<EventKey, CausalEvent<T>>,
    by_id: BTreeMap<EventId, EventKey>,
    children: BTreeMap<EventId, BTreeSet<EventId>>,
    coalesced: BTreeMap<String, EventId>,
    coalescing_key: BTreeMap<EventId, String>,
    diagnostics: SchedulerDiagnostics,
}

impl<T> Scheduler<T> {
    /// Create a scheduler with an explicit pending-event bound.
    pub fn new(max_pending: usize) -> Self {
        Self {
            now_ns: 0,
            next_id: 1,
            next_ordinal: 0,
            max_pending,
            queue: BTreeMap::new(),
            by_id: BTreeMap::new(),
            children: BTreeMap::new(),
            coalesced: BTreeMap::new(),
            coalescing_key: BTreeMap::new(),
            diagnostics: SchedulerDiagnostics::default(),
        }
    }

    /// Current injected virtual time.
    pub fn now_ns(&self) -> u64 {
        self.now_ns
    }

    /// Number of pending events.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// True when no event is pending.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Read deterministic scheduler diagnostics.
    pub fn diagnostics(&self) -> &SchedulerDiagnostics {
        &self.diagnostics
    }

    /// Schedule at an absolute virtual time.
    pub fn schedule_at(
        &mut self,
        virtual_time_ns: u64,
        parent: Option<EventId>,
        payload: T,
    ) -> Result<EventId, ScheduleError> {
        if self.queue.len() >= self.max_pending {
            return Err(ScheduleError::Capacity {
                limit: self.max_pending,
            });
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(ScheduleError::TimeOverflow)?;
        let ordinal = self.next_ordinal;
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or(ScheduleError::TimeOverflow)?;
        let key = EventKey {
            virtual_time_ns,
            ordinal,
        };
        let event = CausalEvent {
            id,
            parent,
            virtual_time_ns,
            ordinal,
            payload,
        };
        self.queue.insert(key, event);
        self.by_id.insert(id, key);
        if let Some(parent) = parent {
            self.children.entry(parent).or_default().insert(id);
        }
        self.diagnostics.scheduled += 1;
        self.diagnostics.peak_pending = self.diagnostics.peak_pending.max(self.queue.len() as u64);
        Ok(id)
    }

    /// Schedule relative to the current virtual time.
    pub fn schedule_after(
        &mut self,
        delay_ns: u64,
        parent: Option<EventId>,
        payload: T,
    ) -> Result<EventId, ScheduleError> {
        let at = self
            .now_ns
            .checked_add(delay_ns)
            .ok_or(ScheduleError::TimeOverflow)?;
        self.schedule_at(at, parent, payload)
    }

    /// Replace the pending event for `coalescing_key` with a new event.
    ///
    /// The replacement receives a new stable ID and ordinal. The old event and
    /// all of its already-scheduled causal descendants are removed.
    pub fn schedule_coalesced(
        &mut self,
        coalescing_key: impl Into<String>,
        virtual_time_ns: u64,
        parent: Option<EventId>,
        payload: T,
    ) -> Result<EventId, ScheduleError> {
        let coalescing_key = coalescing_key.into();
        if let Some(previous) = self.coalesced.remove(&coalescing_key) {
            if self.cancel_tree(previous) {
                self.diagnostics.superseded += 1;
            }
        }
        let id = self.schedule_at(virtual_time_ns, parent, payload)?;
        self.coalesced.insert(coalescing_key.clone(), id);
        self.coalescing_key.insert(id, coalescing_key);
        Ok(id)
    }

    /// Cancel an event and every pending causal descendant.
    pub fn cancel_tree(&mut self, id: EventId) -> bool {
        let descendants = self.children.remove(&id).unwrap_or_default();
        for child in descendants {
            self.cancel_tree(child);
        }
        let Some(key) = self.by_id.remove(&id) else {
            return false;
        };
        let event = self.queue.remove(&key);
        if let Some(event) = &event
            && let Some(parent) = event.parent
            && let Some(siblings) = self.children.get_mut(&parent)
        {
            siblings.remove(&id);
        }
        if let Some(coalescing_key) = self.coalescing_key.remove(&id)
            && self.coalesced.get(&coalescing_key) == Some(&id)
        {
            self.coalesced.remove(&coalescing_key);
        }
        if event.is_some() {
            self.diagnostics.cancelled += 1;
            true
        } else {
            false
        }
    }

    /// Pop the next event in total order and advance virtual time.
    pub fn pop(&mut self) -> Option<CausalEvent<T>> {
        let key = *self.queue.keys().next()?;
        let event = self.queue.remove(&key)?;
        self.by_id.remove(&event.id);
        if let Some(parent) = event.parent
            && let Some(children) = self.children.get_mut(&parent)
        {
            children.remove(&event.id);
        }
        if let Some(coalescing_key) = self.coalescing_key.remove(&event.id)
            && self.coalesced.get(&coalescing_key) == Some(&event.id)
        {
            self.coalesced.remove(&coalescing_key);
        }
        self.now_ns = self.now_ns.max(event.virtual_time_ns);
        self.diagnostics.executed += 1;
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_499_500_501_order_is_stable() {
        let mut scheduler = Scheduler::new(8);
        scheduler.schedule_at(500, None, "500").unwrap();
        scheduler.schedule_at(499, None, "499").unwrap();
        scheduler.schedule_at(501, None, "501").unwrap();
        scheduler.schedule_at(500, None, "500-second").unwrap();
        let ordered = std::iter::from_fn(|| scheduler.pop())
            .map(|event| event.payload)
            .collect::<Vec<_>>();
        assert_eq!(ordered, ["499", "500", "500-second", "501"]);
    }

    #[test]
    fn coalescing_and_cancellation_leave_no_orphans() {
        let mut scheduler = Scheduler::new(8);
        let parent = scheduler.schedule_at(10, None, "parent").unwrap();
        let replaced = scheduler
            .schedule_coalesced("peer-1", 20, Some(parent), "old")
            .unwrap();
        scheduler.schedule_at(30, Some(replaced), "child").unwrap();
        scheduler
            .schedule_coalesced("peer-1", 20, Some(parent), "new")
            .unwrap();
        let events = std::iter::from_fn(|| scheduler.pop()).collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, "parent");
        assert_eq!(events[1].payload, "new");
        assert_eq!(scheduler.diagnostics().cancelled, 2);
        assert_eq!(scheduler.diagnostics().superseded, 1);
    }

    #[test]
    fn pending_bound_fails_loud() {
        let mut scheduler = Scheduler::new(1);
        scheduler.schedule_at(1, None, ()).unwrap();
        assert_eq!(
            scheduler.schedule_at(2, None, ()),
            Err(ScheduleError::Capacity { limit: 1 })
        );
    }
}
