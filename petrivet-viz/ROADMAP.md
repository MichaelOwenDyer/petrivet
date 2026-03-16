# petrivet-viz — Roadmap

Items are sorted within each tier by **value ÷ difficulty** (highest first).
A ⚙ prefix marks items that require a change in the `petrivet` or `petrivet-wasm`
crates before the UI work can begin.

---

## Tier 1 — Quick wins  _(high value, low effort)_

- [x] **Token animation** — ghost bullets travel input → transition → output;
      speed controlled by slider (0 = instant).
- [x] **Animation: seamless merge** — tokens remain visible on input places
      throughout the animation; all places update atomically when output
      bullets land, eliminating the brief disappearance between phases.
- [x] **Marking notation** — display as `(m₀, m₁, …)` per the theoretical
      convention instead of `[…]`.
- [x] **Net class info icon** — clicking ℹ next to the net class shows a
      plain-language description of the structural constraints and what
      analysis methods become available.
- [ ] **Place name display** — place names are currently only shown when a
      node is clicked (visible in the toolbar). Show them persistently as a
      small label below each circle, separately from the token-count label
      inside. _(Cytoscape compound-node or ghost-label approach; no new dep.)_
- [ ] **Node label toggle** — long node labels crowd dense nets.
      Add a toolbar toggle to show/hide all node labels.
- [ ] **Analysis on marking change** — currently *Analyze* is manual. Run it
      automatically in a Web Worker so it is non-blocking and always reflects
      the current marking. _(Blocked on browser Web Worker + WASM thread-safety;
      requires `wasm-bindgen` with `--target no-modules` or `SharedArrayBuffer`
      + Atomics setup — see Tier 7.)_

---

## Tier 2 — UX polish  _(good value, moderate effort)_

- [ ] **Elastic / physics layout mode** — a toolbar toggle that starts a live
      force-directed layout (`cose` with `infinite: true` or the `cola`
      plugin). Nodes bounce apart naturally. A "Freeze" button halts the
      simulation so the user can lock a layout for export.
      _(Optional dep: `cytoscape-cola` for better physics; otherwise use
      built-in `cose`.)_
- [ ] **Per-transition liveness display** — after analysis, colour-code
      transitions: L0 dead (dark red), L1 (orange), L3 (yellow), L4 live
      (green outline). Shown as a second style layer, toggleable.
- [ ] **Per-place bound display** — after boundedness analysis, show each
      place's upper bound as a small badge. "ω" for unbounded.
- [ ] **PNML drag-and-drop feedback** — highlight the canvas with a drop
      indicator while a file is being dragged over it.
- [ ] **Keyboard shortcuts** — `R` to reset, `F` to fit, `Space` to fire a
      selected enabled transition.

---

## Tier 3 — Net editing  _(very high value, significant effort)_

This tier enables the "hot-reload as you edit" vision.

### Rust / WASM prerequisites

- [ ] ⚙ **`WasmNetBuilder`** — expose `NetBuilder` through a `#[wasm_bindgen]`
      wrapper so JS can add/remove places, transitions, and arcs and get back
      a new `WasmSystem`. Updating the initial marking interactively should
      also be supported (click a place, type a new token count).

### UI work (unblocked once `WasmNetBuilder` exists)

- [ ] **Right-click drag to create nodes** — right-drag from a **place**
      creates a new transition at the drop point with a P→T arc; right-drag
      from a **transition** creates a new place with a T→P arc. Releasing back
      over the originating node cancels.
- [ ] **Arc creation between existing nodes** — same gesture but releasing
      over an existing compatible node (place→transition or transition→place)
      adds an arc instead of a new node.
- [ ] **Delete nodes / arcs** — select one or more elements, press Delete.
      Deleting a place or transition also removes its incident arcs.
- [ ] **Arc direction reversal** — middle-click an arc to reverse its
      direction in-place (P→T becomes T→P and vice-versa, swapping endpoints).
- [ ] **Inline marking edit** — double-click a place to edit its initial
      token count directly on the canvas. Triggers re-analysis immediately.
- [ ] **Auto-analysis on every edit** — after any structural change or
      initial-marking change, re-run all analyses and update the sidebar
      panel without any user action required ("hot-reload").

---

## Tier 4 — PNML export  _(high value, moderate Rust effort)_

- [ ] ⚙ **PNML serialisation in `petrivet`** — implement `to_xml()` on
      `PnmlDocument` / `Net`, writing back all structural data plus the
      `<graphics>` blocks for positions.
- [ ] ⚙ **`WasmSystem::toPnml()`** — expose serialisation through the WASM
      wrapper, returning a UTF-8 XML string.
- [ ] **Export button** — "Save PNML" in the toolbar triggers a browser
      download of the current net with its current Cytoscape layout positions
      written into the `<graphics>` elements, so the layout round-trips
      exactly.

---

## Tier 5 — State space exploration  _(very high value, complex)_

The vision: the state space graph is itself a graph of interactive Petri nets.
Clicking any marking node in the state space switches the main canvas to that
marking, so you can explore reachability interactively.

### Rust / WASM prerequisites

- [ ] ⚙ **Edge-walking API in `petrivet`** — add methods to
      `CoverabilityGraph` and `ReachabilityGraph` that iterate over edges as
      `(src_marking_index, transition, dst_marking_index)` triples. This
      unlocks structured access to the graph topology from outside the crate.
- [ ] ⚙ **`WasmSystem::coverabilityGraph()` / `reachabilityGraph()`** —
      return a typed JS object `{ nodes: WasmOmega[][], edges: { src, t, dst }[] }`
      so the browser can render and interact with the state space.

### UI work

- [ ] **State space panel** — a second Cytoscape canvas (side-by-side or as a
      resizable split pane) showing the full reachability / coverability graph.
      Each node is labelled with its marking tuple.
- [ ] **Click to navigate** — clicking a node in the state space panel calls
      `sys.setMarking(marking)` (needs WASM) to jump the main canvas to that
      exact marking without reset.
- [ ] **Current state highlight** — the node corresponding to the current
      marking in the main canvas is always highlighted in the state space
      panel, moving as you fire transitions.
- [ ] **State space minimap** — optionally, show the state space as a minimap
      overlay in the corner of the main canvas, so both views are always
      visible simultaneously.
- [ ] ⚙ **`WasmSystem::setMarking()`** — WASM method to teleport the
      system to an arbitrary reachable marking.

---

## Tier 6 — Advanced analysis UI  _(moderate value, deferred)_

- [ ] **Reachability / coverability query** — a panel where the user can
      specify a target marking (by editing a token count per place, using the
      current marking as m₀) and run `analyzeReachability` /
      `analyzeCoverability`. Display the firing sequence witness or the proof
      of unreachability.  
      _(Design is still open — see discussion.)_
- [ ] **Firing sequence replay** — given a firing sequence (e.g., from a
      reachability proof), animate each step in order on the canvas.

---

## Tier 7 — Infrastructure  _(foundational, deferred until complexity warrants)_

- [ ] **Web Worker for analysis** — move all `sys.isBounded()` /
      `sys.isLive()` / `sys.isDeadlockFree()` calls off the main thread.
      Requires either (a) `--target no-modules` WASM build + `importScripts`
      in a Worker, or (b) enabling `SharedArrayBuffer` (needs COOP/COEP
      headers) so the same WASM memory is shared. Once done, analysis can run
      continuously and update the sidebar without ever freezing the UI.
- [ ] **Analysis performance: avoid redundant state-space exploration** — the
      three current analysis calls (`isBounded`, `isLive`, `isDeadlockFree`)
      each independently build the coverability/reachability graph. A single
      combined `analyzeAll()` WASM entry point that reuses one exploration
      pass would cut the work to roughly one third. For the 6-philosopher
      dining problem the analysis currently triggers two "page unresponsive"
      browser warnings; this fix should reduce that to at most one brief
      pause until the Web Worker solution lands.
- [ ] **Framework migration (Svelte)** — the current vanilla-TS approach is
      fine for the viewer. Once the net editor and state space panel add
      significant reactive state, migrating to Svelte will reduce boilerplate
      considerably. Vite already supports Svelte via a plugin; the migration
      is incremental.
- [ ] **Component split** — regardless of framework, split `main.ts` into
      logical modules: `canvas.ts`, `analysis.ts`, `animation.ts`,
      `file-io.ts`.

---

## Implementation notes

**Why no DOT rendering?**  
The state space and the net topology are both rendered as interactive
Cytoscape canvases rather than SVG from Graphviz. This keeps the UI unified
and "live" — nodes are clickable, draggable, and animated. DOT export from
`WasmSystem.toDot()` is still available for sending nets to external tools.

**Coordinate system for PNML positions**  
PNML files from standard editors (PIPE, GreatSPN) use ≈30 px nodes. We scale
positions up by `90 / minPairwiseDist` (clamped 1.5×–6×) so our larger nodes
never overlap. This is computed fresh on each file load and stored nowhere.

**WASM state invariant**  
`WasmSystem` holds the net topology immutably (`Rc<Net>`) and the current
marking mutably. All analysis methods operate on the *current* marking.
`reset()` restores the current marking to the initial marking without
rebuilding the net.
