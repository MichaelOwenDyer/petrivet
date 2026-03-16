import { WasmSystem } from 'petrivet-wasm';
import cytoscape from 'cytoscape';
import type { Core, ElementDefinition, NodeSingular } from 'cytoscape';

let sys: WasmSystem | null = null;
let cy: Core | null = null;

function main(): void {
  cy = cytoscape({
    container: document.getElementById('cy')!,
    style: netStyles(),
    layout: { name: 'preset' },
    minZoom: 0.1,
    maxZoom: 4,
  });

  cy.on('tap', 'node[type="transition"]', (evt) => {
    if (!sys) return;
    const fired = sys.fire(evt.target.data('index') as number);
    if (fired) syncMarking();
  });

  cy.on('tap', 'node', (evt) => showNodeInfo(evt.target as NodeSingular));
  cy.on('tap', (evt) => { if (evt.target === cy) clearNodeInfo(); });

  setupFileInput();
  setupDropzone();
  setupToolbar();
}

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

// Net rendering (full rebuild)
function renderNet(): void {
  if (!sys || !cy) return;

  const s = sys.netStructure();
  const elements: ElementDefinition[] = [];

  for (let i = 0; i < s.place_count; i++) {
    const pos = s.place_positions[i];
    elements.push({
      data: {
        id: `p${i}`,
        type: 'place',
        index: i,
        name: s.place_names[i] ?? `p${i}`,
        tokens: 0,
        label: '',
      },
      ...(pos ? { position: { x: pos.x, y: pos.y } } : {}),
    });
  }

  for (let i = 0; i < s.transition_count; i++) {
    const pos = s.transition_positions[i];
    elements.push({
      data: {
        id: `t${i}`,
        type: 'transition',
        index: i,
        name: s.transition_names[i] ?? `t${i}`,
        label: s.transition_names[i] ?? `t${i}`,
        enabled: false,
      },
      ...(pos ? { position: { x: pos.x, y: pos.y } } : {}),
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

  const hasPositions = s.place_positions.some((p) => p !== null);
  if (hasPositions) {
    cy.layout({ name: 'preset' }).run();
  } else {
    cy.layout({
      name: 'cose',
      animate: false,
      padding: 60,
      nodeRepulsion: () => 4096,
      idealEdgeLength: () => 80,
    } as Parameters<Core['layout']>[0]).run();
  }

  cy.fit(undefined, 60);
  syncMarking();
  updateAnalysisPanel();

  const netName = s.net_name ?? 'Untitled net';
  document.title = `petrivet — ${netName}`;
  el('net-name').textContent = netName;
}

// Marking sync (called after every fire / reset)
function syncMarking(): void {
  if (!sys || !cy) return;

  const marking = Array.from(sys.currentMarking());
  const enabled = new Set(Array.from(sys.enabledTransitions()));

  for (let i = 0; i < marking.length; i++) {
    const tokens = marking[i]!;
    cy.$(`#p${i}`)
      .data('tokens', tokens)
      .data('label', tokenLabel(tokens));
  }

  cy.$('node[type="transition"]').forEach((node) => {
    node.data('enabled', enabled.has(node.data('index') as number));
  });

  const isDeadlocked = sys.isDeadlocked();
  el('marking-row').textContent = `[${marking.join(', ')}]`;

  const warn = el('deadlock-warn');
  if (isDeadlocked) {
    if (!warn) {
      const d = document.createElement('div');
      d.id = 'deadlock-warn';
      d.className = 'deadlock-warn';
      d.textContent = '⚠ Deadlocked';
      el('marking-row').after(d);
    }
  } else {
    warn?.remove();
  }

  el('btn-reset').removeAttribute('disabled');
}

// Analysis panel
function updateAnalysisPanel(): void {
  if (!sys) return;

  const s = sys.netStructure();
  const bounded = sys.isBounded();
  const live = sys.isLive();
  const dlFree = sys.isDeadlockFree();

  el('analysis-content').innerHTML = `
    <div class="prop"><span>Class</span><span>${s.net_class}</span></div>
    <div class="prop"><span>Places</span><span>${s.place_count}</span></div>
    <div class="prop"><span>Transitions</span><span>${s.transition_count}</span></div>
    <div class="prop"><span>Bounded</span><span>${yesNo(bounded)}</span></div>
    <div class="prop"><span>Live (L4)</span><span>${yesNo(live)}</span></div>
    <div class="prop"><span>Deadlock-free</span><span>${yesNo(dlFree)}</span></div>
  `;
}

function yesNo(b: boolean): string {
  return b ? '<span style="color:#4ade80">yes</span>' : '<span style="color:#f87171">no</span>';
}

function showNodeInfo(node: NodeSingular): void {
  const d = node.data() as Record<string, unknown>;
  if (d.type === 'place') {
    el('node-info').textContent =
      `Place: ${String(d.name)}  ·  tokens: ${Number(d.tokens)}`;
  } else if (d.type === 'transition') {
    const state = d.enabled ? 'enabled' : 'disabled';
    el('node-info').textContent =
      `Transition: ${String(d.name)}  (${state})`;
  }
}

function clearNodeInfo(): void {
  el('node-info').textContent = '';
}

function setupFileInput(): void {
  const input = el('file-input') as HTMLInputElement;
  input.addEventListener('change', async () => {
    const file = input.files?.[0];
    if (file) loadPnml(await file.text());
    input.value = '';
  });
}

function setupDropzone(): void {
  const canvas = el('cy');
  canvas.addEventListener('dragover', (e) => e.preventDefault());
  canvas.addEventListener('drop', async (e) => {
    e.preventDefault();
    const file = (e as DragEvent).dataTransfer?.files[0];
    if (file?.name.endsWith('.pnml')) loadPnml(await file.text());
  });
}

function setupToolbar(): void {
  el('btn-open').addEventListener('click', () => el('file-input').click());

  el('btn-reset').addEventListener('click', () => {
    sys?.reset();
    syncMarking();
  });

  el('btn-fit').addEventListener('click', () => cy?.fit(undefined, 60));
}

function netStyles(): cytoscape.StylesheetStyle[] {
  return [
    {
      selector: 'node[type="place"]',
      style: {
        shape: 'ellipse',
        width: 54,
        height: 54,
        'background-color': '#ffffff',
        'border-width': 2,
        'border-color': '#334155',
        label: 'data(label)',
        'text-valign': 'center',
        'text-halign': 'center',
        'font-size': 13,
        color: '#1e293b',
        'font-weight': 'bold',
      } as cytoscape.Css.Node,
    },
    {
      // Larger font for numeric counts (tokens > 5 means label is a number string)
      selector: 'node[type="place"][tokens > 5]',
      style: { 'font-size': 18 } as cytoscape.Css.Node,
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
        width: 26,
        height: 54,
        'background-color': '#94a3b8',
        'border-width': 0,
        label: 'data(label)',
        'text-valign': 'bottom',
        'text-margin-y': 6,
        'text-halign': 'center',
        'font-size': 11,
        color: '#475569',
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
      selector: 'node[type="transition"]:active',
      style: { 'background-color': '#16a34a' } as cytoscape.Css.Node,
    },
    {
      selector: 'node[type="transition"]:selected',
      style: {
        'border-width': 2,
        'border-color': '#3b82f6',
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

function tokenLabel(n: number): string {
  if (n === 0) return '';
  if (n <= 5) return '●'.repeat(n);
  return String(n);
}

function el(id: string): HTMLElement {
  return document.getElementById(id)!;
}

function setStatus(msg: string): void {
  el('status').textContent = msg;
}

main();
