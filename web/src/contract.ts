/**
 * Resolution-aware analysis query contract (candidate for issue #56).
 *
 * The whole scale story lives here. A run may represent millions or billions of
 * nodes (`fidelity.represented_nodes`), but the UI never receives them all. It
 * asks a {@link DataProvider} for *resolution-bounded* results: at coarse levels
 * the provider returns aggregate cohorts; at the finest level (or inside a
 * sampled-exact region) it returns individual nodes. The renderer's working set
 * is O(budget), independent of run size.
 *
 * The envelope types (manifest / fidelity / provenance) mirror the real M0
 * run-artifact schema (schemas/run-artifact-v1alpha1.schema.json) so a future
 * provider can read genuine artifacts. Time is virtual-clock **nanoseconds**, as
 * the artifact stores it; the UI converts to seconds at its own boundary.
 */

// ---- Envelope (mirrors run-artifact-v1alpha1) --------------------------------

/** The engine ladder's scale rung — the primary regime selector. */
export type Scale = "individual" | "cohort" | "hybrid";
export type BloomMode = "exact-bits" | "sparse-bits" | "occupancy" | "cohort-fpr" | "sampled-exact";

/** A declared approximation with its validated range and uncertainty — the data
 * that lets aggregate views stay honest (never smooth an estimate into "exact"). */
export interface Approximation {
  method: string;
  parameters: Record<string, string>;
  validatedRange: string;
  uncertainty: string;
}

/** An exact-instantiated region embedded inside an aggregate population. */
export interface SampledRegion {
  id: string;
  selection: string;
  nodeCount: number;
}

export interface Fidelity {
  wire: string;
  protocol: string;
  compute: string;
  scale: Scale;
  bloom: BloomMode;
  /** True population — may be far larger than anything ever rendered. */
  representedNodes: number;
  approximations: Approximation[];
  sampledRegions: SampledRegion[];
}

export interface Provenance {
  engineName: string;
  engineVersion: string;
  seed: number;
  fipsCommit: string | null;
}

export interface RunManifest {
  artifactId: string;
  runId: string;
  fidelity: Fidelity;
  provenance: Provenance;
}

// ---- Query surface -----------------------------------------------------------

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A resolution-bounded topology request. */
export interface TopologyQuery {
  /** 0 = coarsest (few super-cohorts) … maxLevel = leaves / exact. */
  level: number;
  /** If set, drill into a sampled-exact region; forces the exact regime. */
  regionId?: string;
  /** Hard cap on returned marks. The provider MUST downsample to honor it. */
  budget: number;
  /** Optional viewport (layout space) for spatial culling. */
  viewport?: Rect;
}

export type Regime = "exact" | "aggregate";

/** A single adopt transition for an individual node. */
export interface Adopt {
  tNs: number;
  root: string;
}

/** One binned sample of a cohort's aggregate state over time. */
export interface CohortSample {
  tNs: number;
  /** Most-represented root in the cohort at this time. */
  dominantRoot: string | null;
  /** Fraction of the cohort accepting `dominantRoot` in [0,1]. */
  adoptedFraction: number;
}

/** An individual node (exact regime). */
export interface ExactNode {
  kind: "exact";
  id: string;
  degree: number;
  depth: number;
  /** Adopt transitions ascending by time; state resolved locally per frame. */
  adopts: Adopt[];
}

/** An aggregate cohort (aggregate regime). */
export interface CohortNode {
  kind: "cohort";
  id: string;
  /** Number of underlying nodes this cohort stands for. */
  population: number;
  /** Binned aggregate state over the run, ascending by time. */
  series: CohortSample[];
  /** Per-root membership share at run end (for the mix indicator). */
  rootMix: Record<string, number>;
  /** Human-readable uncertainty, surfaced so the view can badge it. */
  uncertainty?: string;
}

export type TopoNode = ExactNode | CohortNode;

export type EdgeKind = "parent" | "link" | "flow";

export interface TopoEdge {
  source: string;
  target: string;
  kind: EdgeKind;
  /** For `flow` edges: number of underlying edges aggregated. */
  weight?: number;
}

/** Honest disclosure of what a result does and does not cover. */
export interface Coverage {
  shownNodes: number;
  /** Underlying node population represented by this result. */
  totalNodes: number;
  /** True if the result was truncated to the budget or drawn from a sample. */
  truncated: boolean;
  sampled: boolean;
}

export interface TopologyResult {
  regime: Regime;
  level: number;
  nodes: TopoNode[];
  edges: TopoEdge[];
  coverage: Coverage;
}

export interface NodeDetail {
  id: string;
  kind: Regime;
  fields: Record<string, string>;
}

/**
 * The analysis data source. A synthetic implementation backs the prototype; a
 * real one will read M0 artifacts + out-of-line aggregate/sample blobs.
 */
export interface DataProvider {
  manifest(): RunManifest;
  /** Finest available level (>= 0). At this level the exact regime is used. */
  maxLevel(): number;
  /** Suggested default level for the manifest's scale + budget. */
  defaultLevel(budget: number): number;
  /** Virtual-time extent [startNs, endNs]. */
  timeExtentNs(): [number, number];
  /** Roots in ratchet (first-seen) order. */
  roots(): string[];
  /** Each root's takeover time (first adoption anywhere), for timeline markers. */
  rootTimeline(): { root: string; tNs: number }[];
  /** Resolution-bounded topology query. */
  topology(q: TopologyQuery): TopologyResult;
  /** Detail for a node/cohort id (for the inspector). */
  nodeDetail(id: string, atNs: number): NodeDetail | null;
}
