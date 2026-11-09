import { fileURLToPath } from "node:url"
import preact from "@preact/preset-vite"
import { defineConfig } from "vite"
import { viteSingleFile } from "vite-plugin-singlefile"

// Desktop entries are self-contained HTML copied into builtin-extensions.
// Browser assets retain a deferred renderer chunk and are served by the host.
export default defineConfig(({ mode }) => ({
  // Browser startup must install its private transport before renderer modules
  // execute. Inlining dynamic imports would eagerly evaluate those modules.
  plugins: mode === 'browser' ? [preact({ reactAliasesEnabled: false })] : [preact(), viteSingleFile()],
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
