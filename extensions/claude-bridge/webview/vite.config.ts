import { fileURLToPath } from "node:url"
import preact from "@preact/preset-vite"
import { defineConfig, normalizePath } from "vite"
import { viteSingleFile } from "vite-plugin-singlefile"
import { buildSync } from 'esbuild'

const previewReact = normalizePath(fileURLToPath(new URL('./src/browser/preview-react.ts', import.meta.url)))
// Only the deferred preview renderer uses development React. Bundle its entire
// React/DOM/JSX/Refresh graph with one NODE_ENV before Vite's production define
// can select a different build for any member of that graph.
export const developmentPreview = {
  name: 'development-preview-react',
  enforce: 'pre' as const,
  load(id: string) {
    if (id !== previewReact) return
    return buildSync({ entryPoints: [previewReact], bundle: true, write: false,
      format: 'esm', platform: 'browser', target: 'es2022',
      define: { 'process.env.NODE_ENV': '"development"' },
    }).outputFiles[0].text
  },
}

// Desktop entries are self-contained HTML copied into builtin-extensions.
// Browser assets retain a deferred renderer chunk and are served by the host.
export default defineConfig(({ mode }) => ({
  // Browser startup must install its private transport before renderer modules
  // execute. Inlining dynamic imports would eagerly evaluate those modules.
  plugins: mode === 'browser' ? [developmentPreview, preact({ reactAliasesEnabled: false })] : [preact(), viteSingleFile()],
  resolve: {
    alias: {
      // Persisted KV for the webview UI (replaces the renderer's localStorage,
      // forbidden in the sandboxed webview iframe — see src/lib/storage.ts).
      "@bridge/storage": fileURLToPath(new URL("./src/lib/storage.ts", import.meta.url)),
    },
  },
  build: {
    target: "es2022",
    cssCodeSplit: false,
    assetsInlineLimit: 100_000_000, // inline everything
    reportCompressedSize: false,
  },
}))
