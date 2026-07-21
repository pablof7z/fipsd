/**
 * Analysis-UI data model.
 *
 * This is the *read side* of the FIPS Wind Tunnel artifact contract. It is
 * intentionally a thin, stable projection of what an immutable run artifact is
 * expected to expose (see docs/architecture.md — "the artifact is the boundary
 * between execution and analysis"). When the M0 artifact/query layer (#56) lands,
 * a loader maps real artifacts onto these types and the renderer is unchanged.
 *
 * Everything here is plain data: no engine semantics, no mutation. Time is
 * virtual-clock seconds from the run's t0.
 */

/** Stable identifier for a node in the simulated network. */
export type NodeId = string;

/** Identifier of a root (a node id that is acting as a spanning-tree root). */
export type RootId = string;

/** Top-level run descriptor — mirrors the artifact manifest / provenance envelope. */
export interface RunManifest {
  id: string;
  campaign: string;
  protocol: string;
  seed: number;
  createdAt: string;
  nodeCount: number;
  /** Virtual-clock duration of the run, in seconds. */
  duration: number;
  /** Free-form, provenance-style key/values shown in the run meta strip. */
  notes?: Record<string, string>;
}

/** A node as it exists for the whole run (topology is fixed; state is event-driven). */
export interface NodeRec {
  id: NodeId;
  label?: string;
  /** Hop distance from the initial/primary root at t0 (for initial layout hints). */
  depth: number;
}

export type EdgeKind = "parent" | "link";

/** An edge in the network. `parent` edges form the active spanning tree; `link`
 * edges are physical/peer links that exist but may not be tree edges. */
export interface EdgeRec {
  id: string;
  source: NodeId;
  target: NodeId;
  kind: EdgeKind;
}

/** The kinds of timed events the timeline replays. Extend as the engine grows. */
export type EventKind =
  | "root_adopt" // node switched its accepted root
  | "tree_announce" // node emitted a TreeAnnounce
  | "parent_change" // node re-parented within the tree
  | "quiescent"; // node reached steady state

/** A single timed event on the virtual clock. */
export interface RunEvent {
  /** Virtual-clock time in seconds. */
  t: number;
  kind: EventKind;
  node: NodeId;
  /** For root_adopt: the root now accepted by `node`. */
  root?: RootId;
  /** For parent_change: the new parent node. */
  parent?: NodeId;
  /** Optional exact byte cost attributed to this event (accounting). */
  bytes?: number;
}

/** The complete artifact projection consumed by the UI. */
export interface Artifact {
  manifest: RunManifest;
  nodes: NodeRec[];
  edges: EdgeRec[];
  /** Events sorted ascending by `t`. */
  events: RunEvent[];
}
