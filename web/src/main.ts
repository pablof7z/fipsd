import "./style.css";
import { generateRun } from "./fixture.ts";
import { SyntheticProvider } from "./providers/synthetic.ts";
import type { DataProvider } from "./contract.ts";
import { TopologyView } from "./graph.ts";
import { rgbStr } from "./palette.ts";

const SEC = 1_000_000_000;
const BUDGET = 1500; // hard render budget (marks); provider downsamples to honor it

const provider: DataProvider = new SyntheticProvider(generateRun({ nodeCount: 2000, roots: 5, seed: 0xf1ff }));
const [, endNs] = provider.timeExtentNs();
const duration = endNs / SEC;
const manifest = provider.manifest();

const canvas = document.getElementById("graph") as HTMLCanvasElement;
const view = new TopologyView(canvas);
view.registerRoots(provider.roots());
view.attachMinimap(document.getElementById("minimap") as HTMLCanvasElement);

// --- Run meta strip -----------------------------------------------------------
const meta = document.getElementById("run-meta")!;
const fid = manifest.fidelity;
meta.innerHTML = "";
const facts: [string, string][] = [
  ["run", manifest.runId],
  ["scale", fid.scale],
  ["represented", fid.representedNodes.toLocaleString()],
  ["seed", `0x${manifest.provenance.seed.toString(16)}`],
];
for (const [k, v] of facts) {
  const el = document.createElement("span");
  el.className = "fact";
  el.innerHTML = `<span class="k">${k}</span><span class="v">${v}</span>`;
  meta.appendChild(el);
}

// --- Legend (click a root to isolate it) --------------------------------------
const legend = document.getElementById("legend")!;
let focusRoot: string | null = null;
function renderLegend() {
  const roots = provider.roots();
  legend.innerHTML = `<div class="legend-title">roots (ratchet order)</div>`;
  roots.forEach((root, i) => {
    const row = document.createElement("div");
    row.className = "legend-row" + (focusRoot && focusRoot !== root ? " dimmed" : "") + (focusRoot === root ? " active" : "");
    row.innerHTML =
      `<span class="swatch" style="background:${rgbStr(view.palette.rootColor(root))};color:${rgbStr(view.palette.rootColor(root))}"></span>` +
      `<span class="legend-label">${root}</span>` +
      `<span class="legend-ord">#${i + 1}</span>`;
    row.addEventListener("click", () => {
      focusRoot = focusRoot === root ? null : root;
      view.setFocusRoot(focusRoot);
      renderLegend();
    });
    legend.appendChild(row);
  });
}
renderLegend();

// --- Root-wave markers on the timeline ----------------------------------------
const waveMarkers = document.getElementById("wave-markers")!;
function renderWaveMarkers() {
  waveMarkers.innerHTML = "";
  for (const { root, tNs } of provider.rootTimeline()) {
    const frac = duration ? tNs / SEC / duration : 0;
    const mark = document.createElement("div");
    mark.className = "wave-mark";
    mark.style.left = `${frac * 100}%`;
    mark.style.background = rgbStr(view.palette.rootColor(root));
    mark.setAttribute("data-label", root);
    waveMarkers.appendChild(mark);
  }
}
renderWaveMarkers();

// --- Inspector ----------------------------------------------------------------
const inspector = document.getElementById("inspector")!;
function renderInspector(id: string | null) {
  if (!id) {
    inspector.innerHTML = `<div class="inspector-empty">Hover or click a node</div>`;
    return;
  }
  const d = provider.nodeDetail(id, Math.round(view.time() * SEC));
  if (!d) {
    inspector.innerHTML = `<div class="inspector-empty">—</div>`;
    return;
  }
  const rows = Object.entries(d.fields)
    .map(([k, v]) => `<span class="k">${k}</span><span class="v mono">${v}</span>`)
    .join("");
  inspector.innerHTML = `
    <div class="insp-head">${d.id}</div>
    <div class="insp-kind"><span class="pill ${d.kind}">${d.kind}</span></div>
    <div class="insp-grid">${rows}</div>`;
}
view.onSelect = renderInspector;
view.onBackgroundClick = () => {
  if (focusRoot) {
    focusRoot = null;
    view.setFocusRoot(null);
    renderLegend();
  }
};

// --- Resolution (LOD) ---------------------------------------------------------
const lod = document.getElementById("lod") as HTMLInputElement;
const regimeBadge = document.getElementById("regime-badge")!;
const coverage = document.getElementById("coverage")!;
lod.min = "0";
lod.max = String(provider.maxLevel());
let level = provider.defaultLevel(BUDGET);
lod.value = String(level);

function applyLevel(fit: boolean) {
  const res = provider.topology({ level, budget: BUDGET });
  view.setData(res);
  regimeBadge.textContent = res.regime;
  regimeBadge.className = `regime-badge ${res.regime}`;
  const c = res.coverage;
  const pct = c.totalNodes ? Math.round((c.shownNodes / c.totalNodes) * 100) : 100;
  const plural = (n: number, w: string) => `${n.toLocaleString()} ${w}${n === 1 ? "" : "s"}`;
  coverage.textContent =
    res.regime === "aggregate"
      ? `${plural(c.shownNodes, "cohort")} · ${c.totalNodes.toLocaleString()} nodes`
      : `${plural(c.shownNodes, "node")}${c.truncated ? ` (top ${pct}% by degree)` : ""}`;
  if (fit) requestAnimationFrame(() => view.fit());
}
lod.addEventListener("input", () => {
  level = parseInt(lod.value, 10);
  applyLevel(false);
});

// --- Transport ----------------------------------------------------------------
const btnPlay = document.getElementById("btn-play") as HTMLButtonElement;
const scrubber = document.getElementById("scrubber") as HTMLInputElement;
const speedSel = document.getElementById("speed") as HTMLSelectElement;
const clockNow = document.getElementById("clock-now")!;
const clockEnd = document.getElementById("clock-end")!;
clockEnd.textContent = duration.toFixed(3);

let playing = false;
let speed = 1;
let userScrubbing = false;

function setPlaying(p: boolean) {
  playing = p;
  view.setPlaying(p);
  btnPlay.textContent = p ? "❚❚" : "▶";
  btnPlay.classList.toggle("is-playing", p);
}

btnPlay.addEventListener("click", () => {
  if (view.time() >= duration - 1e-6) view.setTime(0);
  setPlaying(!playing);
});
speedSel.addEventListener("change", () => (speed = parseFloat(speedSel.value)));
scrubber.addEventListener("input", () => {
  userScrubbing = true;
  view.setTime((parseFloat(scrubber.value) / 1000) * duration);
});
scrubber.addEventListener("change", () => (userScrubbing = false));

document.getElementById("toggle-labels")!.addEventListener("change", (e) =>
  view.setShowLabels((e.target as HTMLInputElement).checked),
);
document.getElementById("toggle-links")!.addEventListener("change", (e) =>
  view.setShowLinks((e.target as HTMLInputElement).checked),
);
document.getElementById("btn-reheat")!.addEventListener("click", () => view.reheat());
document.getElementById("btn-fit")!.addEventListener("click", () => view.fit());

window.addEventListener("resize", () => view.resize());
window.addEventListener("keydown", (e) => {
  if (e.code === "Space") { e.preventDefault(); btnPlay.click(); }
});

// --- Deep links (down-payment on shareable reports, #64) ----------------------
const params = new URLSearchParams(location.search);
const tParam = parseFloat(params.get("t") ?? "");
if (Number.isFinite(tParam)) view.setTime(Math.max(0, Math.min(duration, tParam)));
if (params.has("lod")) {
  level = Math.max(0, Math.min(provider.maxLevel(), parseInt(params.get("lod")!, 10) || 0));
  lod.value = String(level);
}

// --- Boot ---------------------------------------------------------------------
applyLevel(false);
for (let i = 0; i < 160; i++) view.frame();
view.fit();
if (params.get("play") === "1") setPlaying(true);

let lastTs = performance.now();
function loop(ts: number) {
  const dt = Math.min(0.05, (ts - lastTs) / 1000);
  lastTs = ts;
  if (playing && !userScrubbing) {
    let t = view.time() + dt * speed;
    if (t >= duration) { t = duration; setPlaying(false); }
    view.setTime(t);
  }
  if (!userScrubbing) {
    clockNow.textContent = view.time().toFixed(3);
    scrubber.value = String(Math.round((view.time() / duration) * 1000));
  }
  view.frame();
  renderInspector(view.hoverId() ?? view.selectedId());
  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);
