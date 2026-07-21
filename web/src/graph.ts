/**
 * Topology view: force-directed layout (d3-force) rendered to Canvas 2D, driven
 * by the virtual clock, and **resolution-aware**.
 *
 * It renders a {@link TopologyResult} — a bounded projection from the data
 * provider — which may be either individual nodes (exact regime, #59) or
 * aggregate cohorts (aggregate regime, #60). The same shell therefore shows a
 * ten-node run and a million-node run; only the projection differs. Layout
 * positions are preserved by id across level switches so LOD changes feel like
 * zooming, not reloading.
 *
 * Motion is data-driven: a custom "root pull" force nudges exact nodes toward
 * their current root, so ratchet takeovers visibly reorganize the network. In
 * the aggregate regime the takeover shows as an adoption front — cohorts recolor
 * and fill hop-by-hop as their dominant root flips (issue #61).
 */

import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationNodeDatum,
} from "d3-force";
import type { TopoEdge, TopoNode, TopologyResult } from "./contract.ts";
import { RootPalette, rgbStr } from "./palette.ts";

const SEC = 1_000_000_000;
const PULSE_WINDOW = 0.9; // seconds a freshly-adopted node/cohort keeps pulsing

interface VNode extends SimulationNodeDatum {
  id: string;
  kind: "exact" | "cohort";
  degree: number;
  population: number;
  /** Level-independent key for layout continuity across LOD switches. */
  posKey: string;
  /** Frame this node first appeared (for fade-in); -Infinity if carried over. */
  appearFrame: number;
  // exact:
  adopts?: { t: number; root: string }[];
  // cohort:
  series?: { t: number; dominantRoot: string | null; adoptedFraction: number }[];
}

/** A fading remnant of a node that vanished on the last LOD change. */
interface Ghost {
  x: number;
  y: number;
  r: number;
  color: string;
  bornFrame: number;
}

interface VLink {
  source: VNode;
  target: VNode;
  kind: TopoEdge["kind"];
  weight: number;
}

interface Transform {
  k: number;
  x: number;
  y: number;
}

interface ResolvedState {
  root: string | null;
  settle: number; // 0 = fresh flip, 1 = settled
  fraction: number; // exact: 1; cohort: adoptedFraction
}

export class TopologyView {
  readonly palette = new RootPalette();
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private nodes: VNode[] = [];
  private links: VLink[] = [];
  private byId = new Map<string, VNode>();
  private posMemo = new Map<string, { x: number; y: number }>();
  private sim: Simulation<VNode, undefined>;
  private regime: "exact" | "aggregate" = "exact";
  private frameNo = 0;
  private ghosts: Ghost[] = [];
  private static readonly FADE = 26; // frames for LOD fade in/out

  private t = 0;
  private playing = false;
  private width = 0;
  private height = 0;
  private dpr = Math.min(window.devicePixelRatio || 1, 2);
  private transform: Transform = { k: 1, x: 0, y: 0 };

  private showLabels = false;
  private showLinks = true;
  private hover: VNode | null = null;
  private selected: VNode | null = null;
  private focusRoot: string | null = null;

  onSelect: ((id: string | null) => void) | null = null;
  onBackgroundClick: (() => void) | null = null;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d")!;
    this.sim = forceSimulation<VNode>([])
      .force("charge", forceManyBody<VNode>().strength((d) => -18 - Math.sqrt(d.population) * 6))
      .force("center", forceCenter(0, 0).strength(0.04))
      .force("collide", forceCollide<VNode>((d) => this.radius(d) + 1.5))
      .force("rootPull", this.rootPullForce(0.05))
      .alphaDecay(0.015)
      .stop();
    this.bindInteraction();
    this.resize();
  }

  /** Register palette hues in ratchet order (so legend + colors are stable). */
  registerRoots(roots: string[]) {
    for (const r of roots) this.palette.hueFor(r);
  }

  /** Level-independent layout key: strips the level from cohort ids
   * (`c<level>:<raw>` → `c:<raw>`) so a subtree region keeps its position as the
   * resolution changes; exact ids are already stable. */
  private static posKeyOf(id: string): string {
    return id.startsWith("c") && id.includes(":") ? "c:" + id.slice(id.indexOf(":") + 1) : id;
  }

  /** Load a new resolution projection, animating the transition:
   * carried-over regions keep their position, vanished ones fade out as ghosts,
   * and newly-split ones fade in seeded from their neighbours. */
  setData(result: TopologyResult) {
    const prevByPosKey = new Map<string, VNode>();
    for (const n of this.nodes) {
      if (n.x != null && n.y != null) this.posMemo.set(n.posKey, { x: n.x, y: n.y });
      prevByPosKey.set(n.posKey, n);
    }

    this.nodes = result.nodes.map((r) => this.toVNode(r));
    this.byId = new Map(this.nodes.map((n) => [n.id, n]));
    const newKeys = new Set(this.nodes.map((n) => n.posKey));

    // Ghost the nodes that disappeared this switch (fade-out).
    for (const [key, prev] of prevByPosKey) {
      if (!newKeys.has(key) && prev.x != null && prev.y != null) {
        const st = this.stateOf(prev);
        this.ghosts.push({
          x: prev.x,
          y: prev.y,
          r: this.radius(prev),
          color: rgbStr(st.root ? this.palette.rootColor(st.root) : { r: 90, g: 100, b: 116 }),
          bornFrame: this.frameNo,
        });
      }
    }

    this.links = [];
    for (const e of result.edges) {
      const s = this.byId.get(e.source);
      const t = this.byId.get(e.target);
      if (s && t) this.links.push({ source: s, target: t, kind: e.kind, weight: e.weight ?? 1 });
    }

    // Seed genuinely-new nodes from the centroid of their memo'd neighbours so
    // they emerge from where their region already is, not from random space.
    const neighborSum = new Map<string, { x: number; y: number; n: number }>();
    for (const l of this.links) {
      for (const [a, b] of [[l.source, l.target], [l.target, l.source]] as const) {
        if (this.posMemo.has(a.posKey) && !this.posMemo.has(b.posKey)) {
          const acc = neighborSum.get(b.id) ?? { x: 0, y: 0, n: 0 };
          acc.x += a.x!;
          acc.y += a.y!;
          acc.n++;
          neighborSum.set(b.id, acc);
        }
      }
    }
    for (const n of this.nodes) {
      if (this.posMemo.has(n.posKey)) continue;
      const acc = neighborSum.get(n.id);
      if (acc && acc.n > 0) {
        n.x = acc.x / acc.n + (Math.random() - 0.5) * 12;
        n.y = acc.y / acc.n + (Math.random() - 0.5) * 12;
      }
    }

    this.regime = result.regime;
    const linkForce = forceLink<VNode, VLink>(this.links.filter((l) => l.kind !== "link" || this.showLinks))
      .id((d) => d.id)
      .distance((l) => (l.kind === "flow" ? 40 : 18))
      .strength((l) => (l.kind === "flow" ? 0.25 : 0.5));

    this.sim.nodes(this.nodes);
    this.sim.force("link", linkForce);
    (this.sim.force("collide") as ReturnType<typeof forceCollide<VNode>>).radius((d) => this.radius(d) + 1.5);
    this.sim.alpha(0.9).restart();
    if (!this.playing) this.sim.alphaTarget(0);

    if (this.selected && !this.byId.has(this.selected.id)) {
      this.selected = null;
      this.onSelect?.(null);
    }
  }

  private toVNode(r: TopoNode): VNode {
    const posKey = TopologyView.posKeyOf(r.id);
    const memo = this.posMemo.get(posKey);
    const base: VNode = {
      id: r.id,
      kind: r.kind,
      posKey,
      appearFrame: memo ? -Infinity : this.frameNo,
      degree: r.kind === "exact" ? r.degree : 0,
      population: r.kind === "cohort" ? r.population : 1,
      x: memo?.x ?? (Math.random() - 0.5) * 200,
      y: memo?.y ?? (Math.random() - 0.5) * 200,
    };
    if (r.kind === "exact") {
      base.adopts = r.adopts.map((a) => ({ t: a.tNs / SEC, root: a.root }));
    } else {
      base.series = r.series.map((s) => ({ t: s.tNs / SEC, dominantRoot: s.dominantRoot, adoptedFraction: s.adoptedFraction }));
    }
    return base;
  }

  // --- Public control ---------------------------------------------------------

  setTime(t: number) {
    this.t = t;
  }
  time() {
    return this.t;
  }
  setPlaying(p: boolean) {
    this.playing = p;
    this.sim.alphaTarget(p ? 0.08 : 0);
    if (p) this.sim.alpha(Math.max(this.sim.alpha(), 0.2));
  }
  setShowLabels(b: boolean) {
    this.showLabels = b;
  }
  setShowLinks(b: boolean) {
    this.showLinks = b;
  }
  reheat() {
    this.sim.alpha(0.9).alphaTarget(this.playing ? 0.08 : 0);
  }
  fit() {
    if (!this.nodes.length) return;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const n of this.nodes) {
      minX = Math.min(minX, n.x!); minY = Math.min(minY, n.y!);
      maxX = Math.max(maxX, n.x!); maxY = Math.max(maxY, n.y!);
    }
    // Fit into the area not covered by the overlay panels (legend / inspector).
    const inset = { left: 210, right: 250, top: 28, bottom: 44 };
    const availW = Math.max(80, this.width - inset.left - inset.right);
    const availH = Math.max(80, this.height - inset.top - inset.bottom);
    const cx = inset.left + availW / 2;
    const cy = inset.top + availH / 2;
    const k = Math.min(6, Math.max(0.15, Math.min(availW / (maxX - minX || 1), availH / (maxY - minY || 1))));
    this.transform.k = k;
    this.transform.x = cx - ((minX + maxX) / 2) * k;
    this.transform.y = cy - ((minY + maxY) / 2) * k;
  }

  resize() {
    const rect = this.canvas.getBoundingClientRect();
    this.width = rect.width;
    this.height = rect.height;
    this.canvas.width = Math.floor(rect.width * this.dpr);
    this.canvas.height = Math.floor(rect.height * this.dpr);
    if (this.transform.x === 0 && this.transform.y === 0) {
      this.transform.x = this.width / 2;
      this.transform.y = this.height / 2;
    }
  }

  frame() {
    this.frameNo++;
    if (this.playing || this.sim.alpha() > this.sim.alphaMin()) this.sim.tick();
    this.render();
    if (this.minCtx) this.renderMinimap();
  }

  // --- Minimap ----------------------------------------------------------------

  private minCtx: CanvasRenderingContext2D | null = null;
  private minW = 0;
  private minH = 0;
  /** Cached world→minimap mapping from the last minimap render, for hit-testing. */
  private minMap: { s: number; ox: number; oy: number } | null = null;

  attachMinimap(canvas: HTMLCanvasElement) {
    this.minCtx = canvas.getContext("2d")!;
    const rect = canvas.getBoundingClientRect();
    this.minW = rect.width;
    this.minH = rect.height;
    canvas.width = Math.floor(rect.width * this.dpr);
    canvas.height = Math.floor(rect.height * this.dpr);

    const recenter = (e: MouseEvent) => {
      if (!this.minMap) return;
      const r = canvas.getBoundingClientRect();
      const wx = (e.clientX - r.left - this.minMap.ox) / this.minMap.s;
      const wy = (e.clientY - r.top - this.minMap.oy) / this.minMap.s;
      this.transform.x = this.width / 2 - wx * this.transform.k;
      this.transform.y = this.height / 2 - wy * this.transform.k;
    };
    let down = false;
    canvas.addEventListener("mousedown", (e) => { down = true; recenter(e); });
    canvas.addEventListener("mousemove", (e) => { if (down) recenter(e); });
    window.addEventListener("mouseup", () => { down = false; });
  }

  private renderMinimap() {
    const ctx = this.minCtx!;
    ctx.save();
    ctx.scale(this.dpr, this.dpr);
    ctx.clearRect(0, 0, this.minW, this.minH);
    if (this.nodes.length) {
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const n of this.nodes) {
        minX = Math.min(minX, n.x!); minY = Math.min(minY, n.y!);
        maxX = Math.max(maxX, n.x!); maxY = Math.max(maxY, n.y!);
      }
      const pad = 8;
      const s = Math.min((this.minW - pad * 2) / (maxX - minX || 1), (this.minH - pad * 2) / (maxY - minY || 1));
      const ox = pad - minX * s + (this.minW - pad * 2 - (maxX - minX) * s) / 2;
      const oy = pad - minY * s + (this.minH - pad * 2 - (maxY - minY) * s) / 2;
      this.minMap = { s, ox, oy };

      for (const n of this.nodes) {
        const st = this.stateOf(n);
        ctx.fillStyle = rgbStr(st.root ? this.palette.rootColor(st.root) : { r: 90, g: 100, b: 116 }, 0.8);
        ctx.fillRect(n.x! * s + ox - 0.7, n.y! * s + oy - 0.7, 1.4, 1.4);
      }

      // Current viewport rectangle.
      const [wx0, wy0] = this.toWorld(0, 0);
      const [wx1, wy1] = this.toWorld(this.width, this.height);
      ctx.strokeStyle = "rgba(255,255,255,0.65)";
      ctx.lineWidth = 1;
      ctx.strokeRect(wx0 * s + ox, wy0 * s + oy, (wx1 - wx0) * s, (wy1 - wy0) * s);
    }
    ctx.restore();
  }

  // --- State resolution -------------------------------------------------------

  private stateOf(n: VNode): ResolvedState {
    if (n.kind === "exact") {
      const a = n.adopts!;
      let lo = 0, hi = a.length - 1, idx = -1;
      while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        if (a[mid].t <= this.t) { idx = mid; lo = mid + 1; } else hi = mid - 1;
      }
      if (idx < 0) return { root: null, settle: 1, fraction: 1 };
      return { root: a[idx].root, settle: Math.min(1, (this.t - a[idx].t) / PULSE_WINDOW), fraction: 1 };
    }
    // cohort: step the binned series and measure time since last dominant flip.
    const s = n.series!;
    let idx = 0;
    for (let i = 0; i < s.length; i++) { if (s[i].t <= this.t) idx = i; else break; }
    const cur = s[idx];
    // find last flip
    let flipT = s[0].t;
    for (let i = idx; i > 0; i--) {
      if (s[i].dominantRoot !== s[i - 1].dominantRoot) { flipT = s[i].t; break; }
    }
    return {
      root: cur.dominantRoot,
      settle: Math.min(1, (this.t - flipT) / PULSE_WINDOW),
      fraction: cur.adoptedFraction,
    };
  }

  // --- Root-pull force (exact regime only) ------------------------------------

  private rootPullForce(strength: number) {
    let nodes: VNode[] = [];
    const force = (alpha: number) => {
      if (this.regime !== "exact") return;
      for (const n of nodes) {
        const st = this.stateOf(n);
        if (!st.root) continue;
        const root = this.byId.get(st.root);
        if (!root || root === n) continue;
        const k = strength * (1 + (1 - st.settle) * 1.5) * alpha;
        n.vx! += (root.x! - n.x!) * k;
        n.vy! += (root.y! - n.y!) * k;
      }
    };
    force.initialize = (n: VNode[]) => { nodes = n; };
    return force;
  }

  // --- Rendering --------------------------------------------------------------

  private radius(n: VNode): number {
    return n.kind === "exact" ? 2.6 + Math.sqrt(n.degree) * 1.3 : 5 + Math.sqrt(n.population) * 1.7;
  }

  private toWorld(sx: number, sy: number): [number, number] {
    return [(sx - this.transform.x) / this.transform.k, (sy - this.transform.y) / this.transform.k];
  }

  private render() {
    const ctx = this.ctx;
    ctx.save();
    ctx.scale(this.dpr, this.dpr);
    ctx.clearRect(0, 0, this.width, this.height);
    const { k, x: tx, y: ty } = this.transform;
    ctx.translate(tx, ty);
    ctx.scale(k, k);

    // Edges
    for (const l of this.links) {
      if (l.kind === "link" && !this.showLinks) continue;
      if (l.kind === "flow") {
        ctx.strokeStyle = "rgba(120,150,190,0.22)";
        ctx.lineWidth = Math.min(4, 0.5 + Math.log2(1 + l.weight)) / k;
      } else {
        ctx.strokeStyle = l.kind === "parent" ? "rgba(120,140,170,0.28)" : "rgba(90,110,140,0.12)";
        ctx.lineWidth = 0.6 / k;
      }
      ctx.beginPath();
      ctx.moveTo(l.source.x!, l.source.y!);
      ctx.lineTo(l.target.x!, l.target.y!);
      ctx.stroke();
    }

    // Fading ghosts of nodes that vanished on the last LOD change.
    this.ghosts = this.ghosts.filter((g) => this.frameNo - g.bornFrame < TopologyView.FADE);
    for (const g of this.ghosts) {
      const a = 1 - (this.frameNo - g.bornFrame) / TopologyView.FADE;
      ctx.beginPath();
      ctx.arc(g.x, g.y, g.r * (0.6 + 0.4 * a), 0, Math.PI * 2);
      ctx.fillStyle = g.color.replace("rgb(", "rgba(").replace(")", `,${(a * 0.5).toFixed(3)})`);
      ctx.fill();
    }

    for (const n of this.nodes) {
      const st = this.stateOf(n);
      const r = this.radius(n);
      const appear = Math.min(1, (this.frameNo - n.appearFrame) / TopologyView.FADE);
      const dim = (this.focusRoot && st.root !== this.focusRoot ? 0.1 : 1) * appear;

      // Wavefront pulse on fresh adopt / dominant flip.
      if (st.root && st.settle < 1) {
        const p = st.settle;
        ctx.beginPath();
        ctx.arc(n.x!, n.y!, r + p * 16, 0, Math.PI * 2);
        ctx.strokeStyle = rgbStr(this.palette.rootColor(st.root), (1 - p) * 0.7 * dim);
        ctx.lineWidth = (1.7 * (1 - p)) / k + 0.2;
        ctx.stroke();
      }

      if (n.kind === "exact") {
        ctx.beginPath();
        ctx.arc(n.x!, n.y!, r, 0, Math.PI * 2);
        ctx.fillStyle = rgbStr(this.palette.nodeColor(st.root, st.settle), dim);
        ctx.fill();
        if (st.root === n.id) {
          ctx.beginPath();
          ctx.arc(n.x!, n.y!, r + 2.5, 0, Math.PI * 2);
          ctx.strokeStyle = rgbStr(this.palette.rootColor(n.id), 0.9 * dim);
          ctx.lineWidth = 1.4 / k;
          ctx.stroke();
        }
      } else {
        // Cohort: outer ghost ring = full population; inner disc = adopted fraction.
        const col = st.root ? this.palette.rootColor(st.root) : { r: 90, g: 100, b: 116 };
        ctx.beginPath();
        ctx.arc(n.x!, n.y!, r, 0, Math.PI * 2);
        ctx.fillStyle = rgbStr(col, 0.14 * dim);
        ctx.fill();
        ctx.strokeStyle = rgbStr(col, 0.5 * dim);
        ctx.lineWidth = 1 / k;
        ctx.stroke();
        // adopted fraction as a filled inner disc (area ∝ fraction)
        const rf = r * Math.sqrt(Math.max(0, Math.min(1, st.fraction)));
        if (rf > 0.4) {
          ctx.beginPath();
          ctx.arc(n.x!, n.y!, rf, 0, Math.PI * 2);
          ctx.fillStyle = rgbStr(col, 0.85 * dim);
          ctx.fill();
        }
      }
    }

    for (const h of [this.selected, this.hover]) {
      if (!h) continue;
      ctx.beginPath();
      ctx.arc(h.x!, h.y!, this.radius(h) + 4, 0, Math.PI * 2);
      ctx.strokeStyle = h === this.selected ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.5)";
      ctx.lineWidth = 1.5 / k;
      ctx.stroke();
    }

    if (this.showLabels) {
      ctx.fillStyle = "rgba(226,232,240,0.85)";
      ctx.font = `${10 / k}px ui-monospace, monospace`;
      ctx.textAlign = "center";
      for (const n of this.nodes) {
        if (n.kind === "exact" && n.degree < 4 && k < 1.6) continue;
        const label = n.kind === "cohort" ? `${n.population}` : n.id;
        ctx.fillText(label, n.x!, n.y! - this.radius(n) - 3 / k);
      }
    }

    ctx.restore();
  }

  // --- Interaction ------------------------------------------------------------

  private nodeAt(sx: number, sy: number): VNode | null {
    const [wx, wy] = this.toWorld(sx, sy);
    let best: VNode | null = null, bestD = Infinity;
    for (const n of this.nodes) {
      const dx = n.x! - wx, dy = n.y! - wy, d = dx * dx + dy * dy;
      const rr = (this.radius(n) + 4) ** 2;
      if (d < rr && d < bestD) { bestD = d; best = n; }
    }
    return best;
  }

  selectedId(): string | null {
    return this.selected?.id ?? null;
  }
  hoverId(): string | null {
    return this.hover?.id ?? null;
  }
  setFocusRoot(root: string | null) {
    this.focusRoot = root;
  }

  private bindInteraction() {
    const c = this.canvas;
    let dragNode: VNode | null = null;
    let panning = false;
    let last: [number, number] = [0, 0];
    const pos = (e: MouseEvent): [number, number] => {
      const r = c.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };

    c.addEventListener("mousemove", (e) => {
      const [sx, sy] = pos(e);
      if (dragNode) {
        const [wx, wy] = this.toWorld(sx, sy);
        dragNode.fx = wx; dragNode.fy = wy;
        this.sim.alphaTarget(0.15);
        return;
      }
      if (panning) {
        this.transform.x += sx - last[0];
        this.transform.y += sy - last[1];
        last = [sx, sy];
        return;
      }
      this.hover = this.nodeAt(sx, sy);
      c.style.cursor = this.hover ? "pointer" : "grab";
    });

    c.addEventListener("mousedown", (e) => {
      const [sx, sy] = pos(e);
      const n = this.nodeAt(sx, sy);
      if (n) {
        dragNode = n;
        this.selected = n;
        this.onSelect?.(n.id);
        this.sim.alphaTarget(0.2).restart();
      } else {
        panning = true;
        last = [sx, sy];
        this.selected = null;
        this.onSelect?.(null);
        this.onBackgroundClick?.();
      }
    });

    window.addEventListener("mouseup", () => {
      if (dragNode) {
        dragNode.fx = null; dragNode.fy = null;
        this.sim.alphaTarget(this.playing ? 0.08 : 0);
      }
      dragNode = null;
      panning = false;
    });

    c.addEventListener("wheel", (e) => {
      e.preventDefault();
      const [sx, sy] = pos(e);
      const [wx, wy] = this.toWorld(sx, sy);
      const factor = Math.exp(-e.deltaY * 0.0012);
      this.transform.k = Math.max(0.15, Math.min(6, this.transform.k * factor));
      this.transform.x = sx - wx * this.transform.k;
      this.transform.y = sy - wy * this.transform.k;
    }, { passive: false });
  }
}

export type { VNode };
