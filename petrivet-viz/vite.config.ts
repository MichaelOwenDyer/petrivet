import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
/** Sibling crate: `petrivet-wasm/pkg` is linked from package.json; dev server must read it. */
const petrivetWasmPkg = path.resolve(__dirname, '../petrivet-wasm/pkg');

/** GitHub project Pages URL is `https://<user>.github.io/<repo>/` — set `VITE_BASE=/repo/` in CI. */
function viteBase(): string {
  const raw = (process.env.VITE_BASE ?? '/').trim();
  if (raw === '' || raw === '/') return '/';
  return raw.endsWith('/') ? raw : `${raw}/`;
}

export default defineConfig({
  base: viteBase(),
  plugins: [wasm(), topLevelAwait()],
  server: {
    fs: {
      allow: [__dirname, petrivetWasmPkg],
    },
  },
});
