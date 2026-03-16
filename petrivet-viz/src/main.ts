import { WasmSystem } from 'petrivet-wasm';
import type { WasmNetStructure, WasmPosition } from 'petrivet-wasm';
import cytoscape from 'cytoscape';
import type { Core, ElementDefinition, NodeSingular } from 'cytoscape';

// App state

let sys: WasmSystem | null = null;
let cy: Core | null = null;
let cachedStructure: WasmNetStructure | null = null;
let animating = false;

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
    if (!sys || animating) return;
    const idx = evt.target.data('index') as number;
    if (sys.enabledTransitions().includes(idx)) {
      void animateFire(idx);
    }
  });

  cy.on('tap', 'node', (evt) => showNodeInfo(evt.target as NodeSingular));
  cy.on('tap', (evt) => { if (evt.target === cy) clearNodeInfo(); });

  setupFileInput();
  setupDropzone();
  setupToolbar();
  setupAnimSlider();
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

  cy.elements().remove();
  cy.add(elements);

  const hasPositions = s.place_positions.some((p) => p != null);
  if (hasPositions) {
    cy.layout({ name: 'preset' }).run();
  } else {
    cy.layout({
      name: 'cose',
      animate: false,
      padding: 80,
      nodeRepulsion: () => 8192,
      idealEdgeLength: () => 100,
      gravity: 0.25,
    } as Parameters<Core['layout']>[0]).run();
  }

  cy.fit(undefined, 60);
  syncMarking();
  updateAnalysisPanel();

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

  const isDeadlocked = sys.isDeadlocked();
  el('marking-row').textContent = `[${marking.join(', ')}]`;

  const existing = el('deadlock-warn');
  if (isDeadlocked && !existing) {
    const d = document.createElement('div');
    d.id = 'deadlock-warn';
    d.className = 'deadlock-warn';
    d.textContent = '⚠ Deadlocked';
    el('marking-row').after(d);
  } else if (!isDeadlocked) {
    existing?.remove();
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

  //  Fire & update inputs
  sys.fire(transIdx);
  const marking = Array.from(sys.currentMarking());
  for (const pi of inputPlaces) {
    const t = marking[pi]!;
    cy.$(`#p${pi}`).data('tokens', t).data('label', tokenLabel(t));
  }

  //  Flash transition
  transNode.addClass('firing');
  await delay(duration * 0.2);
  transNode.removeClass('firing');

  //  Phase 2: new tokens emanate from the transition
  await Promise.all(
    outputPlaces.map((pi) => {
      const to = cy!.$(`#p${pi}`).position();
      return moveGhost(transPos, to, duration);
    }),
  );

  //  Update outputs & final sync
  for (const pi of outputPlaces) {
    const t = marking[pi]!;
    cy.$(`#p${pi}`).data('tokens', t).data('label', tokenLabel(t));
  }

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

function delay(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// Analysis panel

function updateAnalysisPanel(): void {
  if (!sys || !cachedStructure) return;
  const s = cachedStructure;
  el('analysis-content').innerHTML = `
    <div class="prop"><span>Class</span><span>${s.net_class}</span></div>
    <div class="prop"><span>Places</span><span>${s.place_count}</span></div>
    <div class="prop"><span>Transitions</span><span>${s.transition_count}</span></div>
    <div class="prop"><span>Bounded</span><span>${yesNo(sys.isBounded())}</span></div>
    <div class="prop"><span>Live (L4)</span><span>${yesNo(sys.isLive())}</span></div>
    <div class="prop"><span>Deadlock-free</span><span>${yesNo(sys.isDeadlockFree())}</span></div>
  `;
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
  el('btn-reset').addEventListener('click', () => {
    if (animating) return;
    sys?.reset();
    syncMarking();
  });
  el('btn-fit').addEventListener('click', () => cy?.fit(undefined, 60));
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
      selector: 'node[type="transition"].firing',
      style: { 'background-color': '#facc15' } as cytoscape.Css.Node,
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
