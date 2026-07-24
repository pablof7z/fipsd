use super::*;

impl IndividualEngine {
    /// Execute one fully resolved normalized case.
    pub fn run_plan(&self, plan: &NormalizedPlan) -> Result<IndividualRun, RunError> {
        self.run_plan_streaming(plan, &mut |_| Ok(()))
    }

    /// Execute a normalized case while emitting each ordered event as it occurs.
    pub fn run_plan_streaming(
        &self,
        plan: &NormalizedPlan,
        observer: &mut impl FnMut(&EventRecord) -> Result<(), String>,
    ) -> Result<IndividualRun, RunError> {
        let config = RunConfig::from_plan(plan)?;
        let state = Simulation::new(plan.clone(), config)?.run_with_observer(observer)?;
        let mut run = state.finish()?;
        if RecoveryEngine::protocol_requested(plan) && !GraphRecoveryRuntime::requested(plan) {
            attach_coupled_recovery(plan, &mut run)?;
        }
        Ok(run)
    }
}

fn attach_coupled_recovery(plan: &NormalizedPlan, run: &mut IndividualRun) -> Result<(), RunError> {
    let recovery = RecoveryEngine.run(plan, &run.report)?;
    let (bloom, approximations) = recovery.fidelity();
    run.artifact.manifest.fidelity.bloom = bloom;
    run.artifact
        .manifest
        .fidelity
        .approximations
        .extend(approximations.clone());
    run.reproduction.fidelity.bloom = bloom;
    run.reproduction
        .fidelity
        .approximations
        .extend(approximations);
    run.artifact.metric_series.extend(recovery.metric_series());
    run.artifact
        .causal_ledger
        .extend(recovery.causal_ledger.clone());
    run.artifact
        .assertion_results
        .extend(recovery.assertions.clone());
    run.reproduction.expected_assertions.extend(
        recovery
            .assertions
            .iter()
            .map(|assertion| assertion.id.clone()),
    );
    run.artifact.samples.push(serde_json::to_value(&recovery)?);
    run.artifact.validate()?;
    run.recovery_report = Some(recovery);
    Ok(())
}

impl Engine for IndividualEngine {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            name: ENGINE_NAME.to_owned(),
            version: ENGINE_VERSION.to_owned(),
            source_revision: engine_source_revision(),
        }
    }

    fn validate(&self, request: &EngineRequest) -> Result<(), EngineError> {
        if request.variant != "fips-80c956a-baseline" {
            return Err(EngineError::Unsupported(format!(
                "individual M1 engine does not support variant {}",
                request.variant
            )));
        }
        RunConfig::from_plan(&request.plan)
            .map(|_| ())
            .map_err(|error| EngineError::Unsupported(error.to_string()))
    }

    fn run(&self, request: &EngineRequest) -> Result<Vec<EngineEffect>, EngineError> {
        self.validate(request)?;
        let run = self
            .run_plan(&request.plan)
            .map_err(|error| EngineError::Invariant(error.to_string()))?;
        Ok(run
            .artifact
            .event_trace
            .into_iter()
            .map(|event| EngineEffect {
                causal_id: event.event_id,
                ordinal: event.ordinal,
                kind: event.kind,
                payload: event.data,
                virtual_time_ns: event.virtual_time_ns,
            })
            .collect())
    }
}
