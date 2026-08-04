import { fileURLToPath } from "node:url"
import preact from "@preact/preset-vite"
import { defineConfig } from "vite"
import { viteSingleFile } from "vite-plugin-singlefile"

// Build a SINGLE self-contained index.html (JS + CSS inlined). KaminIDE webviews
// serve over the kaminwebview:// custom protocol with a permissive CSP, so inline
// scripts run (proven by hello-world). The build script copies dist/index.html →
// builtin-extensions/claude-bridge/webview.html, the only artifact that ships.
export default defineConfig({
  plugins: [preact(), viteSingleFile()],
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
})
