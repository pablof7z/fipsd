/**
 * Synthetic {@link DataProvider}.
 *
 * Wraps a generated run (fixture.ts) and serves it at multiple resolutions:
 *
 * - **exact regime** (finest level): individual nodes + their adopt timelines —
 *   the #59 view.
 * - **aggregate regime** (coarser levels): nodes are partitioned into cohorts by
 *   a cut through the spanning tree; each cohort carries a binned time series of
 *   its dominant root and adopted fraction — the #60 view.
 *
 * This is the seam the real provider will replace: same interface, real M0
 * artifact + aggregate/sample blobs behind it.
 */

import type {
  Adopt,
  CohortNode,
  CohortSample,
  Coverage,
  DataProvider,
  ExactNode,
  NodeDetail,
  RunManifest,
  TopoEdge,
  TopologyQuery,
  TopologyResult,
} from "../contract.ts";
import type { Artifact } from "../types.ts";

const SEC = 1_000_000_000; // ns per second
const BINS = 96; // time-series bins per cohort

interface RawNode {
  id: string;
  depth: number;
  degree: number;
  parent: number; // index, -1 for root
  adopts: Adopt[];
}

export class SyntheticProvider implements DataProvider {
  private art: Artifact;
  private raw: RawNode[];
  private idToIdx = new Map<string, number>();
  private parentEdges: [number, number][] = [];
  private linkEdges: [number, number][] = [];
  private rootOrder: string[] = [];
  private durationNs: number;
  /** Tree cut-depth for each aggregate level, coarse→fine; the final level is
   * the exact regime (one cohort per node). Chosen so cohort counts grow ~1.8×
   * per level for a smooth LOD slider. */
  private levelDepths: number[] = [];

  constructor(art: Artifact) {
    this.art = art;
    this.durationNs = Math.round(art.manifest.duration * SEC);

    this.raw = art.nodes.map((n, i) => {
      this.idToIdx.set(n.id, i);
      return { id: n.id, depth: n.depth, degree: 0, parent: -1, adopts: [] };
    });

    for (const e of art.edges) {
      const s = this.idToIdx.get(e.source)!;
      const t = this.idToIdx.get(e.target)!;
      this.raw[s].degree++;
      this.raw[t].degree++;
      if (e.kind === "parent") {
        this.parentEdges.push([s, t]);
        this.raw[t].parent = s; // fixture emits parent -> child
      } else {
        this.linkEdges.push([s, t]);
      }
    }

    const seen = new Set<string>();
    for (const ev of art.events) {
      if (ev.kind === "root_adopt" && ev.root) {
        this.raw[this.idToIdx.get(ev.node)!].adopts.push({ tNs: Math.round(ev.t * SEC), root: ev.root });
        if (!seen.has(ev.root)) {
          seen.add(ev.root);
          this.rootOrder.push(ev.root);
        }
      }
    }
    for (const r of this.raw) r.adopts.sort((a, b) => a.tNs - b.tNs);

    // Build aggregate levels with roughly geometric cohort counts, so the LOD
    // slider steps feel even. The final level is the exact regime.
    const maxDepth = Math.max(...this.raw.map((r) => r.depth));
    let target = 12;
    let lastCount = 0;
    for (let d = 0; d <= maxDepth; d++) {
      const count = this.cohortCountAtDepth(d);
      if (count >= target && count !== lastCount) {
        this.levelDepths.push(d);
        lastCount = count;
        target = Math.ceil(target * 1.8);
      }
    }
    if (!this.levelDepths.length) this.levelDepths.push(maxDepth);
  }

  manifest(): RunManifest {
    const m = this.art.manifest;
    return {
      artifactId: m.id,
      runId: `${m.campaign}-seed-${m.seed}`,
      fidelity: {
        wire: "modeled",
        protocol: "semantic-exact",
        compute: "operation-counted",
        scale: "individual",
        bloom: "exact-bits",
        representedNodes: m.nodeCount,
        approximations: [],
        sampledRegions: [],
      },
      provenance: {
        engineName: "synthetic",
        engineVersion: "0.1.0",
        seed: m.seed,
        fipsCommit: null,
      },
    };
  }

  maxLevel(): number {
    // aggregate levels + one final exact level
    return this.levelDepths.length;
  }

  defaultLevel(budget: number): number {
    // Render exact when the whole run fits the budget; otherwise open on a
    // meaningfully-aggregated overview (~300 cohorts) rather than the finest
    // level that merely fits — so the default reads as structure, not confetti.
    if (this.raw.length <= budget) return this.maxLevel();
    const target = Math.min(300, budget);
    let bestL = 0;
    let bestDelta = Infinity;
    for (let l = 0; l < this.maxLevel(); l++) {
      const count = this.cohortCountAtDepth(this.levelDepths[l]);
      if (count > budget) break;
      const delta = Math.abs(count - target);
      if (delta < bestDelta) {
        bestDelta = delta;
        bestL = l;
      }
    }
    return bestL;
  }

  timeExtentNs(): [number, number] {
    return [0, this.durationNs];
  }

  roots(): string[] {
    return [...this.rootOrder];
  }

  rootTimeline(): { root: string; tNs: number }[] {
    const first = new Map<string, number>();
    for (const r of this.raw)
      for (const a of r.adopts) {
        const prev = first.get(a.root);
        if (prev === undefined || a.tNs < prev) first.set(a.root, a.tNs);
      }
    return this.rootOrder.map((root) => ({ root, tNs: first.get(root) ?? 0 }));
  }

  // --- Aggregation helpers ----------------------------------------------------

  /** Cohort id of node `i` for a tree cut at `cutDepth`: its ancestor at that
   * depth (or the node itself if shallower). Deterministic, subtree-based. */
  private cohortAtDepth(i: number, cutDepth: number): number {
    let cur = i;
    while (this.raw[cur].depth > cutDepth && this.raw[cur].parent >= 0) cur = this.raw[cur].parent;
    return cur;
  }

  private cohortCountAtDepth(cutDepth: number): number {
    const s = new Set<number>();
    for (let i = 0; i < this.raw.length; i++) s.add(this.cohortAtDepth(i, cutDepth));
    return s.size;
  }

  /** Current root of node `i` at time `tNs` (last adopt <= t). */
  private rootAt(i: number, tNs: number): string | null {
    const a = this.raw[i].adopts;
    let lo = 0,
      hi = a.length - 1,
      idx = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (a[mid].tNs <= tNs) {
        idx = mid;
        lo = mid + 1;
      } else hi = mid - 1;
    }
    return idx < 0 ? null : a[idx].root;
  }

  // --- Topology query ---------------------------------------------------------

  topology(q: TopologyQuery): TopologyResult {
    const exact = q.level >= this.maxLevel();
    return exact ? this.exactResult(q) : this.aggregateResult(q);
  }

  private exactResult(q: TopologyQuery): TopologyResult {
    // Budget-bounded: if over budget, keep the highest-degree nodes (hubs first).
    let order = this.raw.map((_, i) => i);
    const truncated = order.length > q.budget;
    if (truncated) {
      order = order.sort((a, b) => this.raw[b].degree - this.raw[a].degree).slice(0, q.budget);
    }
    const keep = new Set(order);
    const nodes: ExactNode[] = order.map((i) => ({
      kind: "exact",
      id: this.raw[i].id,
      degree: this.raw[i].degree,
      depth: this.raw[i].depth,
      adopts: this.raw[i].adopts,
    }));
    const edges: TopoEdge[] = [];
    for (const [s, t] of this.parentEdges)
      if (keep.has(s) && keep.has(t)) edges.push({ source: this.raw[s].id, target: this.raw[t].id, kind: "parent" });
    for (const [s, t] of this.linkEdges)
      if (keep.has(s) && keep.has(t)) edges.push({ source: this.raw[s].id, target: this.raw[t].id, kind: "link" });

    const coverage: Coverage = {
      shownNodes: nodes.length,
      totalNodes: this.raw.length,
      truncated,
      sampled: false,
    };
    return { regime: "exact", level: q.level, nodes, edges, coverage };
  }

  private aggregateResult(q: TopologyQuery): TopologyResult {
    const level = q.level;
    const cutDepth = this.levelDepths[level];
    // members[cohortIdx] = node indices
    const members = new Map<number, number[]>();
    for (let i = 0; i < this.raw.length; i++) {
      const c = this.cohortAtDepth(i, cutDepth);
      (members.get(c) ?? members.set(c, []).get(c)!).push(i);
    }

    const nodes: CohortNode[] = [];
    for (const [c, idxs] of members) {
      const series: CohortSample[] = [];
      for (let b = 0; b < BINS; b++) {
        const tNs = Math.round((b / (BINS - 1)) * this.durationNs);
        const counts = new Map<string, number>();
        for (const i of idxs) {
          const r = this.rootAt(i, tNs);
          if (r) counts.set(r, (counts.get(r) ?? 0) + 1);
        }
        let dominant: string | null = null;
        let best = 0;
        for (const [r, n] of counts)
          if (n > best) {
            best = n;
            dominant = r;
          }
        series.push({ tNs, dominantRoot: dominant, adoptedFraction: idxs.length ? best / idxs.length : 0 });
      }
      // Final root mix.
      const finalCounts = new Map<string, number>();
      for (const i of idxs) {
        const r = this.rootAt(i, this.durationNs);
        if (r) finalCounts.set(r, (finalCounts.get(r) ?? 0) + 1);
      }
      const rootMix: Record<string, number> = {};
      for (const [r, n] of finalCounts) rootMix[r] = n / idxs.length;

      nodes.push({
        kind: "cohort",
        id: `c${level}:${this.raw[c].id}`,
        population: idxs.length,
        series,
        rootMix,
        uncertainty: idxs.length > 1 ? `aggregate of ${idxs.length} nodes` : undefined,
      });
    }

    // Flow edges between cohorts (aggregate tree + link edges).
    const flow = new Map<string, TopoEdge>();
    const addFlow = (s: number, t: number) => {
      const cs = this.cohortAtDepth(s, cutDepth);
      const ct = this.cohortAtDepth(t, cutDepth);
      if (cs === ct) return;
      const a = `c${level}:${this.raw[cs].id}`;
      const b = `c${level}:${this.raw[ct].id}`;
      const key = a < b ? `${a}|${b}` : `${b}|${a}`;
      const ex = flow.get(key);
      if (ex) ex.weight = (ex.weight ?? 1) + 1;
      else flow.set(key, { source: a, target: b, kind: "flow", weight: 1 });
    };
    for (const [s, t] of this.parentEdges) addFlow(s, t);
    for (const [s, t] of this.linkEdges) addFlow(s, t);

    const coverage: Coverage = {
      shownNodes: nodes.length,
      totalNodes: this.raw.length,
      truncated: false,
      sampled: false,
    };
    return { regime: "aggregate", level, nodes, edges: [...flow.values()], coverage };
  }

  nodeDetail(id: string, atNs: number): NodeDetail | null {
    if (id.startsWith("c")) {
      // cohort id form: c<level>:<rawId>
      const rawId = id.split(":")[1];
      const c = this.idToIdx.get(rawId);
      if (c === undefined) return null;
      const level = parseInt(id.slice(1), 10);
      const cutDepth = this.levelDepths[level] ?? 0;
      const idxs: number[] = [];
      for (let i = 0; i < this.raw.length; i++) if (this.cohortAtDepth(i, cutDepth) === c) idxs.push(i);
      const counts = new Map<string, number>();
      for (const i of idxs) {
        const r = this.rootAt(i, atNs);
        if (r) counts.set(r, (counts.get(r) ?? 0) + 1);
      }
      let dom: string | null = null,
        best = 0;
      for (const [r, n] of counts)
        if (n > best) {
          best = n;
          dom = r;
        }
      return {
        id,
        kind: "aggregate",
        fields: {
          population: String(idxs.length),
          "dominant root": dom ?? "—",
          "adopted %": idxs.length ? ((best / idxs.length) * 100).toFixed(0) + "%" : "0%",
          "roots present": String(counts.size),
        },
      };
    }
    const i = this.idToIdx.get(id);
    if (i === undefined) return null;
    return {
      id,
      kind: "exact",
      fields: {
        degree: String(this.raw[i].degree),
        depth: String(this.raw[i].depth),
        root: this.rootAt(i, atNs) ?? "—",
        adopts: String(this.raw[i].adopts.length),
      },
    };
  }
}
