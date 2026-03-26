import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
/** Sibling crate: `petrivet-wasm/pkg` is linked from package.json; dev server must read it. */
const petrivetWasmPkg = path.resolve(__dirname, '../petrivet-wasm/pkg');

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  server: {
    fs: {
      allow: [__dirname, petrivetWasmPkg],
    },
  },
});
