#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";

const [artifactPath, outputDirectory, widthText = "800", heightText = "600"] =
  process.argv.slice(2);
if (!artifactPath || !outputDirectory) {
  throw new Error("usage: render-trace-audit.mjs ARTIFACT OUTPUT_DIR [WIDTH HEIGHT]");
}
const width = Number(widthText);
const height = Number(heightText);
const artifact = JSON.parse(await fs.readFile(artifactPath, "utf8"));
const events = artifact.event_trace;
const state = { nodes: new Map(), edges: new Map(), transmissions: new Map() };
const stepNS = 16_000_000;

function copy(value) {
  return structuredClone(value);
}

function applyTopology(data) {
  state.nodes.clear();
  state.edges.clear();
  for (const node of data.nodes ?? []) state.nodes.set(node.id, copy(node));
  for (const edge of data.edges ?? []) state.edges.set(edge.id, copy(edge));
}

function applyArrival(data) {
  const id = data.node;
  const node = state.nodes.get(id) ?? {
    id, address: data.address ?? "", active: true, root: id, parent: null,
    sequence: 1, transport_type: "udp",
  };
  Object.assign(node, {
    active: true, address: data.address ?? node.address, root: id, parent: null,
    media_zone: data.media_zone ?? node.media_zone,
  });
  state.nodes.set(id, node);
  const edgeIDs = data.edges ?? (data.edge === undefined ? [] : [data.edge]);
  const targets = data.targets ?? (data.target === undefined ? [] : [data.target]);
  edgeIDs.forEach((edgeID, index) => {
    const edge = state.edges.get(edgeID);
    state.edges.set(edgeID, edge
      ? { ...edge, active: true }
      : { id: edgeID, from: id, to: targets[index], active: true });
  });
}

function removeTransmission(event, plane) {
  const copyIndex = event.data.copy ?? 0;
  if (event.causal_parent) {
    state.transmissions.delete(`${event.causal_parent}:${copyIndex}`);
    return;
  }
  for (const [id, item] of state.transmissions) {
    if (item.from === event.data.from && item.to === event.data.to
        && item.copy === copyIndex && (!plane || item.plane === plane)) {
      state.transmissions.delete(id);
    }
  }
}

function applyTransmission(event, plane) {
  for (const delivery of event.data.deliveries ?? []) {
    const copyIndex = delivery.copy ?? 0;
    const id = `${event.event_id}:${copyIndex}`;
    state.transmissions.set(id, {
      id, from: event.data.from, to: event.data.to, start_ns: event.virtual_time_ns,
      end_ns: delivery.deliver_at_ns, copy: copyIndex, plane,
    });
  }
}

function apply(event) {
  const { kind, data } = event;
  if (kind === "input.initial-topology") applyTopology(data);
  else if (["input.descending-root-arrival", "input.node-arrived",
    "input.authenticated-sybil-arrived"].includes(kind)) applyArrival(data);
  else if (["input.node-disappeared", "input.node-reappeared"].includes(kind)) {
    const node = state.nodes.get(data.node);
    if (node) {
      node.active = data.active ?? node.active;
      if (node.active) Object.assign(node, { root: node.id, parent: null });
    }
  } else if (["input.network-partitioned", "input.network-merged",
    "input.transport-class-failed", "input.transport-class-restored"].includes(kind)) {
    const active = kind.endsWith("merged") || kind.endsWith("restored");
    for (const changed of data.changed_edges ?? []) {
      const edge = state.edges.get(changed.id);
      if (edge) edge.active = active;
    }
  } else if (["input.parent-ancestry-swapped",
    "input.parent-quality-alternated"].includes(kind)) {
    const node = state.nodes.get(data.node);
    if (node && data.new_parent !== undefined) node.parent = data.new_parent;
    if (node && data.switched === true) node.sequence += 1;
  } else if (kind === "tree-announce.due") applyTransmission(event, "control");
  else if (kind === "data.frame-due") applyTransmission(event, "data");
  else if (kind === "bloom.filter-due") applyTransmission(event, "bloom");
  else if (kind === "lookup.frame-due") applyTransmission(event, "lookup");
  else if (kind === "session.frame-due") applyTransmission(event, "session");
  else if (kind === "tree-announce.delivered") {
    removeTransmission(event, "control");
    const node = state.nodes.get(data.to);
    if (node) Object.assign(node, {
      root: data.root_node ?? node.root,
      parent: data.parent ?? null,
      sequence: data.sequence ?? node.sequence,
    });
  } else if (kind.endsWith(".frame-delivered")
      || kind === "bloom.filter-delivered") {
    removeTransmission(event);
  }
}

function networkPositions(nodes) {
  const sorted = [...nodes].sort((left, right) => left.id - right.id);
  const count = Math.max(1, sorted.length);
  const radius = Math.min(width, height) * 0.46;
  const center = { x: width / 2, y: height / 2 };
  return new Map(sorted.map((node, index) => {
    const fraction = Math.sqrt((index + 0.5) / count);
    const angle = index * 2.399963229728653;
    return [node.id, {
      x: center.x + Math.cos(angle) * radius * fraction,
      y: center.y + Math.sin(angle) * radius * fraction,
    }];
  }));
}

function depths(nodes) {
  const result = new Map();
  for (const node of nodes.values()) {
    let current = node;
    const visited = new Set([node.id]);
    let depth = 0;
    while (current.parent !== null && current.parent !== undefined
        && !visited.has(current.parent) && nodes.has(current.parent)) {
      visited.add(current.parent);
      current = nodes.get(current.parent);
      depth += 1;
    }
    result.set(node.id, depth);
  }
  return result;
}

function cohortSnapshot(nodes) {
  const counts = new Map();
  for (const node of nodes.values()) counts.set(node.root, (counts.get(node.root) ?? 0) + 1);
  const major = [...counts].sort((a, b) => b[1] - a[1] || a[0] - b[0])
    .slice(0, 7).map(([root]) => root);
  const slots = new Map(major.map((root, index) => [root, index]));
  const nodeDepths = depths(nodes);
  const grouped = new Map();
  for (const node of nodes.values()) {
    const key = `${slots.get(node.root) ?? major.length}:${Math.min(7,
      Math.floor((nodeDepths.get(node.id) ?? 0) / 4))}:${node.transport_type ?? "udp"}`;
    const bucket = grouped.get(key) ?? { key, node_ids: [], active: 0 };
    bucket.node_ids.push(node.id);
    if (node.active) bucket.active += 1;
    grouped.set(key, bucket);
  }
  const buckets = [...grouped.values()].sort((a, b) => a.key.localeCompare(b.key));
  const columns = Math.max(1, new Set(buckets.map(item => item.key.split(":")[0])).size);
  const offsets = { wifi: [-12, -8], ble: [12, -8], tor: [-12, 8], ethernet: [12, 8] };
  const membership = new Map();
  const nodePositions = new Map();
  for (const bucket of buckets) {
    const [rootGroup, depthBand, transport] = bucket.key.split(":");
    const [dx, dy] = offsets[transport] ?? [0, 0];
    bucket.x = 54 + (width - 108) * (Number(rootGroup) + 0.5) / columns + dx;
    bucket.y = 54 + (height - 108) * (Number(depthBand) + 0.5) / 8 + dy;
    bucket.node_ids.sort((a, b) => a - b).forEach(id => {
      membership.set(id, bucket.key);
      nodePositions.set(id, { key: bucket.key, x: bucket.x, y: bucket.y });
    });
  }
  return { buckets, membership, nodePositions, majorRoots: major };
}

function mapChanges(before, after) {
  const added = [...after.keys()].filter(key => !before.has(key));
  const removed = [...before.keys()].filter(key => !after.has(key));
  return { added, removed };
}

function snapshot(timeNS) {
  const nodes = [...state.nodes.values()].sort((a, b) => a.id - b.id);
  const positions = networkPositions(nodes);
  const cohorts = cohortSnapshot(state.nodes);
  const transmissions = [...state.transmissions.values()].sort((a, b) => a.id.localeCompare(b.id))
    .map(item => {
      const from = positions.get(item.from);
      const to = positions.get(item.to);
      const progress = Math.min(1, Math.max(0,
        (timeNS - item.start_ns) / Math.max(1, item.end_ns - item.start_ns)));
      return { ...item, progress, x: from.x + (to.x - from.x) * progress,
        y: from.y + (to.y - from.y) * progress };
    });
  return {
    nodes: nodes.map(node => ({ id: node.id, active: node.active, root: node.root,
      parent: node.parent ?? null, ...positions.get(node.id) })),
    physical_edges: [...state.edges.values()].sort((a, b) => a.id - b.id)
      .map(edge => ({ id: edge.id, from: edge.from, to: edge.to, active: edge.active })),
    parent_links: nodes.filter(node => node.parent !== null && node.parent !== undefined)
      .map(node => `${node.id}->${node.parent}`).sort(),
    transmissions,
    cohorts: cohorts.buckets,
    major_roots: cohorts.majorRoots,
    cohort_membership: Object.fromEntries(cohorts.membership),
    cohort_node_positions: Object.fromEntries(cohorts.nodePositions),
  };
}

const frames = [];
let cursor = 0;
let previous = null;
const durationNS = events.at(-1)?.virtual_time_ns ?? 0;
for (let frameIndex = 0, timeNS = 0; ; frameIndex += 1, timeNS += stepNS) {
  const actualTime = Math.min(timeNS, durationNS);
  const applied = [];
  while (cursor < events.length && events[cursor].virtual_time_ns <= actualTime) {
    apply(events[cursor]);
    applied.push(events[cursor]);
    cursor += 1;
  }
  for (const [id, item] of state.transmissions) {
    if (item.end_ns < actualTime) state.transmissions.delete(id);
  }
  const visible = snapshot(actualTime);
  const currentTx = new Map(visible.transmissions.map(item => [item.id, item]));
  const priorTx = new Map((previous?.transmissions ?? []).map(item => [item.id, item]));
  const txDelta = mapChanges(priorTx, currentTx);
  const previousNodes = new Map((previous?.nodes ?? []).map(item => [item.id, item]));
  const nodeChanges = visible.nodes.filter(node => {
    const old = previousNodes.get(node.id);
    return old && (old.active !== node.active || old.root !== node.root || old.parent !== node.parent);
  }).map(node => ({ id: node.id, before: previousNodes.get(node.id), after: node }));
  const cohortChanges = previous ? visible.nodes.filter(node =>
    previous.cohort_membership[node.id] !== visible.cohort_membership[node.id]).map(node => node.id) : [];
  const networkMoves = previous ? visible.nodes.filter(node => {
    const old = previousNodes.get(node.id);
    return old && Math.hypot(old.x - node.x, old.y - node.y) > 0.01;
  }).map(node => node.id) : [];
  const cohortMoves = previous ? visible.nodes.filter(node => {
    const old = previous.cohort_node_positions[node.id];
    const next = visible.cohort_node_positions[node.id];
    return old && next && Math.hypot(old.x - next.x, old.y - next.y) > 0.01;
  }).map(node => node.id) : [];
  const explanations = [];
  if (txDelta.added.length) explanations.push(
    `${txDelta.added.length} bright full-edge line(s) appear for in-flight messages; not new links.`);
  if (txDelta.removed.length) explanations.push(
    `${txDelta.removed.length} bright line(s) disappear on delivery/expiry; links did not vanish.`);
  if (cohortChanges.length) explanations.push(
    `${cohortChanges.length} node(s) change cohort bubble because root/depth grouping was recomputed.`);
  if (cohortMoves.length) explanations.push(
    `${cohortMoves.length} node(s) move on screen in Cohorts view; this is layout, not physical motion.`);
  frames.push({
    frame: frameIndex, virtual_time_ns: actualTime,
    applied_events: applied.map(event => ({ id: event.event_id, kind: event.kind,
      ordinal: event.ordinal, causal_parent: event.causal_parent })),
    deltas: { transmissions_started: txDelta.added, transmissions_ended: txDelta.removed,
      node_state_changes: nodeChanges, network_position_changes: networkMoves,
      cohort_membership_changes: cohortChanges, cohort_position_changes: cohortMoves },
    explanations, visible,
  });
  previous = visible;
  if (actualTime === durationNS) break;
}

const summary = {
  artifact_id: artifact.manifest.artifact_id,
  run_id: artifact.manifest.run_id,
  canvas: { width, height },
  frame_step_ns: stepNS,
  frame_count: frames.length,
  event_count: events.length,
  frames_with_events: frames.filter(frame => frame.applied_events.length).length,
  frames_with_transmission_starts: frames.filter(frame => frame.deltas.transmissions_started.length).length,
  frames_with_transmission_ends: frames.filter(frame => frame.deltas.transmissions_ended.length).length,
  frames_with_node_state_changes: frames.filter(frame => frame.deltas.node_state_changes.length).length,
  frames_with_cohort_regrouping: frames.filter(frame => frame.deltas.cohort_membership_changes.length).length,
  frames_with_network_movement: frames.filter(frame => frame.deltas.network_position_changes.length).length,
  frames_with_cohort_movement: frames.filter(frame => frame.deltas.cohort_position_changes.length).length,
};
await fs.mkdir(outputDirectory, { recursive: true });
await fs.writeFile(path.join(outputDirectory, "frames.jsonl"),
  frames.map(frame => JSON.stringify(frame)).join("\n") + "\n");
const timeline = ["frame\tseconds\tevents\tnode_changes\tnetwork_moves\tcohort_moves\ttx_started\ttx_ended",
  ...frames.map(item => [item.frame, item.virtual_time_ns / 1e9, item.applied_events.length,
    item.deltas.node_state_changes.length, item.deltas.network_position_changes.length,
    item.deltas.cohort_position_changes.length, item.deltas.transmissions_started.length,
    item.deltas.transmissions_ended.length].join("\t"))];
await fs.writeFile(path.join(outputDirectory, "timeline.tsv"), timeline.join("\n") + "\n");
await fs.writeFile(path.join(outputDirectory, "summary.json"),
  JSON.stringify(summary, null, 2) + "\n");
console.log(JSON.stringify(summary, null, 2));
