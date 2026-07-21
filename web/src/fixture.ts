/**
 * Synthetic run generator.
 *
 * Produces an {@link Artifact} that structurally resembles a Root Ratchet run
 * (docs/roadmap.md M1/M7): a preferential-attachment tree plus a handful of
 * extra peer links, over which several "descending roots" successively take
 * over. Each takeover cascades hop-by-hop from the new root outward, which is
 * what the renderer draws as a propagation wavefront (#61).
 *
 * Fully deterministic given a seed, so a given seed always yields the same run —
 * matching the engine's reproducibility contract.
 */

import type { Artifact, EdgeRec, NodeRec, RunEvent, RootId } from "./types.ts";

/** Small, fast, seedable PRNG (mulberry32). */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface FixtureOpts {
  nodeCount?: number;
  seed?: number;
  /** How many successive root takeovers occur over the run. */
  roots?: number;
  /** Seconds of virtual time each hop of a wave takes to propagate. */
  hopDelay?: number;
}

export function generateRun(opts: FixtureOpts = {}): Artifact {
  const nodeCount = opts.nodeCount ?? 320;
  const seed = opts.seed ?? 0xf1ff;
  const rootWaves = opts.roots ?? 4;
  const hopDelay = opts.hopDelay ?? 0.18;
  const rand = rng(seed);

  // --- Topology: preferential attachment tree ---------------------------------
  const nodes: NodeRec[] = [{ id: "n0", depth: 0 }];
  const edges: EdgeRec[] = [];
  const parentOf: number[] = [-1];
  const children: number[][] = [[]];
  // Attachment pool biased by degree so hubs emerge (heavy-tailed, tree-like).
  const pool: number[] = [0];

  for (let i = 1; i < nodeCount; i++) {
    const p = pool[Math.floor(rand() * pool.length)];
    nodes.push({ id: `n${i}`, depth: nodes[p].depth + 1 });
    edges.push({ id: `e${edges.length}`, source: `n${p}`, target: `n${i}`, kind: "parent" });
    parentOf.push(p);
    children.push([]);
    children[p].push(i);
    pool.push(i, p); // both endpoints gain attachment weight
  }

  // A few extra peer links (non-tree) between nearby-depth nodes for realism.
  const extraLinks = Math.floor(nodeCount * 0.12);
  for (let k = 0; k < extraLinks; k++) {
    const a = 1 + Math.floor(rand() * (nodeCount - 1));
    const b = 1 + Math.floor(rand() * (nodeCount - 1));
    if (a === b || parentOf[a] === b || parentOf[b] === a) continue;
    edges.push({ id: `e${edges.length}`, source: `n${a}`, target: `n${b}`, kind: "link" });
  }

  // --- Adjacency for BFS cascades (tree edges only, both directions) -----------
  const adj: number[][] = nodes.map((_, i) => [...children[i]]);
  for (let i = 1; i < nodeCount; i++) adj[i].push(parentOf[i]);

  // --- Root waves: descending roots take over via hop-by-hop cascade ----------
  const events: RunEvent[] = [];
  // Root ids descend (ratchet): lower id wins. Start from the initial root n0
  // then a sequence of ever-"lower" roots seizing control.
  const rootSources: number[] = [0];
  for (let w = 1; w < rootWaves; w++) {
    // choose a deep-ish node as the origin of the next ratchet wave
    let cand = 1 + Math.floor(rand() * (nodeCount - 1));
    for (let tries = 0; tries < 6; tries++) {
      const c2 = 1 + Math.floor(rand() * (nodeCount - 1));
      if (nodes[c2].depth > nodes[cand].depth) cand = c2;
    }
    rootSources.push(cand);
  }

  let t = 0;
  const quietGap = 1.2; // settle time between waves
  for (let w = 0; w < rootSources.length; w++) {
    const origin = rootSources[w];
    const root: RootId = `n${origin}`;
    // BFS from origin; adoption time grows with hop distance + small jitter.
    const dist = new Array(nodeCount).fill(-1);
    dist[origin] = 0;
    const queue = [origin];
    const order: number[] = [];
    for (let qi = 0; qi < queue.length; qi++) {
      const u = queue[qi];
      order.push(u);
      for (const v of adj[u]) {
        if (dist[v] === -1) {
          dist[v] = dist[u] + 1;
          queue.push(v);
        }
      }
    }
    for (const u of order) {
      const jitter = rand() * hopDelay * 0.6;
      const at = t + dist[u] * hopDelay + jitter;
      events.push({ t: at, kind: "root_adopt", node: `n${u}`, root, bytes: 40 + Math.floor(rand() * 24) });
      // Occasional TreeAnnounce as a node commits its new parent toward the root.
      if (rand() < 0.5) {
        events.push({ t: at + 0.02, kind: "tree_announce", node: `n${u}`, bytes: 88 + Math.floor(rand() * 40) });
      }
    }
    const maxHop = Math.max(...dist);
    t += maxHop * hopDelay + quietGap;
  }

  // Everyone goes quiescent shortly after the last wave settles.
  for (let i = 0; i < nodeCount; i++) {
    events.push({ t: t + rand() * 0.4, kind: "quiescent", node: `n${i}` });
  }
  const duration = t + 0.6;

  events.sort((a, b) => a.t - b.t);

  return {
    manifest: {
      id: "synthetic-root-ratchet",
      campaign: "root-ratchet",
      protocol: "fips-v1alpha1 (synthetic)",
      seed,
      createdAt: "1970-01-01T00:00:00Z",
      nodeCount,
      duration,
      notes: {
        source: "synthetic fixture",
        waves: String(rootWaves),
        edges: String(edges.length),
      },
    },
    nodes,
    edges,
    events,
  };
}
