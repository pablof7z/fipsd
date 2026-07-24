use super::*;

impl Simulation {
    pub(crate) fn reject_flow(&mut self, flow: &Flow, reason: &str, evidence: &str) {
        let counters = &mut self.traffic.as_mut().unwrap().counters;
        counters.rejected_flows += 1;
        counters.lost_useful_bytes += flow.useful_payload_bytes;
        self.add_ledger(&flow.id, "rejected", flow.useful_payload_bytes, evidence);
        self.add_ledger(&flow.id, "lost-payload", flow.useful_payload_bytes, reason);
    }
}
