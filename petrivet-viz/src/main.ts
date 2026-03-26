import initWasm, { WasmSystem, WasmNetBuilder } from 'petrivet-wasm';
import type {
  WasmNetStructure,
  WasmPosition,
  WasmBuilderStructure,
  WasmBuilderPlace,
} from 'petrivet-wasm';
import cytoscape from 'cytoscape';
import type { Core, ElementDefinition, NodeSingular } from 'cytoscape';
import cola from 'cytoscape-cola';
import fcose from 'cytoscape-fcose';
import dagre from 'cytoscape-dagre';

// Register layout extensions once, at module scope, before any cy instance
// is created. Calling use() more than once for the same extension is safe.
cytoscape.use(cola);
cytoscape.use(fcose);
cytoscape.use(dagre);

// App state

let sys: WasmSystem | null = null;
let cy: Core | null = null;
let cachedStructure: WasmNetStructure | null = null;
let animating = false;
let activeLayout: ReturnType<Core['layout']> | null = null;
/** Cola physics while in edit mode (separate from simulation layouts). */
let editColaLayout: ReturnType<Core['layout']> | null = null;
// Marking vector at which analysis was last run, so we can show a stale badge
let lastAnalyzedMarking: number[] | null = null;

// Edit mode
let editMode = false;
let builder: WasmNetBuilder | null = null;
/** After "New net", fit the viewport once after edit Cola has placed nodes. */
let pendingFitForNewNet = false;
/** Detect topology growth so Cola can restart and pick up new nodes. */
let lastSyncedEditNodeCount = -1;
/** Captured from the edit canvas before `build()` so simulation keeps the same layout. */
let simLayoutSnapshot: Map<string, { x: number; y: number }> | null = null;

/** Simulation mode: run Cola after static layouts when enabled (toolbar checkbox). */
let simPhysicsEnabled = true;

interface ArcDrawState {
  sourceId: string;
  sourceType: 'place' | 'transition';
}
let arcDrawState: ArcDrawState | null = null;

// Bootstrap

function main(): void {
  cy = cytoscape({
    container: el('cy'),
    style: netStyles(),
    layout: { name: 'preset' },
    minZoom: 0.05,
    maxZoom: 6,
  });

  cy.on('tap', 'node[type="transition"]', (evt) => {
    if (!sys || animating || editMode) return;
    const idx = evt.target.data('index') as number;
    if (sys.enabledTransitions().includes(idx)) {
      void animateFire(idx);
    }
  });

  cy.on('tap', 'node', (evt) => showNodeInfo(evt.target as NodeSingular));
  cy.on('tap', (evt) => { if (evt.target === cy) clearNodeInfo(); });

  // Ghost arc canvas — overlays the cy container; created here so Cytoscape
  // has already applied `position: relative` to the container.
  const ghost = document.createElement('canvas');
  ghost.id = 'arc-ghost';
  el('cy').appendChild(ghost);

  setupFileInput();
  setupDropzone();
  setupToolbar();
  setupAnimSlider();
  setupLayoutSelector();
  setupEditMode();
}

// PNML loading

function loadPnml(xml: string): void {
  try {
    simLayoutSnapshot = null;
    const next = WasmSystem.parsePnml(xml);
    sys?.free();
    sys = next;
    renderNet();
    setStatus('');
  } catch (err) {
    setStatus(`Parse error: ${err instanceof Error ? err.message : String(err)}`);
  }
}

// Net rendering

function renderNet(): void {
  if (!sys || !cy) return;

  const snapshot = simLayoutSnapshot;
  simLayoutSnapshot = null;

  cachedStructure = sys.netStructure();
  const s = cachedStructure;

  // Compute a position scale so PNML nodes don't visually overlap.
  const scale = pnmlPositionScale(s.place_positions, s.transition_positions);

  const elements: ElementDefinition[] = [];

  for (let i = 0; i < s.place_count; i++) {
    const id = `p${i}`;
    const snap = snapshot?.get(id);
    const pnml = s.place_positions[i];
    let position: { x: number; y: number } | undefined;
    if (snap) {
      position = { x: snap.x, y: snap.y };
    } else if (pnml != null) {
      position = { x: pnml.x * scale, y: pnml.y * scale };
    }
    elements.push({
      data: {
        id: `p${i}`, type: 'place', index: i,
        name: s.place_names[i] ?? defaultPlaceName(i),
        tokens: 0, label: '',
      },
      ...(position != null ? { position } : {}),
    });
  }

  for (let i = 0; i < s.transition_count; i++) {
    const id = `t${i}`;
    const snap = snapshot?.get(id);
    const pnml = s.transition_positions[i];
    let position: { x: number; y: number } | undefined;
    if (snap) {
      position = { x: snap.x, y: snap.y };
    } else if (pnml != null) {
      position = { x: pnml.x * scale, y: pnml.y * scale };
    }
    elements.push({
      data: {
        id: `t${i}`, type: 'transition', index: i,
        name: s.transition_names[i] ?? defaultTransitionName(i),
        label: s.transition_names[i] ?? defaultTransitionName(i),
        enabled: false,
      },
      ...(position != null ? { position } : {}),
    });
  }

  for (const arc of s.pt_arcs) {
    elements.push({
      data: {
        id: `arc_p${arc.source}_t${arc.target}`,
        source: `p${arc.source}`,
        target: `t${arc.target}`,
      },
    });
  }
  for (const arc of s.tp_arcs) {
    elements.push({
      data: {
        id: `arc_t${arc.source}_p${arc.target}`,
        source: `t${arc.source}`,
        target: `p${arc.target}`,
      },
    });
  }

  // Stop any running physics before replacing graph elements
  activeLayout?.stop();
  activeLayout = null;

  cy.elements().remove();
  cy.add(elements);

  const hasPositions =
    (snapshot != null && snapshot.size > 0) ||
    s.place_positions.some((p) => p != null) ||
    s.transition_positions.some((p) => p != null);
  if (hasPositions) {
    cy.layout({ name: 'preset' }).run();
    cy.fit(undefined, 60);
  } else {
    // Use the currently selected layout algorithm (fitAfter=true so it fits
    // once the layout converges or times out for cola).
    const selectedLayout = (el('layout-select') as HTMLSelectElement).value;
    applyLayout(selectedLayout, true);
  }
  lastAnalyzedMarking = null;
  el('analysis-stale').classList.remove('visible');
  el('analysis-content').innerHTML =
    '<p class="hint">Click <em>Analyze marking</em> to compute properties.</p>';
  el('btn-analyze').removeAttribute('disabled');

  syncMarking();

  const netName = s.net_name ?? 'Untitled net';
  document.title = `petrivet — ${netName}`;
  el('net-name').textContent = netName;
}

// Position scaling

/**
 * Given PNML positions (in "editor pixels" for a tool with ~30px nodes),
 * compute a uniform scale factor so our larger nodes have comfortable spacing.
 *
 * Strategy: find the closest pair of nodes and scale so that distance
 * becomes at least `TARGET_MIN_DIST` pixels. Clamped between 1.5 and 6.
 */
function pnmlPositionScale(
  places: (WasmPosition | undefined)[],
  transitions: (WasmPosition | undefined)[],
): number {
  const all = ([...places, ...transitions] as (WasmPosition | undefined)[])
    .filter((p): p is WasmPosition => p != null);

  if (all.length < 2) return 2.0;

  const TARGET_MIN_DIST = 90; // px between node centres in layout space

  let minDist = Infinity;
  for (let i = 0; i < all.length; i++) {
    for (let j = i + 1; j < all.length; j++) {
      const dx = all[i]!.x - all[j]!.x;
      const dy = all[i]!.y - all[j]!.y;
      const d = Math.sqrt(dx * dx + dy * dy);
      if (d > 0 && d < minDist) minDist = d;
    }
  }

  const raw = TARGET_MIN_DIST / minDist;
  return Math.min(Math.max(raw, 1.5), 6);
}

// Labels & tokens (place nodes)

function tokenLabel(n: number): string {
  if (n === 0) return '';
  if (n <= 4) return '●'.repeat(n);
  return String(n);
}

function defaultPlaceName(index: number): string {
  return `P${index + 1}`;
}

function defaultTransitionName(index: number): string {
  return `T${index + 1}`;
}

function placeCanvasLabel(name: string | undefined, n: number, index: number): string {
  const displayName = name && name.trim().length > 0 ? name : defaultPlaceName(index);
  const tok = tokenLabel(n);
  return tok ? `${displayName}\n${tok}` : displayName;
}

function placeDisplayLabel(p: WasmBuilderPlace): string {
  const name = p.name && p.name.trim().length > 0 ? p.name : defaultPlaceName(p.id);
  const tok = p.initial_tokens > 0 ? tokenLabel(p.initial_tokens) : '';
  return tok ? `${name}\n${tok}` : name;
}

// Marking sync

function syncMarking(): void {
  if (!sys || !cy) return;
  syncMarkingForPlaces(Array.from(sys.currentMarking()));
}

/** @param markingForSidebar — if set, sidebar / stale use this vector (e.g. true WASM while canvas uses a display-only merge). */
function syncMarkingForPlaces(marking: number[], markingForSidebar?: number[]): void {
  if (!sys || !cy) return;

  const enabled = new Set(Array.from(sys.enabledTransitions()));
  const sidebarVec = markingForSidebar ?? marking;

  for (let i = 0; i < marking.length; i++) {
    const t = marking[i]!;
    const node = cy.$(`#p${i}`);
    const dName = node.data('name') as string | undefined;
    node.data('tokens', t).data('label', placeCanvasLabel(dName, t, i));
  }

  cy.$('node[type="transition"]').forEach((node) => {
    node.data('enabled', enabled.has(node.data('index') as number));
  });

  el('marking-row').textContent = `(${sidebarVec.join(', ')})`;
  el('deadlock-warn').textContent = sys.isDeadlocked() ? '⚠ Deadlocked' : '';

  if (lastAnalyzedMarking) {
    const stale = sidebarVec.some((v, i) => v !== lastAnalyzedMarking![i]);
    el('analysis-stale').classList.toggle('visible', stale);
  }

  el('btn-reset').removeAttribute('disabled');
}

function computeInputConsumption(transIdx: number, s: WasmNetStructure): Map<number, number> {
  const m = new Map<number, number>();
  for (const a of s.pt_arcs) {
    if (a.target === transIdx) {
      m.set(a.source, (m.get(a.source) ?? 0) + 1);
    }
  }
  return m;
}

function applyInputPlacePreviewAfterConsume(transIdx: number): void {
  if (!sys || !cy || !cachedStructure) return;
  const s = cachedStructure;
  const marking = Array.from(sys.currentMarking());
  const cons = computeInputConsumption(transIdx, s);
  for (const [pi, count] of cons) {
    const newM = Math.max(0, marking[pi]! - count);
    const node = cy.$(`#p${pi}`);
    const dName = node.data('name') as string | undefined;
    node.data('tokens', newM).data('label', placeCanvasLabel(dName, newM, pi));
  }
}

// Transition firing with token animation

async function animateFire(transIdx: number): Promise<void> {
  if (!sys || !cy || !cachedStructure || animating) return;

  const duration = animDuration();

  if (duration < 20) {
    // Instant — skip animation entirely
    sys.fire(transIdx);
    syncMarking();
    return;
  }

  animating = true;

  const s = cachedStructure;
  const inputPlaces = s.pt_arcs
    .filter((a) => a.target === transIdx)
    .map((a) => a.source);
  const outputPlaces = s.tp_arcs
    .filter((a) => a.source === transIdx)
    .map((a) => a.target);

  const transNode = cy.$(`#t${transIdx}`);
  const transPos = transNode.position();

  const markingBeforeFire = Array.from(sys.currentMarking());

  // Input places show post-consumption marking as soon as the animation starts
  // (tokens are treated as having left for the transition).
  applyInputPlacePreviewAfterConsume(transIdx);

  //  Phase 1: tokens converge onto the transition
  await Promise.all(
    inputPlaces.map((pi) => {
      const from = cy!.$(`#p${pi}`).position();
      return moveGhost(from, transPos, duration);
    }),
  );

  sys.fire(transIdx);

  const nowEnabled = new Set(Array.from(sys.enabledTransitions()));
  transNode.data('enabled', nowEnabled.has(transIdx));

  const markingAfter = Array.from(sys.currentMarking());
  const outSet = new Set(outputPlaces);
  const merged = markingAfter.map((v, i) => (outSet.has(i) ? markingBeforeFire[i] ?? 0 : v));
  syncMarkingForPlaces(merged, markingAfter);

  //  Phase 2: new tokens emanate from the transition
  await Promise.all(
    outputPlaces.map((pi) => {
      const to = cy!.$(`#p${pi}`).position();
      return moveGhost(transPos, to, duration);
    }),
  );

  syncMarking();
  animating = false;
}

/** Animate a transient "bullet" dot between two positions. */
function moveGhost(
  from: { x: number; y: number },
  to: { x: number; y: number },
  duration: number,
): Promise<void> {
  if (!cy) return Promise.resolve();
  const id = `ghost-${Math.random().toString(36).slice(2)}`;
  cy.add({ group: 'nodes', data: { id, type: 'token-ghost' }, position: { ...from } });
  return new Promise<void>((resolve) => {
    cy!.$(`#${id}`).animate({
      position: to,
      duration,
      easing: 'ease-in-out',
      complete() {
        cy!.remove(`#${id}`);
        resolve();
      },
    });
  });
}

// Analysis panel

const NET_CLASS_INFO: Record<string, { label: string; desc: string }> = {
  Circuit: {
    label: 'Circuit',
    desc: 'A simple closed loop.',
  },
  SNet: {
    label: 'S-net',
    desc: 'Every transition has exactly one input place and one output place. ',
  },
  TNet: {
    label: 'T-net',
    desc: 'Every place has exactly one input transition and one output transition. ',
  },
  FreeChoice: {
    label: 'Free-Choice',
    desc: 'If two transitions share an input place they have identical input sets. '
      + 'Choice and concurrency are structurally separated. '
      + 'Commoner\'s Theorem and the Rank Theorem give efficient liveness criteria.',
  },
  AsymmetricChoice: {
    label: 'Asymmetric Choice',
    desc: 'If two transitions share an input place, one\'s input set is a subset of '
      + 'the other\'s. A strict generalisation of Free-Choice. '
      + 'Many structural analysis results still apply.',
  },
  Unrestricted: {
    label: 'P/T Net',
    desc: 'No structural restrictions. The most general class. ',
  },
};

function runAnalysis(): void {
  if (!sys) return;
  lastAnalyzedMarking = Array.from(sys.currentMarking());
  updateAnalysisPanel();
  el('analysis-stale').classList.remove('visible');
}

function updateAnalysisPanel(): void {
  if (!sys || !cachedStructure) return;
  const s = cachedStructure;

  const classKey = s.net_class as string;
  const classInfo = NET_CLASS_INFO[classKey] ?? { label: classKey, desc: '' };
  const infoBtn = classInfo.desc
    ? `<button class="info-btn" aria-label="About ${classInfo.label}">ℹ</button>`
    : '';

  el('analysis-content').innerHTML = `
    <div class="prop">
      <span>Class</span>
      <span class="class-cell">${infoBtn}${classInfo.label}</span>
    </div>
    <div class="prop"><span>Places</span><span>${s.place_count}</span></div>
    <div class="prop"><span>Transitions</span><span>${s.transition_count}</span></div>
    <div class="prop"><span>Bounded</span><span>${yesNo(sys.isBounded())}</span></div>
    <div class="prop"><span>Live (L4)</span><span>${yesNo(sys.isLive())}</span></div>
    <div class="prop"><span>Deadlock-free</span><span>${yesNo(sys.isDeadlockFree())}</span></div>
  `;

  // Tooltip: opens to the LEFT of the sidebar, floating over the canvas
  el('analysis-content').querySelector('.info-btn')?.addEventListener('click', (e) => {
    e.stopPropagation();
    const existing = document.getElementById('class-tooltip');
    if (existing) { existing.remove(); return; }

    const btn = e.currentTarget as HTMLElement;
    const rect = btn.getBoundingClientRect();

    const tooltip = document.createElement('div');
    tooltip.id = 'class-tooltip';
    tooltip.className = 'class-tooltip';
    tooltip.textContent = classInfo.desc;

    // Position: right edge of tooltip is 8px left of the sidebar's left edge
    const rightOffset = window.innerWidth - rect.left + 8;
    const topOffset = Math.min(rect.top, window.innerHeight - 160);
    tooltip.style.right = `${rightOffset}px`;
    tooltip.style.top = `${topOffset}px`;

    document.body.appendChild(tooltip);

    const dismiss = (ev: Event) => {
      if (!tooltip.contains(ev.target as Node)) {
        tooltip.remove();
        document.removeEventListener('click', dismiss);
      }
    };
    setTimeout(() => document.addEventListener('click', dismiss), 0);
  });
}

function yesNo(b: boolean): string {
  return b
    ? '<span style="color:#4ade80">yes</span>'
    : '<span style="color:#f87171">no</span>';
}

// Node info

function showNodeInfo(node: NodeSingular): void {
  const d = node.data() as Record<string, unknown>;
  if (d.type === 'place') {
    el('node-info').textContent =
      `Place: ${String(d.name)}  ·  tokens: ${Number(d.tokens)}`;
  } else if (d.type === 'transition') {
    el('node-info').textContent =
      `Transition: ${String(d.name)}  (${d.enabled ? 'enabled' : 'disabled'})`;
  }
}

function clearNodeInfo(): void {
  el('node-info').textContent = '';
}

// File I/O

function setupFileInput(): void {
  const input = el('file-input') as HTMLInputElement;
  input.addEventListener('change', async () => {
    const file = input.files?.[0];
    if (file) loadPnml(await file.text());
    input.value = '';
  });
}

function setupDropzone(): void {
  el('cy').addEventListener('dragover', (e) => e.preventDefault());
  el('cy').addEventListener('drop', async (e) => {
    e.preventDefault();
    const file = (e as DragEvent).dataTransfer?.files[0];
    if (file?.name.endsWith('.pnml')) loadPnml(await file.text());
  });
}

// Toolbar & slider

function setupToolbar(): void {
  el('btn-open').addEventListener('click', () => el('file-input').click());
  el('btn-new').addEventListener('click', () => {
    if (!editMode) enterEditMode(true);
  });
  el('btn-reset').addEventListener('click', () => {
    if (animating || editMode) return;
    sys?.reset();
    syncMarking();
  });
  el('btn-fit').addEventListener('click', () => cy?.fit(undefined, 60));
  el('btn-analyze').addEventListener('click', () => runAnalysis());
  el('btn-edit').addEventListener('click', () => {
    if (editMode) exitEditMode();
    else enterEditMode(false);
  });
}

type LayoutOpts = Parameters<Core['layout']>[0];

function layoutOptions(name: string): LayoutOpts {
  switch (name) {
    case 'fcose':
      return {
        name: 'fcose',
        animate: true,
        animationDuration: 800,
        quality: 'proof',
        randomize: false,
        // Push nodes apart; good defaults for bipartite Petri net topology
        nodeRepulsion: () => 8000,
        idealEdgeLength: () => 110,
        edgeElasticity: () => 0.45,
        nestingFactor: 0.1,
        gravity: 0.25,
        numIter: 2500,
        // nodeSeparation: 80,
      } as unknown as LayoutOpts;

    case 'cola':
      return {
        name: 'cola',
        infinite: true,
        animate: true,
        // Default cola option is fit: true — that refits the viewport on every tick,
        // which fights user zoom/pan while physics runs.
        fit: false,
        refresh: 2,
        nodeSpacing: 24,
        edgeLength: 120,
        avoidOverlap: true,
        unconstrainedIterations: 50,
        userConstIter: 0,
        allConstIter: 0,
      } as unknown as LayoutOpts;

    case 'dagre':
      return {
        name: 'dagre',
        animate: true,
        animationDuration: 600,
        rankDir: 'LR',
        nodeSep: 60,
        edgeSep: 20,
        rankSep: 110,
      } as unknown as LayoutOpts;

    case 'breadthfirst':
      return {
        name: 'breadthfirst',
        animate: true,
        animationDuration: 600,
        directed: false,
        padding: 40,
        spacingFactor: 1.75,
      } as LayoutOpts;

    case 'circle':
      return {
        name: 'circle',
        animate: true,
        animationDuration: 600,
        padding: 40,
        spacingFactor: 1.5,
      } as LayoutOpts;

    case 'grid':
    default:
      return {
        name: 'grid',
        animate: true,
        animationDuration: 600,
        padding: 40,
        spacingFactor: 1.5,
      } as LayoutOpts;
  }
}

/** Run Cola physics on the current node positions (simulation mode only). */
function runSimPhysicsCola(fitAfterDelay = true): void {
  if (!cy || editMode || !simPhysicsEnabled) return;
  activeLayout?.stop();
  const layout = cy.layout(layoutOptions('cola'));
  activeLayout = layout;
  layout.run();
  const btnFreeze = el('btn-freeze') as HTMLButtonElement;
  btnFreeze.style.display = '';
  btnFreeze.textContent = '⏸';
  btnFreeze.title = 'Pause physics';
  if (fitAfterDelay) {
    setTimeout(() => cy?.fit(undefined, 60), 1200);
  }
}

/** Apply a static layout; if physics is on, Cola takes over afterward. */
function applyLayout(name: string, fitAfter = true): void {
  if (!cy) return;
  if (editMode) return;

  activeLayout?.stop();
  activeLayout = null;

  const staticLayout = cy.layout(layoutOptions(name));
  staticLayout.on('layoutstop', () => {
    if (fitAfter) cy?.fit(undefined, 60);
    if (simPhysicsEnabled) {
      runSimPhysicsCola(true);
    } else {
      (el('btn-freeze') as HTMLButtonElement).style.display = 'none';
    }
  });
  staticLayout.run();
}

function setupLayoutSelector(): void {
  const select = el('layout-select') as HTMLSelectElement;
  const physics = el('physics-enabled') as HTMLInputElement;
  simPhysicsEnabled = physics.checked;

  select.addEventListener('change', () => applyLayout(select.value));

  physics.addEventListener('change', () => {
    simPhysicsEnabled = physics.checked;
    if (!simPhysicsEnabled) {
      activeLayout?.stop();
      activeLayout = null;
      (el('btn-freeze') as HTMLButtonElement).style.display = 'none';
    } else if (cy && !editMode) {
      runSimPhysicsCola(true);
    }
  });

  const btnFreeze = el('btn-freeze') as HTMLButtonElement;
  btnFreeze.addEventListener('click', () => {
    if (activeLayout) {
      activeLayout.stop();
      activeLayout = null;
      btnFreeze.textContent = '▶';
      btnFreeze.title = 'Resume physics';
    } else if (simPhysicsEnabled && cy && !editMode) {
      runSimPhysicsCola(false);
    }
  });
}

// ---------------------------------------------------------------------------
// Edit mode
// ---------------------------------------------------------------------------

function captureLayoutFromCy(): Map<string, { x: number; y: number }> {
  const m = new Map<string, { x: number; y: number }>();
  if (!cy) return m;
  cy.nodes().forEach((n) => {
    const typ = n.data('type');
    if (typ === 'place' || typ === 'transition') {
      const p = n.position();
      m.set(n.id(), { x: p.x, y: p.y });
    }
  });
  return m;
}

function seedBuilderFromCyLayout(builder: WasmNetBuilder, layout: Map<string, { x: number; y: number }>): void {
  for (const [nodeId, pos] of layout) {
    if (nodeId.startsWith('p')) {
      const id = parseInt(nodeId.slice(1), 10);
      if (!Number.isNaN(id)) builder.setPlacePosition(id, pos.x, pos.y);
    } else if (nodeId.startsWith('t')) {
      const id = parseInt(nodeId.slice(1), 10);
      if (!Number.isNaN(id)) builder.setTransitionPosition(id, pos.x, pos.y);
    }
  }
}

/** Minimal connected net: P1 → T1 → P2 with one token on P1. */
function createSeededNetBuilder(): WasmNetBuilder {
  const b = new WasmNetBuilder();
  b.setNetName('Untitled net');
  const p0 = b.addPlace(-140, 0, 'P1');
  const t0 = b.addTransition(0, 0, 'T1');
  const p1 = b.addPlace(140, 0, 'P2');
  b.addArcPT(p0, t0);
  b.addArcTP(t0, p1);
  b.setInitialTokens(p0, 1);
  return b;
}

function defaultViewportCenter(): { x: number; y: number } {
  if (!cy) return { x: 0, y: 0 };
  const ext = cy.extent();
  return { x: (ext.x1 + ext.x2) / 2, y: (ext.y1 + ext.y2) / 2 };
}

/** One-time fit after creating a new net (waits for layout so bounds are valid). */
function scheduleNewNetFitOnce(layout: ReturnType<Core['layout']>): void {
  if (!pendingFitForNewNet || !cy) return;
  const fitOnce = (): void => {
    if (!pendingFitForNewNet || !cy) return;
    pendingFitForNewNet = false;
    cy.fit(undefined, 60);
  };
  layout.one('layoutready', fitOnce);
  window.setTimeout(fitOnce, 500);
}

function pauseEditCola(): void {
  editColaLayout?.stop();
  editColaLayout = null;
}

function startEditCola(): void {
  if (!cy || !editMode) return;
  const nModel = cy.nodes().filter((node) => {
    const t = node.data('type');
    return t === 'place' || t === 'transition';
  }).length;
  if (nModel === 0) return;
  editColaLayout?.stop();
  const layout = cy.layout(layoutOptions('cola'));
  editColaLayout = layout;
  scheduleNewNetFitOnce(layout);
  layout.run();
}

function enterEditMode(newNet: boolean): void {
  editMode = true;
  builder?.free();

  activeLayout?.stop();
  activeLayout = null;

  const layoutSeed = !newNet && sys && cy ? captureLayoutFromCy() : null;

  lastSyncedEditNodeCount = -1;

  if (newNet || !sys) {
    builder = createSeededNetBuilder();
    sys?.free();
    sys = null;
    pendingFitForNewNet = true;
  } else {
    builder = sys.toBuilder();
    sys.free();
    sys = null;
    pendingFitForNewNet = false;
  }

  if (layoutSeed && builder) {
    seedBuilderFromCyLayout(builder, layoutSeed);
  }

  (el('layout-select') as HTMLSelectElement).disabled = true;
  el('btn-freeze').style.display = 'none';

  el('btn-edit').textContent = 'Simulate';
  el('btn-edit').classList.add('active');
  el('btn-reset').setAttribute('disabled', '');
  document.body.classList.add('edit-mode');
  el('sim-hint').style.display = 'none';
  el('edit-hint').style.display = '';
  el('analysis-content').innerHTML = '<p class="hint">Exit edit mode to run analysis.</p>';
  el('btn-analyze').setAttribute('disabled', '');

  lastAnalyzedMarking = null;
  syncEditBuilderGraph();
  updateEditSelectionUi();
}

function exitEditMode(): void {
  editMode = false;
  lastSyncedEditNodeCount = -1;
  pauseEditCola();
  (el('layout-select') as HTMLSelectElement).disabled = false;
  el('btn-edit').textContent = 'Edit';
  el('btn-edit').classList.remove('active');
  document.body.classList.remove('edit-mode');
  el('sim-hint').style.display = '';
  el('edit-hint').style.display = 'none';
  cancelArcDraw();

  if (builder) {
    try {
      if (cy) {
        simLayoutSnapshot = captureLayoutFromCy();
      }
      const newSys = builder.build();
      sys?.free();
      sys = newSys;
      builder.free();
      builder = null;
      renderNet();
      el('btn-reset').removeAttribute('disabled');
      el('btn-analyze').removeAttribute('disabled');
      setStatus('');
    } catch (err) {
      setStatus(`Net invalid — ${err instanceof Error ? err.message : String(err)}`);
      // Stay in edit mode if the net is invalid
      editMode = true;
      el('btn-edit').textContent = 'Simulate';
      el('btn-edit').classList.add('active');
      document.body.classList.add('edit-mode');
      el('sim-hint').style.display = 'none';
      el('edit-hint').style.display = '';
    }
  }
}

/** Incrementally sync Cytoscape with the builder — preserves node positions and edit Cola. */
function syncEditBuilderGraph(): void {
  if (!builder || !cy) return;
  const s: WasmBuilderStructure = builder.structure();
  const captured = captureLayoutFromCy();

  const wantPlace = new Set(s.places.map((p) => `p${p.id}`));
  const wantTrans = new Set(s.transitions.map((t) => `t${t.id}`));
  const wantNodes = new Set([...wantPlace, ...wantTrans]);

  cy.nodes().forEach((n) => {
    const typ = n.data('type');
    if (typ === 'place' || typ === 'transition') {
      if (!wantNodes.has(n.id())) n.remove();
    }
  });

  cy.edges().remove();

  for (const p of s.places) {
    const id = `p${p.id}`;
    let pos = captured.get(id);
    if (!pos) {
      if (p.x !== 0 || p.y !== 0) {
        pos = { x: p.x, y: p.y };
      } else {
        pos = defaultViewportCenter();
      }
    }
    const label = placeDisplayLabel(p);
    const existing = cy.$(`#${id}`);
    if (existing.length > 0) {
      existing.data({
        name: p.name ?? defaultPlaceName(p.id),
        tokens: p.initial_tokens,
        label,
        index: p.id,
        type: 'place',
        enabled: false,
      });
    } else {
      cy.add({
        group: 'nodes',
        data: {
          id,
          type: 'place',
          index: p.id,
          name: p.name ?? defaultPlaceName(p.id),
          tokens: p.initial_tokens,
          label,
          enabled: false,
        },
        position: pos,
      });
    }
  }

  for (const t of s.transitions) {
    const id = `t${t.id}`;
    let pos = captured.get(id);
    if (!pos) {
      if (t.x !== 0 || t.y !== 0) {
        pos = { x: t.x, y: t.y };
      } else {
        pos = defaultViewportCenter();
      }
    }
    const name = t.name ?? defaultTransitionName(t.id);
    const existing = cy.$(`#${id}`);
    if (existing.length > 0) {
      existing.data({
        name,
        label: name,
        index: t.id,
        type: 'transition',
        enabled: false,
      });
    } else {
      cy.add({
        group: 'nodes',
        data: {
          id,
          type: 'transition',
          index: t.id,
          name,
          label: name,
          enabled: false,
        },
        position: pos,
      });
    }
  }

  for (const arc of s.pt_arcs) {
    cy.add({
      group: 'edges',
      data: {
        id: `arc_p${arc.source_id}_t${arc.target_id}`,
        source: `p${arc.source_id}`,
        target: `t${arc.target_id}`,
      },
    });
  }
  for (const arc of s.tp_arcs) {
    cy.add({
      group: 'edges',
      data: {
        id: `arc_t${arc.source_id}_p${arc.target_id}`,
        source: `t${arc.source_id}`,
        target: `p${arc.target_id}`,
      },
    });
  }

  updateEditStatus();

  const nodeCount = s.places.length + s.transitions.length;
  if (editMode && !arcDrawState && nodeCount > 0) {
    if (nodeCount !== lastSyncedEditNodeCount) {
      lastSyncedEditNodeCount = nodeCount;
      pauseEditCola();
      startEditCola();
    }
  }
  updateEditSelectionUi();
}

/** Lightweight status while editing — does not call `build()` (that runs only on Simulate). */
function updateEditStatus(): void {
  if (!builder) return;
  const p = builder.placeCount();
  const t = builder.transitionCount();
  if (p === 0 || t === 0) {
    setStatus(
      `Edit mode — ${p} place(s), ${t} transition(s). Add at least one place and one transition, then connect them with arcs.`,
    );
  } else {
    setStatus(`Edit mode — ${p} places, ${t} transitions. Press Simulate when the net is ready.`);
  }
}

function updateEditSelectionUi(): void {
  if (!editMode || !cy || !builder) return;
  const sel = cy.elements(':selected');
  const summary = el('edit-sel-summary');
  const tokensInput = el('edit-place-tokens') as HTMLInputElement;
  const btnApply = el('btn-apply-tokens') as HTMLButtonElement;
  const btnRev = el('btn-reverse-arc') as HTMLButtonElement;
  const btnDel = el('btn-delete-selection') as HTMLButtonElement;

  if (sel.length === 0) {
    summary.textContent = 'Nothing selected';
    tokensInput.disabled = true;
    btnApply.disabled = true;
    btnRev.disabled = true;
    btnDel.disabled = true;
    return;
  }

  const nodes = sel.nodes();
  const edges = sel.edges();
  const places = nodes.filter('[type="place"]');
  const transitions = nodes.filter('[type="transition"]');

  const parts: string[] = [];
  if (places.length > 0) parts.push(`${places.length} place(s)`);
  if (transitions.length > 0) parts.push(`${transitions.length} transition(s)`);
  if (edges.length > 0) parts.push(`${edges.length} arc(s)`);
  summary.textContent = parts.join(', ');

  if (places.length === 1 && transitions.length === 0 && edges.length === 0) {
    tokensInput.disabled = false;
    tokensInput.value = String(places.first().data('tokens') ?? 0);
    btnApply.disabled = false;
  } else {
    tokensInput.disabled = true;
    btnApply.disabled = true;
  }

  btnRev.disabled = !(edges.length === 1 && nodes.length === 0);
  btnDel.disabled = false;
}

function deleteEditSelection(): void {
  if (!editMode || !builder || !cy) return;
  const sel = cy.elements(':selected');
  if (sel.length === 0) return;
  const nodes = sel.nodes();
  const edges = sel.edges();
  if (nodes.length > 0) {
    const n = nodes.length;
    if (
      !window.confirm(
        n === 1 ? 'Remove this node and its incident arcs?' : `Remove ${n} nodes and their incident arcs?`,
      )
    ) {
      return;
    }
  }

  edges.forEach((ele) => {
    const arcId = ele.id();
    const mPt = arcId.match(/^arc_p(\d+)_t(\d+)$/);
    if (mPt) {
      builder!.removeArcPT(parseInt(mPt[1], 10), parseInt(mPt[2], 10));
      return;
    }
    const mTp = arcId.match(/^arc_t(\d+)_p(\d+)$/);
    if (mTp) {
      builder!.removeArcTP(parseInt(mTp[1], 10), parseInt(mTp[2], 10));
    }
  });
  nodes.forEach((ele) => {
    const typ = ele.data('type') as string;
    const id = ele.data('index') as number;
    if (typ === 'place') builder!.removePlace(id);
    else if (typ === 'transition') builder!.removeTransition(id);
  });
  syncEditBuilderGraph();
  updateEditSelectionUi();
}

function reverseSelectedArc(): void {
  if (!editMode || !builder || !cy) return;
  const edges = cy.edges(':selected');
  if (edges.length !== 1) return;
  const arcId = edges.first().id();
  const mPt = arcId.match(/^arc_p(\d+)_t(\d+)$/);
  if (mPt) {
    const p = parseInt(mPt[1], 10);
    const t = parseInt(mPt[2], 10);
    builder.removeArcPT(p, t);
    builder.addArcTP(t, p);
  } else {
    const mTp = arcId.match(/^arc_t(\d+)_p(\d+)$/);
    if (mTp) {
      const t = parseInt(mTp[1], 10);
      const p = parseInt(mTp[2], 10);
      builder.removeArcTP(t, p);
      builder.addArcPT(p, t);
    }
  }
  syncEditBuilderGraph();
  updateEditSelectionUi();
}

function applyTokensFromEditTools(): void {
  if (!editMode || !builder || !cy) return;
  const places = cy.nodes(':selected[type="place"]');
  if (places.length !== 1) return;
  const id = places.first().data('index') as number;
  const raw = (el('edit-place-tokens') as HTMLInputElement).value;
  const n = Math.max(0, parseInt(raw, 10) || 0);
  builder.setInitialTokens(id, n);
  syncEditBuilderGraph();
  updateEditSelectionUi();
}

// ---------------------------------------------------------------------------
// Arc drawing (right-click drag)
// ---------------------------------------------------------------------------

function ghostCanvas(): HTMLCanvasElement {
  return document.getElementById('arc-ghost') as HTMLCanvasElement;
}

function startArcDraw(node: NodeSingular): void {
  pauseEditCola();
  arcDrawState = {
    sourceId: node.id(),
    sourceType: node.data('type') as 'place' | 'transition',
  };
  const canvas = ghostCanvas();
  canvas.width = el('cy').clientWidth;
  canvas.height = el('cy').clientHeight;
  canvas.style.display = '';
  el('cy').appendChild(canvas);
  clearArcDrawHintClasses();
  node.addClass('arc-draw-source');
}

/** Rendered-space hit test (matches edge / node picking style). */
function isPointerOverNode(clientX: number, clientY: number, node: NodeSingular): boolean {
  const rect = el('cy').getBoundingClientRect();
  const rendered = { x: clientX - rect.left, y: clientY - rect.top };
  const nPos = node.renderedPosition();
  const hw = node.renderedOuterWidth() / 2 + 4;
  const hh = node.renderedOuterHeight() / 2 + 4;
  return Math.abs(nPos.x - rendered.x) <= hw && Math.abs(nPos.y - rendered.y) <= hh;
}

function clearGhostCanvas(): void {
  const canvas = ghostCanvas();
  canvas.style.display = 'none';
  const ctx = canvas.getContext('2d');
  if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
}

function clearArcDrawHintClasses(): void {
  if (!cy) return;
  cy.nodes().removeClass('arc-draw-source arc-draw-target arc-draw-invalid');
}

function validArcEndpoints(
  srcType: 'place' | 'transition',
  tgtType: string,
): boolean {
  return (
    (srcType === 'place' && tgtType === 'transition')
    || (srcType === 'transition' && tgtType === 'place')
  );
}

function updateArcGhost(clientX: number, clientY: number): void {
  if (!arcDrawState || !cy) return;
  const state = arcDrawState;
  const srcNode = cy.$(`#${state.sourceId}`).filter('node').first();
  if (srcNode.empty()) return;

  clearArcDrawHintClasses();
  srcNode.addClass('arc-draw-source');

  const canvas = ghostCanvas();
  const ctx = canvas.getContext('2d')!;
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  const rect = el('cy').getBoundingClientRect();
  const from = (srcNode as NodeSingular).renderedPosition();
  const to = { x: clientX - rect.left, y: clientY - rect.top };

  const srcSingular = srcNode as unknown as NodeSingular;
  let stroke = '#3b82f6';
  let previewNew = false;

  if (isPointerOverNode(clientX, clientY, srcSingular)) {
    stroke = '#ef4444';
  } else {
    const targetNode = cy.nodes().filter((n) => {
      if (n.id() === state.sourceId) return false;
      if (n.data('type') === 'token-ghost') return false;
      const nPos = n.renderedPosition();
      const hw = n.renderedOuterWidth() / 2 + 4;
      const hh = n.renderedOuterHeight() / 2 + 4;
      return Math.abs(nPos.x - to.x) <= hw && Math.abs(nPos.y - to.y) <= hh;
    }).first();

    if (!targetNode.empty()) {
      const tgtType = targetNode.data('type') as string;
      if (validArcEndpoints(state.sourceType, tgtType)) {
        targetNode.addClass('arc-draw-target');
        stroke = '#22c55e';
      } else {
        targetNode.addClass('arc-draw-invalid');
        stroke = '#f97316';
      }
    } else {
      previewNew = true;
    }
  }

  const lw = 1.5;
  ctx.beginPath();
  ctx.setLineDash(previewNew ? [6, 4] : []);
  ctx.strokeStyle = stroke;
  ctx.lineWidth = lw;
  ctx.lineCap = 'round';
  ctx.moveTo(from.x, from.y);
  ctx.lineTo(to.x, to.y);
  ctx.stroke();
  ctx.setLineDash([]);

  const angle = Math.atan2(to.y - from.y, to.x - from.x);
  const sz = 9;
  ctx.beginPath();
  ctx.moveTo(to.x, to.y);
  ctx.lineTo(to.x - sz * Math.cos(angle - Math.PI / 6), to.y - sz * Math.sin(angle - Math.PI / 6));
  ctx.lineTo(to.x - sz * Math.cos(angle + Math.PI / 6), to.y - sz * Math.sin(angle + Math.PI / 6));
  ctx.closePath();
  ctx.fillStyle = stroke;
  ctx.fill();

  if (previewNew && !isPointerOverNode(clientX, clientY, srcSingular)) {
    ctx.beginPath();
    ctx.strokeStyle = stroke;
    ctx.lineWidth = 1.5;
    ctx.setLineDash([4, 4]);
    ctx.arc(to.x, to.y, 14, 0, Math.PI * 2);
    ctx.stroke();
    ctx.setLineDash([]);
  }
}

function finishArcDraw(clientX: number, clientY: number): void {
  if (!arcDrawState || !builder || !cy) {
    arcDrawState = null;
    clearGhostCanvas();
    clearArcDrawHintClasses();
    if (!editColaLayout) startEditCola();
    return;
  }
  const state = arcDrawState;
  arcDrawState = null;
  clearGhostCanvas();
  clearArcDrawHintClasses();

  const srcNode = cy.$(`#${state.sourceId}`).first();
  if (srcNode.empty()) {
    if (!editColaLayout) startEditCola();
    return;
  }

  // Drop on source node → cancel (no new arc / node)
  const srcSingular = srcNode as unknown as NodeSingular;
  if (isPointerOverNode(clientX, clientY, srcSingular)) {
    if (!editColaLayout) startEditCola();
    return;
  }

  const rect = el('cy').getBoundingClientRect();
  const rendered = { x: clientX - rect.left, y: clientY - rect.top };

  const targetNode = cy.nodes().filter((n) => {
    if (n.id() === state.sourceId) return false;
    if (n.data('type') === 'token-ghost') return false;
    const nPos = n.renderedPosition();
    const hw = n.renderedOuterWidth() / 2 + 4;
    const hh = n.renderedOuterHeight() / 2 + 4;
    return Math.abs(nPos.x - rendered.x) <= hw && Math.abs(nPos.y - rendered.y) <= hh;
  }).first();

  const sourceBuilderID = parseInt(state.sourceId.slice(1), 10);
  const modelPos = clientToModelPos(clientX, clientY);

  if (!targetNode.empty()) {
    const targetType = targetNode.data('type') as string;
    const targetBuilderID = targetNode.data('index') as number;
    if (state.sourceType === 'place' && targetType === 'transition') {
      builder.addArcPT(sourceBuilderID, targetBuilderID);
    } else if (state.sourceType === 'transition' && targetType === 'place') {
      builder.addArcTP(sourceBuilderID, targetBuilderID);
    }
  } else {
    if (state.sourceType === 'place') {
      const newId = builder.addTransition(modelPos.x, modelPos.y, null);
      builder.addArcPT(sourceBuilderID, newId);
    } else {
      const newId = builder.addPlace(modelPos.x, modelPos.y, null);
      builder.addArcTP(sourceBuilderID, newId);
    }
  }

  syncEditBuilderGraph();
  if (!editColaLayout) startEditCola();
}

function cancelArcDraw(): void {
  arcDrawState = null;
  clearGhostCanvas();
  clearArcDrawHintClasses();
  if (!editColaLayout) startEditCola();
}

function clientToModelPos(clientX: number, clientY: number): { x: number; y: number } {
  if (!cy) return { x: 0, y: 0 };
  const rect = el('cy').getBoundingClientRect();
  const pan = cy.pan();
  const zoom = cy.zoom();
  return {
    x: (clientX - rect.left - pan.x) / zoom,
    y: (clientY - rect.top - pan.y) / zoom,
  };
}

// ---------------------------------------------------------------------------
// Edit mode event wiring
// ---------------------------------------------------------------------------

function setupEditMode(): void {
  if (!cy) return;
  const container = el('cy');

  // Right-click drag from node: start arc drawing
  cy.on('cxttapstart', 'node[type="place"], node[type="transition"]', (evt) => {
    if (!editMode) return;
    startArcDraw(evt.target as NodeSingular);
  });

  const onArcPointerMove = (evt: MouseEvent) => {
    if (arcDrawState) updateArcGhost(evt.clientX, evt.clientY);
  };
  const onArcPointerUp = (evt: MouseEvent) => {
    if (!editMode || !arcDrawState) return;
    if (evt.button === 2) finishArcDraw(evt.clientX, evt.clientY);
  };
  window.addEventListener('mousemove', onArcPointerMove);
  window.addEventListener('mouseup', onArcPointerUp);

  // Escape cancels arc drawing
  window.addEventListener('keydown', (evt) => {
    if (evt.key === 'Escape' && arcDrawState) cancelArcDraw();
  });

  cy.on('select unselect', () => {
    if (editMode) updateEditSelectionUi();
  });

  (el('btn-apply-tokens') as HTMLButtonElement).addEventListener('click', applyTokensFromEditTools);
  (el('btn-reverse-arc') as HTMLButtonElement).addEventListener('click', reverseSelectedArc);
  (el('btn-delete-selection') as HTMLButtonElement).addEventListener('click', deleteEditSelection);

  document.addEventListener('keydown', (evt) => {
    if (!editMode || !builder || !cy) return;
    if (evt.key !== 'Delete' && evt.key !== 'Backspace') return;
    const t = evt.target;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement) return;
    evt.preventDefault();
    deleteEditSelection();
  });

  // Double-click node in edit mode: combined rename + token editor
  cy.on('dblclick', 'node[type="place"], node[type="transition"]', (evt) => {
    if (!editMode || !builder) return;
    const node = evt.target as NodeSingular;
    const type = node.data('type') as string;
    const id = node.data('index') as number;

    if (type === 'place') {
      const newName = window.prompt(`Name for place (leave blank for default):`, node.data('name') as string);
      if (newName !== null) {
        builder.setPlaceName(id, newName);
        node.data('name', newName || defaultPlaceName(id));
      }
    } else {
      const newName = window.prompt(`Name for transition:`, node.data('name') as string);
      if (newName !== null) {
        builder.setTransitionName(id, newName);
      }
    }
    syncEditBuilderGraph();
  });

  // Sync drag positions back to builder
  cy.on('dragend', 'node', (evt) => {
    if (!editMode || !builder) return;
    const node = evt.target as NodeSingular;
    const type = node.data('type') as string;
    const id = node.data('index') as number;
    const pos = node.position();
    if (type === 'place') builder.setPlacePosition(id, pos.x, pos.y);
    else if (type === 'transition') builder.setTransitionPosition(id, pos.x, pos.y);
  });

  // Prevent browser context menu in edit mode
  container.addEventListener('contextmenu', (evt) => {
    if (editMode) evt.preventDefault();
  });
}

function setupAnimSlider(): void {
  const slider = el('anim-speed') as HTMLInputElement;
  const label = el('anim-speed-val');
  const update = () => {
    const v = Number(slider.value);
    label.textContent = v === 0 ? 'off' : `${v}ms`;
  };
  slider.addEventListener('input', update);
  update();
}

function animDuration(): number {
  return Number((el('anim-speed') as HTMLInputElement).value);
}

// Cytoscape styles

function netStyles(): cytoscape.StylesheetStyle[] {
  return [
    {
      selector: 'node[type="place"]',
      style: {
        shape: 'ellipse',
        width: 78,
        height: 78,
        'background-color': '#ffffff',
        'border-width': 2,
        'border-color': '#334155',
        label: 'data(label)',
        'text-valign': 'center',
        'text-halign': 'center',
        'text-wrap': 'wrap',
        'text-max-width': '104px',
        'line-height': 1.22,
        'font-size': 12,
        color: '#1e293b',
        'font-weight': 'bold',
        'min-zoomed-font-size': 4,
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="place"][tokens > 0]',
      style: { 'font-size': 13 } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="place"][tokens > 5]',
      style: { 'font-size': 14 } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="place"]:selected',
      style: {
        'border-color': '#3b82f6',
        'border-width': 3,
        'background-color': '#eff6ff',
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node.arc-draw-source',
      style: {
        'border-color': '#9333ea',
        'border-width': 4,
        'background-color': '#faf5ff',
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node.arc-draw-target',
      style: {
        'border-color': '#16a34a',
        'border-width': 4,
        'background-color': '#f0fdf4',
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node.arc-draw-invalid',
      style: {
        'border-color': '#ea580c',
        'border-width': 4,
        'background-color': '#fff7ed',
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="transition"]',
      style: {
        shape: 'rectangle',
        width: 22,
        height: 52,
        'background-color': '#94a3b8',
        'border-width': 0,
        label: 'data(label)',
        'text-valign': 'bottom',
        'text-margin-y': 6,
        'text-halign': 'center',
        'font-size': 11,
        color: '#475569',
        'min-zoomed-font-size': 4,
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="transition"][?enabled]',
      style: {
        'background-color': '#22c55e',
        color: '#15803d',
        cursor: 'pointer',
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="transition"]:selected',
      style: {
        'border-width': 2,
        'border-color': '#3b82f6',
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="token-ghost"]',
      style: {
        shape: 'ellipse',
        width: 16,
        height: 16,
        'background-color': '#1e293b',
        'border-width': 2,
        'border-color': '#475569',
        label: '',
        events: 'no',
        'z-index': 9999,
        'overlay-opacity': 0,
      } as cytoscape.Css.Node,
    },
    {
      selector: 'edge',
      style: {
        width: 1.5,
        'line-color': '#94a3b8',
        'target-arrow-color': '#94a3b8',
        'target-arrow-shape': 'triangle',
        'curve-style': 'bezier',
        'arrow-scale': 0.9,
      } as cytoscape.Css.Edge,
    },
    {
      selector: 'edge:selected',
      style: {
        'line-color': '#3b82f6',
        'target-arrow-color': '#3b82f6',
      } as cytoscape.Css.Edge,
    },
  ];
}

// Helpers

function el(id: string): HTMLElement {
  return document.getElementById(id)!;
}

function setStatus(msg: string): void {
  el('status').textContent = msg;
}

async function bootstrap(): Promise<void> {
  try {
    setStatus('Loading engine…');
    await initWasm();
    setStatus('');
    main();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error('petrivet-wasm init failed:', e);
    setStatus(`WASM failed to load: ${msg}`);
  }
}

void bootstrap();
