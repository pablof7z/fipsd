self.onmessage = async ({data: file}) => {
  try {
    const artifact = JSON.parse(await file.text());
    const manifest = artifact.manifest;
    if (!manifest?.artifact_id || !manifest?.run_id || !manifest?.fidelity || !manifest?.provenance) {
      throw new Error("missing run artifact manifest fields");
    }
    const events = artifact.event_trace || [];
    const fidelity = manifest.fidelity;
    const scale = fidelity.scale;
    const representation = scale === "cohort" ? "cohort" : scale === "hybrid" ? "hybrid" : fidelity.represented_nodes <= 200 ? "exact-graph" : "aggregated";
    const metricSummary = (series) => ({
      name: series.name, unit: series.unit, first: series.points?.[0]?.value ?? null,
      last: series.points?.at(-1)?.value ?? null, minimum: null, maximum: null,
      source: {collection: `metric_series/${series.name}`, start: 0, end_exclusive: series.points?.length || 0, total: series.points?.length || 0}
    });
    const stages = new Map();
    for (const entry of artifact.causal_ledger || []) {
      const row = stages.get(entry.stage) || {stage: entry.stage, count: 0, entries: 0};
      row.count += entry.count; row.entries += 1; stages.set(entry.stage, row);
    }
    const q = (name) => artifact.metric_series?.find(item => item.name === name)?.points?.at(-1)?.value;
    const edgeMap = new Map(), nodeMap = new Map(), depthMap = new Map();
    for (const event of events) {
      const from = event.data?.from, to = event.data?.to;
      if (from === undefined || to === undefined) continue;
      nodeMap.set(from, (nodeMap.get(from) || 0) + 1); nodeMap.set(to, (nodeMap.get(to) || 0) + 1);
      const key = `${from}:${to}`, depth = event.data?.depth || 0, bytes = event.data?.frame_bytes || 0;
      const edge = edgeMap.get(key) || {from, to, frames: 0, frame_bytes: 0, maximum_depth: 0};
      edge.frames += 1; edge.frame_bytes += bytes; edge.maximum_depth = Math.max(edge.maximum_depth, depth); edgeMap.set(key, edge);
      depthMap.set(depth, (depthMap.get(depth) || 0) + 1);
    }
    const allEdges = [...edgeMap.values()];
    const descendants = new Map();
    for (const event of events) if (event.causal_parent) descendants.set(event.causal_parent, (descendants.get(event.causal_parent) || 0) + 1);
    const wave = events.filter(event => event.kind === "input.descending-root-arrival").map(event => ({causal_id: event.event_id, root: event.data?.address || "unknown", arrival_ns: event.virtual_time_ns, propagated_events: descendants.get(event.event_id) || 0, status: descendants.has(event.event_id) ? "adopted" : "coalesced"}));
    const analysis = {
      api_version: "experiments.fips.network/analysis/v1alpha1", artifact_id: manifest.artifact_id,
      run_id: manifest.run_id, represented_nodes: fidelity.represented_nodes, representation,
      representation_boundaries: {exact_graph_max_nodes: 200, aggregate_max_nodes: 1000000},
      fidelity: {exact: !(fidelity.approximations || []).length, statement: "See immutable artifact fidelity contract.", uncertainty: (fidelity.approximations || []).map(item => `${item.method}: ${item.uncertainty}`)},
      provenance: manifest.provenance,
      assertions: Object.fromEntries((artifact.assertion_results || []).map(item => [item.id, item.outcome])),
      metrics: (artifact.metric_series || []).map(metricSummary),
      quiescence: {root_ns: q("quiescence.root"), tree_ns: q("quiescence.tree"), bloom_ns: q("quiescence.bloom"), lookup_ns: q("quiescence.lookup"), data_plane_ns: q("quiescence.data-plane")},
      causal: {stages: [...stages.values()].sort((a,b) => a.stage.localeCompare(b.stage)), roots: [], critical_path: [], source_entries: artifact.causal_ledger?.length || 0},
      network: {mode: representation === "exact-graph" ? "exact" : "aggregated", exact_nodes: representation === "exact-graph" ? [...nodeMap].map(([id, observed_events]) => ({id, observed_events})) : [], exact_edges: representation === "exact-graph" ? allEdges : [], depth_frame_distribution: Object.fromEntries(depthMap), top_edges: allEdges.sort((a,b) => b.frame_bytes - a.frame_bytes).slice(0,20), source: {collection: "event_trace/tree-announce", start: 0, end_exclusive: events.length, total: events.length}, fidelity: "observed physical event edges remain distinct from root and parent semantics"},
      root_wave: {points: wave, final_consensus_root: artifact.samples?.find(item => item.final_root)?.final_root || null, fidelity: "exact recorded arrival lineage with direct causal descendants"},
      sample_count: artifact.samples?.length || 0, event_count: events.length, normalized_plan: artifact.normalized_plan
    };
    analysis._event_fingerprints = events.slice(0, 10000).map(event => `${event.virtual_time_ns}/${event.ordinal}/${event.event_id}/${event.kind}`);
    self.postMessage({analysis});
  } catch (error) {
    self.postMessage({error: error.message});
  }
};
