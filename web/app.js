const content = document.querySelector("#content");
const notice = document.querySelector("#notice");
const picker = document.querySelector("#artifact-file");
let current = window.__FIPS_ANALYSIS__ || null;

const escapeHtml = (value) => String(value ?? "unknown")
  .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
  .replaceAll('"', "&quot;").replaceAll("'", "&#039;");

function value(item, suffix = "") {
  return item === null || item === undefined ? '<span class="unknown">unknown</span>' : `${escapeHtml(item)}${suffix}`;
}

function card(label, body, detail = "") {
  return `<article class="card"><p class="label">${escapeHtml(label)}</p><div class="value">${body}</div>${detail ? `<p class="detail">${escapeHtml(detail)}</p>` : ""}</article>`;
}

function summary(document) {
  const q = document.quiescence || {};
  return `<section id="summary"><div class="section-head"><h2>${escapeHtml(document.run_id)}</h2><span class="badge">${escapeHtml(document.representation)}</span></div>
    <div class="grid">${card("Represented nodes", value(document.represented_nodes))}${card("Events", value(document.event_count))}${card("Samples", value(document.sample_count))}${card("Fidelity", document.fidelity.exact ? "Exact" : "Approximate", document.fidelity.statement)}</div>
    <h3>Quiescence</h3><div class="grid compact">${card("Root", value(q.root_ns, " ns"))}${card("Tree", value(q.tree_ns, " ns"))}${card("Bloom", value(q.bloom_ns, " ns"))}${card("Lookup", value(q.lookup_ns, " ns"))}${card("Data plane", value(q.data_plane_ns, " ns"))}</div>
    ${document.fidelity.uncertainty?.length ? `<div class="callout"><strong>Uncertainty</strong>${document.fidelity.uncertainty.map(item => `<p>${escapeHtml(item)}</p>`).join("")}</div>` : ""}</section>`;
}

function metrics(document) {
  const rows = document.metrics.map(metric => `<tr><th>${escapeHtml(metric.name)}</th><td>${value(metric.last)}</td><td>${escapeHtml(metric.unit)}</td><td>${escapeHtml(metric.source.collection)} [${metric.source.start}, ${metric.source.end_exclusive})</td></tr>`).join("");
  return `<section id="metrics"><div class="section-head"><h2>Metrics</h2><span>${document.metrics.length} series</span></div><div class="table-wrap"><table><thead><tr><th>Metric</th><th>Latest</th><th>Unit</th><th>Evidence</th></tr></thead><tbody>${rows || '<tr><td colspan="4" class="unknown">No metric series observed</td></tr>'}</tbody></table></div></section>`;
}

function rootWave(document) {
  const points = document.root_wave?.points || [];
  const rows = points.map(point => `<li><time>${value(point.arrival_ns, " ns")}</time><code>${escapeHtml(point.root)}</code><strong>${escapeHtml(point.status)}</strong><small>${point.propagated_events} causal events · <a href="#causal">${escapeHtml(point.causal_id)}</a></small></li>`).join("");
  return `<section id="root-wave"><div class="section-head"><h2>Root lineage</h2><span>${points.length} generations</span></div><ol class="wave">${rows || '<li class="unknown">No descending-root lineage observed</li>'}</ol><p class="detail">Final consensus root: <code>${value(document.root_wave?.final_consensus_root)}</code></p></section>`;
}

function exactGraph(network) {
  const nodes = network.exact_nodes || [];
  if (!nodes.length) return "";
  const positions = new Map(nodes.map((node, index) => [node.id, {x: 300 + 220 * Math.cos(2 * Math.PI * index / nodes.length), y: 230 + 180 * Math.sin(2 * Math.PI * index / nodes.length)}]));
  const edges = network.exact_edges.map(edge => { const a = positions.get(edge.from), b = positions.get(edge.to); return a && b ? `<line x1="${a.x}" y1="${a.y}" x2="${b.x}" y2="${b.y}"><title>${edge.from}→${edge.to}: ${edge.frame_bytes} bytes</title></line>` : ""; }).join("");
  const circles = nodes.map(node => { const p = positions.get(node.id); return `<g><circle cx="${p.x}" cy="${p.y}" r="16"/><text x="${p.x}" y="${p.y + 4}">${node.id}</text></g>`; }).join("");
  return `<svg class="graph" viewBox="0 0 600 460" role="img" aria-label="Observed physical event edges">${edges}${circles}</svg>`;
}

function network(document) {
  const view = document.network || {top_edges: [], depth_frame_distribution: {}};
  const top = view.top_edges.map(edge => `<tr><th>${edge.from} → ${edge.to}</th><td>${edge.frames}</td><td>${edge.frame_bytes}</td><td>${edge.maximum_depth}</td></tr>`).join("");
  const depths = Object.entries(view.depth_frame_distribution).map(([depth, frames]) => `<li><span>depth ${depth}</span><meter min="0" max="${Math.max(...Object.values(view.depth_frame_distribution))}" value="${frames}"></meter><strong>${frames}</strong></li>`).join("");
  return `<section id="network"><div class="section-head"><h2>Network evidence</h2><span class="badge">${escapeHtml(view.mode)}</span></div>${exactGraph(view)}<h3>Frames by depth</h3><ol class="distribution">${depths || '<li class="unknown">No edge observations</li>'}</ol><h3>Top observed edges</h3><div class="table-wrap"><table><thead><tr><th>Physical edge</th><th>Frames</th><th>Bytes</th><th>Max depth</th></tr></thead><tbody>${top}</tbody></table></div><p class="detail">${escapeHtml(view.fidelity)}</p></section>`;
}

function causal(document) {
  const stages = document.causal.stages.map(stage => `<li><span>${escapeHtml(stage.stage)}</span><strong>${stage.count}</strong><small>${stage.entries} ledger entries</small></li>`).join("");
  const path = document.causal.critical_path.map(id => `<code>${escapeHtml(id)}</code>`).join("<span>→</span>");
  return `<section id="causal"><div class="section-head"><h2>Causal accounting</h2><span>${document.causal.source_entries} source entries</span></div><ol class="stages">${stages || '<li class="unknown">No causal ledger observed</li>'}</ol><h3>Critical path</h3><div class="path">${path || '<span class="unknown">unknown</span>'}</div></section>`;
}

function provenance(document) {
  const p = document.provenance;
  return `<section id="provenance"><div class="section-head"><h2>Provenance</h2><span class="badge">${escapeHtml(document.api_version)}</span></div><dl><dt>Engine</dt><dd>${escapeHtml(p.engine_name)} ${escapeHtml(p.engine_version)}</dd><dt>Source revision</dt><dd><code>${escapeHtml(p.engine_source_revision)}</code></dd><dt>Seed</dt><dd>${escapeHtml(p.seed)}</dd><dt>Plan SHA-256</dt><dd><code>${escapeHtml(p.normalized_plan_sha256)}</code></dd><dt>FIPS commit</dt><dd><code>${value(p.fips_commit)}</code></dd><dt>Image digest</dt><dd><code>${value(p.image_digest)}</code></dd></dl></section>`;
}

function comparison(left, right, firstDivergence) {
  if (!right) return '<section id="comparison"><h2>Comparison</h2><p class="unknown">Open two compatible artifacts to compare them.</p></section>';
  const compatible = left.represented_nodes === right.represented_nodes;
  const leftMetrics = new Map(left.metrics.map(item => [item.name, item.last]));
  const rightMetrics = new Map(right.metrics.map(item => [item.name, item.last]));
  const names = [...new Set([...leftMetrics.keys(), ...rightMetrics.keys()])].sort();
  const rows = names.map(name => { const a = leftMetrics.get(name), b = rightMetrics.get(name); const kind = a === undefined ? "left-unobserved" : b === undefined ? "right-unobserved" : a === b ? "match" : "divergence"; return `<tr><th>${escapeHtml(name)}</th><td>${value(a)}</td><td>${value(b)}</td><td>${kind}</td></tr>`; }).join("");
  return `<section id="comparison"><div class="section-head"><h2>Comparison</h2><span class="badge ${compatible ? "" : "warn"}">${compatible ? "compatible population" : "normalization required"}</span></div><p>First semantic divergence: <code>${value(firstDivergence)}</code></p><div class="table-wrap"><table><thead><tr><th>Metric</th><th>${escapeHtml(left.run_id)}</th><th>${escapeHtml(right.run_id)}</th><th>Class</th></tr></thead><tbody>${rows}</tbody></table></div></section>`;
}

function render(document, right = null, firstDivergence = null) {
  current = document;
  content.innerHTML = summary(document) + rootWave(document) + network(document) + metrics(document) + causal(document) + comparison(document, right, firstDivergence) + provenance(document);
  notice.textContent = `Immutable artifact ${document.artifact_id} · ${document.representation} view`;
  localStorage.setItem("fips-last-run", document.run_id);
  focusHash();
}

function focusHash() {
  const id = location.hash.slice(1);
  if (id) document.querySelector(`#${CSS.escape(id)}`)?.scrollIntoView({behavior: "smooth"});
}

function parseFile(file) {
  return new Promise((resolve, reject) => {
    const worker = new Worker("worker.js");
    worker.onmessage = ({data}) => { worker.terminate(); data.error ? reject(new Error(data.error)) : resolve(data.analysis); };
    worker.postMessage(file);
  });
}

picker.addEventListener("change", async () => {
  const files = [...(picker.files || [])].slice(0, 2);
  if (!files.length) return;
  if (files.some(file => file.size > 64 * 1024 * 1024)) {
    notice.textContent = "Artifact rejected: local analysis limit is 64 MiB.";
    return;
  }
  notice.textContent = `Parsing ${files.length} artifact${files.length > 1 ? "s" : ""} locally…`;
  try {
    const documents = await Promise.all(files.map(parseFile));
    let divergence = null;
    if (documents[1]) {
      const a = documents[0]._event_fingerprints || [], b = documents[1]._event_fingerprints || [];
      const index = a.findIndex((item, position) => item !== b[position]);
      divergence = index >= 0 ? a[index] : a.length !== b.length ? `event-index-${Math.min(a.length, b.length)}` : null;
    }
    documents.forEach(item => delete item._event_fingerprints);
    render(documents[0], documents[1], divergence);
  } catch (error) { notice.textContent = `Artifact rejected: ${error.message}`; }
});

window.addEventListener("hashchange", focusHash);
if (current) render(current);
