use super::*;

impl Simulation {
    pub(super) fn schedule_inputs(&mut self) -> Result<(), RunError> {
        self.scheduler
            .schedule_at(0, None, SimEvent::InitialAnnounce)?;
        self.schedule_traffic()?;
        let first_arrival = self.graph.node_count() as u32 - self.config.reserved_arrivals;
        for ordinal in 0..self.config.arrivals {
            let at = self
                .config
                .arrival_interval_ns
                .checked_mul(u64::from(ordinal))
                .and_then(|offset| self.config.arrival_start_ns.checked_add(offset))
                .ok_or(RunError::Arithmetic)?;
            self.scheduler.schedule_at(
                at,
                None,
                SimEvent::Activate {
                    node: first_arrival + ordinal,
                    ordinal,
                    lower_root: true,
                    targets: Vec::new(),
                },
            )?;
        }
        for (manual, input) in self.config.manual_arrivals.iter().enumerate() {
            let ordinal = self.config.arrivals + manual as u32;
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::Activate {
                    node: first_arrival + ordinal,
                    ordinal,
                    lower_root: input.lower_root,
                    targets: input.targets.clone(),
                },
            )?;
        }
        if let Some(at) = self.config.inject_parent_loop_at_ns {
            self.scheduler
                .schedule_at(at, None, SimEvent::InjectParentLoop)?;
        }
        for lifecycle in &self.config.lifecycle {
            let payload = if lifecycle.reappear {
                SimEvent::Reappear {
                    node: lifecycle.node,
                }
            } else {
                SimEvent::Deactivate {
                    node: lifecycle.node,
                }
            };
            self.scheduler.schedule_at(lifecycle.at_ns, None, payload)?;
        }
        for input in &self.config.rekeys {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::SessionRekey {
                    input: input.clone(),
                },
            )?;
        }
        for input in &self.config.cache_expiries {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::ExpireCoordinateCache {
                    input: input.clone(),
                },
            )?;
        }
        for input in &self.config.lookup_waves {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::LookupWave {
                    input: input.clone(),
                },
            )?;
        }
        for input in &self.config.transport_classes {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::TransportClass {
                    input: input.clone(),
                },
            )?;
        }
        for input in &self.config.parent_costs {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::ParentCost {
                    input: input.clone(),
                },
            )?;
        }
        let sybil_start =
            first_arrival + self.config.arrivals + self.config.manual_arrivals.len() as u32;
        for input in &self.config.sybils {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::SybilArrival {
                    input: input.clone(),
                    node: sybil_start + input.ordinal,
                },
            )?;
        }
        for input in &self.config.cuts {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::NetworkCut {
                    input: input.clone(),
                },
            )?;
        }
        for input in &self.config.link_updates {
            self.scheduler.schedule_at(
                input.at_ns,
                None,
                SimEvent::LinkUpdate {
                    input: input.clone(),
                },
            )?;
        }
        Ok(())
    }
}
