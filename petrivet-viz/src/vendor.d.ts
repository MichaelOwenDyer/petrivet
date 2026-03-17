// Minimal module declarations for Cytoscape layout extensions that ship
// without TypeScript types. cytoscape.use() accepts `any`, so these stubs
// are enough to let the compiler resolve the imports.
declare module 'cytoscape-fcose' {
  const ext: (cy: unknown) => void;
  export default ext;
}

declare module 'cytoscape-cola' {
  const ext: (cy: unknown) => void;
  export default ext;
}

declare module 'cytoscape-dagre' {
  const ext: (cy: unknown) => void;
  export default ext;
}
