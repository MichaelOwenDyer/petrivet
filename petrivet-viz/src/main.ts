import { WasmSystem, WasmNetBuilder } from 'petrivet-wasm';
import type { WasmNetStructure, WasmPosition, WasmBuilderStructure } from 'petrivet-wasm';
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
// Marking vector at which analysis was last run, so we can show a stale badge
let lastAnalyzedMarking: number[] | null = null;

// Edit mode
let editMode = false;
let builder: WasmNetBuilder | null = null;

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

  cachedStructure = sys.netStructure();
  const s = cachedStructure;

  // Compute a position scale so PNML nodes don't visually overlap.
  const scale = pnmlPositionScale(s.place_positions, s.transition_positions);

  const elements: ElementDefinition[] = [];

  for (let i = 0; i < s.place_count; i++) {
    const pos = s.place_positions[i];
    elements.push({
      data: {
        id: `p${i}`, type: 'place', index: i,
        name: s.place_names[i] ?? `p${i}`,
        tokens: 0, label: '',
      },
      ...(pos != null ? { position: { x: pos.x * scale, y: pos.y * scale } } : {}),
    });
  }

  for (let i = 0; i < s.transition_count; i++) {
    const pos = s.transition_positions[i];
    elements.push({
      data: {
        id: `t${i}`, type: 'transition', index: i,
        name: s.transition_names[i] ?? `t${i}`,
        label: s.transition_names[i] ?? `t${i}`,
        enabled: false,
      },
      ...(pos != null ? { position: { x: pos.x * scale, y: pos.y * scale } } : {}),
    });
  }

  for (const arc of s.pt_arcs) {
    elements.push({ data: { source: `p${arc.source}`, target: `t${arc.target}` } });
  }
  for (const arc of s.tp_arcs) {
    elements.push({ data: { source: `t${arc.source}`, target: `p${arc.target}` } });
  }

  // Stop any running physics before replacing graph elements
  activeLayout?.stop();
  activeLayout = null;

  cy.elements().remove();
  cy.add(elements);

  const hasPositions = s.place_positions.some((p) => p != null);
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

// Marking sync

function syncMarking(): void {
  if (!sys || !cy) return;

  const marking = Array.from(sys.currentMarking());
  const enabled = new Set(Array.from(sys.enabledTransitions()));

  for (let i = 0; i < marking.length; i++) {
    const t = marking[i]!;
    cy.$(`#p${i}`).data('tokens', t).data('label', tokenLabel(t));
  }

  cy.$('node[type="transition"]').forEach((node) => {
    node.data('enabled', enabled.has(node.data('index') as number));
  });

  el('marking-row').textContent = `(${marking.join(', ')})`;
  el('deadlock-warn').textContent = sys.isDeadlocked() ? '⚠ Deadlocked' : '';

  // Show stale badge if the marking has changed since analysis was last run
  if (lastAnalyzedMarking) {
    const stale = marking.some((v, i) => v !== lastAnalyzedMarking![i]);
    el('analysis-stale').classList.toggle('visible', stale);
  }

  el('btn-reset').removeAttribute('disabled');
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

  //  Phase 1: tokens converge onto the transition
  await Promise.all(
    inputPlaces.map((pi) => {
      const from = cy!.$(`#p${pi}`).position();
      return moveGhost(from, transPos, duration);
    }),
  );

  // Fire (updates WASM state only — no visual change yet, so input tokens
  // stay visible while the bullets are "inside" the transition)
  sys.fire(transIdx);

  const nowEnabled = new Set(Array.from(sys.enabledTransitions()));
  transNode.data('enabled', nowEnabled.has(transIdx));

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
        refresh: 1,
        nodeSpacing: 20,
        edgeLength: 120,
        avoidOverlap: true,
        unconstrainedIterations: 10,
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

/** Apply the selected layout. For cola (physics) the simulation runs
 *  indefinitely until the user freezes it; all other layouts run once. */
function applyLayout(name: string, fitAfter = true): void {
  if (!cy) return;

  // Stop any previously running continuous layout
  activeLayout?.stop();
  activeLayout = null;

  const btnFreeze = el('btn-freeze') as HTMLButtonElement;
  const isCola = name === 'cola';
  btnFreeze.style.display = isCola ? '' : 'none';
  btnFreeze.textContent = '⏸';
  btnFreeze.title = 'Freeze physics';

  const layout = cy.layout(layoutOptions(name));

  if (isCola) {
    activeLayout = layout;
    if (fitAfter) {
      // Cola layoutstop fires only when stopped explicitly; fit after a short
      // delay once the spring forces have had time to spread the net out.
      setTimeout(() => cy?.fit(undefined, 60), 1200);
    }
  } else if (fitAfter) {
    layout.on('layoutstop', () => cy?.fit(undefined, 60));
  }

  layout.run();
}

function setupLayoutSelector(): void {
  const select = el('layout-select') as HTMLSelectElement;
  select.addEventListener('change', () => applyLayout(select.value));

  const btnFreeze = el('btn-freeze') as HTMLButtonElement;
  btnFreeze.addEventListener('click', () => {
    if (activeLayout) {
      // Freeze: stop the physics simulation
      activeLayout.stop();
      activeLayout = null;
      btnFreeze.textContent = '▶';
      btnFreeze.title = 'Resume physics';
    } else {
      // Resume: restart cola
      if (select.value === 'cola') {
        const layout = cy!.layout(layoutOptions('cola'));
        activeLayout = layout;
        layout.run();
        btnFreeze.textContent = '⏸';
        btnFreeze.title = 'Freeze physics';
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Edit mode
// ---------------------------------------------------------------------------

function enterEditMode(newNet: boolean): void {
  editMode = true;
  builder?.free();

  if (newNet || !sys) {
    builder = new WasmNetBuilder();
    sys?.free();
    sys = null;
  } else {
    builder = sys.toBuilder();
  }

  el('btn-edit').textContent = 'Simulate';
  el('btn-edit').classList.add('active');
  el('btn-reset').setAttribute('disabled', '');
  document.body.classList.add('edit-mode');
  el('sim-hint').style.display = 'none';
  el('edit-hint').style.display = '';
  el('analysis-content').innerHTML = '<p class="hint">Exit edit mode to run analysis.</p>';
  el('btn-analyze').setAttribute('disabled', '');

  lastAnalyzedMarking = null;
  renderBuilderNet();
}

function exitEditMode(): void {
  editMode = false;
  el('btn-edit').textContent = 'Edit';
  el('btn-edit').classList.remove('active');
  document.body.classList.remove('edit-mode');
  el('sim-hint').style.display = '';
  el('edit-hint').style.display = 'none';
  cancelArcDraw();

  if (builder) {
    try {
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

function renderBuilderNet(): void {
  if (!builder || !cy) return;
  const s: WasmBuilderStructure = builder.structure();

  const elements: ElementDefinition[] = [];

  for (const p of s.places) {
    elements.push({
      data: {
        id: `p${p.id}`,
        type: 'place',
        index: p.id,
        name: p.name ?? `p${p.id}`,
        tokens: p.initial_tokens,
        label: p.initial_tokens > 0 ? tokenLabel(p.initial_tokens) : '',
        enabled: false,
      },
      position: { x: p.x, y: p.y },
    });
  }

  for (const t of s.transitions) {
    elements.push({
      data: {
        id: `t${t.id}`,
        type: 'transition',
        index: t.id,
        name: t.name ?? `t${t.id}`,
        label: t.name ?? `t${t.id}`,
        enabled: false,
      },
      position: { x: t.x, y: t.y },
    });
  }

  for (const arc of s.pt_arcs) {
    elements.push({
      data: {
        id: `arc_p${arc.source_id}_t${arc.target_id}`,
        source: `p${arc.source_id}`,
        target: `t${arc.target_id}`,
      },
    });
  }

  for (const arc of s.tp_arcs) {
    elements.push({
      data: {
        id: `arc_t${arc.source_id}_p${arc.target_id}`,
        source: `t${arc.source_id}`,
        target: `p${arc.target_id}`,
      },
    });
  }

  activeLayout?.stop();
  activeLayout = null;

  // Preserve positions for existing nodes, add new ones at their stored pos.
  const existingIds = new Set(cy.elements().map((e) => e.id()));
  const newElements = elements.filter((e) => !existingIds.has(e.data.id as string));
  const removedIds = new Set(
    cy.elements().map((e) => e.id()).filter((id) => !elements.some((e) => e.data.id === id))
  );

  cy.elements().filter((e) => removedIds.has(e.id())).remove();
  if (newElements.length > 0) cy.add(newElements);

  // Update labels/tokens for existing nodes
  for (const p of s.places) {
    cy.$(`#p${p.id}`).data({
      tokens: p.initial_tokens,
      label: p.initial_tokens > 0 ? tokenLabel(p.initial_tokens) : '',
      name: p.name ?? `p${p.id}`,
    });
  }
  for (const t of s.transitions) {
    const label = t.name ?? `t${t.id}`;
    cy.$(`#t${t.id}`).data({ label, name: label });
  }

  tryBuildFromBuilder();
}

function tryBuildFromBuilder(): void {
  if (!builder) return;
  try {
    const newSys = builder.build();
    sys?.free();
    sys = newSys;
    setStatus(`Edit mode — ${builder.placeCount()} places, ${builder.transitionCount()} transitions`);
  } catch (err) {
    setStatus(`Edit mode — ${err instanceof Error ? err.message : String(err)}`);
  }
}

// ---------------------------------------------------------------------------
// Arc drawing (right-click drag)
// ---------------------------------------------------------------------------

function ghostCanvas(): HTMLCanvasElement {
  return document.getElementById('arc-ghost') as HTMLCanvasElement;
}

function startArcDraw(node: NodeSingular): void {
  arcDrawState = {
    sourceId: node.id(),
    sourceType: node.data('type') as 'place' | 'transition',
  };
  const canvas = ghostCanvas();
  canvas.width = el('cy').clientWidth;
  canvas.height = el('cy').clientHeight;
  canvas.style.display = '';
}

function updateArcGhost(clientX: number, clientY: number): void {
  if (!arcDrawState || !cy) return;
  const srcNode = cy.$(`#${arcDrawState.sourceId}`).nodes().first();
  if (srcNode.empty()) return;

  const canvas = ghostCanvas();
  const ctx = canvas.getContext('2d')!;
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  const rect = el('cy').getBoundingClientRect();
  const from = srcNode.renderedPosition();
  const to = { x: clientX - rect.left, y: clientY - rect.top };

  ctx.beginPath();
  ctx.setLineDash([6, 4]);
  ctx.strokeStyle = '#3b82f6';
  ctx.lineWidth = 2;
  ctx.moveTo(from.x, from.y);
  ctx.lineTo(to.x, to.y);
  ctx.stroke();

  // Arrowhead
  const angle = Math.atan2(to.y - from.y, to.x - from.x);
  const sz = 10;
  ctx.beginPath();
  ctx.setLineDash([]);
  ctx.moveTo(to.x, to.y);
  ctx.lineTo(to.x - sz * Math.cos(angle - Math.PI / 6), to.y - sz * Math.sin(angle - Math.PI / 6));
  ctx.lineTo(to.x - sz * Math.cos(angle + Math.PI / 6), to.y - sz * Math.sin(angle + Math.PI / 6));
  ctx.closePath();
  ctx.fillStyle = '#3b82f6';
  ctx.fill();
}

function finishArcDraw(clientX: number, clientY: number): void {
  if (!arcDrawState || !builder || !cy) { arcDrawState = null; return; }
  const state = arcDrawState;
  arcDrawState = null;
  cancelArcDraw();

  // Find node under cursor (excluding source)
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

  const sourceBuilderID = parseInt(state.sourceId.slice(1));
  const modelPos = clientToModelPos(clientX, clientY);

  if (!targetNode.empty()) {
    const targetType = targetNode.data('type') as string;
    const targetBuilderID = targetNode.data('index') as number;
    if (state.sourceType === 'place' && targetType === 'transition') {
      builder.addArcPT(sourceBuilderID, targetBuilderID);
    } else if (state.sourceType === 'transition' && targetType === 'place') {
      builder.addArcTP(sourceBuilderID, targetBuilderID);
    }
    // same-type drop: no-op
  } else {
    // Create new opposite-type node + arc
    if (state.sourceType === 'place') {
      const newId = builder.addTransition(modelPos.x, modelPos.y, null);
      builder.addArcPT(sourceBuilderID, newId);
    } else {
      const newId = builder.addPlace(modelPos.x, modelPos.y, null);
      builder.addArcTP(sourceBuilderID, newId);
    }
  }

  renderBuilderNet();
}

function cancelArcDraw(): void {
  arcDrawState = null;
  const canvas = ghostCanvas();
  canvas.style.display = 'none';
  const ctx = canvas.getContext('2d');
  if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
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

  // Track cursor for ghost arc
  container.addEventListener('mousemove', (evt) => {
    if (arcDrawState) updateArcGhost(evt.clientX, evt.clientY);
  });

  // End arc drawing on mouse-up anywhere
  container.addEventListener('mouseup', (evt) => {
    if (!editMode || !arcDrawState) return;
    if (evt.button === 2) finishArcDraw(evt.clientX, evt.clientY);
  });

  // Escape cancels arc drawing
  window.addEventListener('keydown', (evt) => {
    if (evt.key === 'Escape' && arcDrawState) cancelArcDraw();
  });

  // Right-click on empty canvas: add place (Shift = add transition)
  cy.on('cxttap', (evt) => {
    if (!editMode || arcDrawState) return;
    if (evt.target !== cy) return; // only on background
    const pos = (evt as cytoscape.EventObject & { position: { x: number; y: number } }).position;
    if ((evt.originalEvent as MouseEvent).shiftKey) {
      builder?.addTransition(pos.x, pos.y, null);
    } else {
      builder?.addPlace(pos.x, pos.y, null);
    }
    renderBuilderNet();
    cy!.fit(undefined, 60);
  });

  // Delete key: remove selected elements
  document.addEventListener('keydown', (evt) => {
    if (!editMode || !builder || !cy) return;
    if (evt.key !== 'Delete' && evt.key !== 'Backspace') return;
    const graph = cy;
    const selected = graph.elements(':selected');
    selected.forEach((ele) => {
      const eleAny = ele as cytoscape.SingularElementReturnValue;
      if (eleAny.isNode()) {
        const type = eleAny.data('type') as string;
        const id = eleAny.data('index') as number;
        if (type === 'place') builder!.removePlace(id);
        else if (type === 'transition') builder!.removeTransition(id);
      } else {
        // Edge — parse arc ID: arc_p{srcId}_t{tgtId} or arc_t{srcId}_p{tgtId}
        const arcId = (ele as cytoscape.EdgeSingular).id();
        const m = arcId.match(/^arc_([pt])(\d+)_([pt])(\d+)$/);
        if (m) {
          const [, aType, aId, , bId] = m;
          if (aType === 'p') builder!.removeArcPT(parseInt(aId), parseInt(bId));
          else builder!.removeArcTP(parseInt(aId), parseInt(bId));
        }
      }
    });
    renderBuilderNet();
  });

  // Double-click node in edit mode: combined rename + token editor
  cy.on('dblclick', 'node[type="place"], node[type="transition"]', (evt) => {
    if (!editMode || !builder) return;
    const node = evt.target as NodeSingular;
    const type = node.data('type') as string;
    const id = node.data('index') as number;

    if (type === 'place') {
      // Rename
      const newName = window.prompt(`Name for place (leave blank for default):`, node.data('name') as string);
      if (newName !== null) {
        builder.setPlaceName(id, newName);
        node.data('name', newName || `p${id}`);
      }
      // Tokens
      const input = window.prompt(`Initial tokens:`, String(node.data('tokens') as number));
      if (input !== null) {
        const n = Math.max(0, parseInt(input, 10) || 0);
        builder.setInitialTokens(id, n);
        node.data('tokens', n);
        node.data('label', n > 0 ? tokenLabel(n) : '');
      }
    } else {
      const newName = window.prompt(`Name for transition:`, node.data('name') as string);
      if (newName !== null) {
        builder.setTransitionName(id, newName);
        const label = newName || `t${id}`;
        node.data('name', label);
        node.data('label', label);
      }
    }
    tryBuildFromBuilder();
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
        width: 52,
        height: 52,
        'background-color': '#ffffff',
        'border-width': 2,
        'border-color': '#334155',
        label: 'data(label)',
        'text-valign': 'center',
        'text-halign': 'center',
        'font-size': 18,
        color: '#1e293b',
        'font-weight': 'bold',
        'min-zoomed-font-size': 4,
      } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="place"][tokens > 5]',
      style: { 'font-size': 20 } as cytoscape.Css.Node,
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

function tokenLabel(n: number): string {
  if (n === 0) return '';
  if (n <= 4) return '●'.repeat(n);
  return String(n);
}

function el(id: string): HTMLElement {
  return document.getElementById(id)!;
}

function setStatus(msg: string): void {
  el('status').textContent = msg;
}

main();
