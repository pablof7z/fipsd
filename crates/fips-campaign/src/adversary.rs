//! Authenticated protocol-valid adversary policies and deterministic budgets.

use fips_artifact::LedgerEntry;
use serde::{Deserialize, Serialize};

/// Hostile authenticated action, distinct from malformed-wire fuzzing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdversaryPolicy {
    IdentityGrinding,
    SybilConcentration,
    ConnectChurn,
    EclipseVisibility,
    StrategicLatency,
    WithheldForwarding,
    AbsentTargetLookup,
    NearCapBloomPressure,
    DishonestAncestry,
}

/// Attacker accounting policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttackerBudgetMode {
    Free,
    OperationCounted,
    Calibrated,
    Bounded,
}

/// Explicit adversary budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackerBudget {
    pub mode: AttackerBudgetMode,
    pub maximum_operations: Option<u64>,
    pub maximum_identities: Option<u64>,
    pub calibrated_units_per_operation: Option<u64>,
}

/// One requested authenticated action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversaryAction {
    pub policy: AdversaryPolicy,
    pub operations: u64,
    pub identities: u64,
    pub at_ns: u64,
}

/// Protocol interpretation of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolDisposition {
    AcceptedProtocolBehavior,
    RejectedProtocolBehavior,
    ModeledDishonestyAssumption,
}

/// Executed or rejected action record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversaryRecord {
    pub causal_id: String,
    pub causal_parent: Option<String>,
    pub policy: AdversaryPolicy,
    pub disposition: ProtocolDisposition,
    pub interpretation: String,
    pub operations_charged: u64,
    pub calibrated_units: u64,
    pub identities_charged: u64,
    pub at_ns: u64,
    pub accepted: bool,
    pub rejection: Option<String>,
}

/// Deterministic action/budget report using artifact ledger rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversaryReport {
    pub kind: String,
    pub authenticated: bool,
    pub malformed_wire_fuzzing: bool,
    pub budget: AttackerBudget,
    pub operations_consumed: u64,
    pub identities_consumed: u64,
    pub calibrated_units_consumed: u64,
    pub exhausted_actions: u64,
    pub records: Vec<AdversaryRecord>,
    pub ledger: Vec<LedgerEntry>,
}

/// Execute authenticated policies through the same causal accounting surface.
pub fn execute_adversary(budget: AttackerBudget, actions: &[AdversaryAction]) -> AdversaryReport {
    let mut report = AdversaryReport {
        kind: "authenticated-adversary-report/v1alpha1".to_owned(),
        authenticated: true,
        malformed_wire_fuzzing: false,
        budget: budget.clone(),
        operations_consumed: 0,
        identities_consumed: 0,
        calibrated_units_consumed: 0,
        exhausted_actions: 0,
        records: Vec::new(),
        ledger: Vec::new(),
    };
    let mut parent = None;
    for (ordinal, action) in actions.iter().enumerate() {
        let causal_id = format!("adversary:{ordinal:08}");
        let calibrated = action
            .operations
            .saturating_mul(budget.calibrated_units_per_operation.unwrap_or(0));
        let operation_total = report.operations_consumed.saturating_add(action.operations);
        let identity_total = report.identities_consumed.saturating_add(action.identities);
        let exhausted = budget.mode == AttackerBudgetMode::Bounded
            && (budget
                .maximum_operations
                .is_some_and(|maximum| operation_total > maximum)
                || budget
                    .maximum_identities
                    .is_some_and(|maximum| identity_total > maximum));
        let (disposition, interpretation) = interpretation(action.policy);
        let rejection = exhausted.then(|| {
            format!(
                "attacker budget exhausted at operations={operation_total}, identities={identity_total}"
            )
        });
        if exhausted {
            report.exhausted_actions += 1;
        } else {
            report.operations_consumed = operation_total;
            report.identities_consumed = identity_total;
            report.calibrated_units_consumed =
                report.calibrated_units_consumed.saturating_add(calibrated);
        }
        report.ledger.push(LedgerEntry {
            causal_id: causal_id.clone(),
            causal_parent: parent.clone(),
            stage: if exhausted { "rejected" } else { "performed" }.to_owned(),
            count: action.operations,
            evidence: vec![format!("authenticated:{:?}", action.policy)],
        });
        report.records.push(AdversaryRecord {
            causal_id: causal_id.clone(),
            causal_parent: parent.clone(),
            policy: action.policy,
            disposition,
            interpretation,
            operations_charged: if exhausted { 0 } else { action.operations },
            calibrated_units: if exhausted { 0 } else { calibrated },
            identities_charged: if exhausted { 0 } else { action.identities },
            at_ns: action.at_ns,
            accepted: !exhausted && disposition != ProtocolDisposition::RejectedProtocolBehavior,
            rejection,
        });
        parent = Some(causal_id);
    }
    report
}

fn interpretation(policy: AdversaryPolicy) -> (ProtocolDisposition, String) {
    let (disposition, text) = match policy {
        AdversaryPolicy::IdentityGrinding => (
            ProtocolDisposition::AcceptedProtocolBehavior,
            "authenticated identities may select favorable addresses; work is budgeted",
        ),
        AdversaryPolicy::SybilConcentration => (
            ProtocolDisposition::AcceptedProtocolBehavior,
            "multiple authenticated identities concentrate at selected attachments",
        ),
        AdversaryPolicy::ConnectChurn => (
            ProtocolDisposition::AcceptedProtocolBehavior,
            "authenticated peers repeatedly establish and close valid sessions",
        ),
        AdversaryPolicy::EclipseVisibility => (
            ProtocolDisposition::ModeledDishonestyAssumption,
            "peer selection is strategically limited without malformed messages",
        ),
        AdversaryPolicy::StrategicLatency => (
            ProtocolDisposition::AcceptedProtocolBehavior,
            "valid messages are delayed within configured transport behavior",
        ),
        AdversaryPolicy::WithheldForwarding => (
            ProtocolDisposition::ModeledDishonestyAssumption,
            "an authenticated peer declines forwarding it is expected to perform",
        ),
        AdversaryPolicy::AbsentTargetLookup => (
            ProtocolDisposition::AcceptedProtocolBehavior,
            "valid lookups intentionally select absent destinations",
        ),
        AdversaryPolicy::NearCapBloomPressure => (
            ProtocolDisposition::AcceptedProtocolBehavior,
            "valid inserts approach but do not cross the configured FPR cap",
        ),
        AdversaryPolicy::DishonestAncestry => (
            ProtocolDisposition::RejectedProtocolBehavior,
            "the receiver rejects ancestry that violates its loop checks",
        ),
    };
    (disposition, text.to_owned())
}
