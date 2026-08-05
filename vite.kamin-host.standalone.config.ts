import { builtinModules } from "node:module"
import { defineConfig } from "vite"

// Standalone production bundle of kamin-host for the native GPUI shell.
//
// Dev runs the TS source under `tsx`; the shipped app can't assume tsx or
// the repo are present, so we bundle the host into one ESM file run under a
// plain `node.exe`. node-pty stays external (its prebuilt `.node` binary
// can't be bundled and is shipped alongside in node_modules); everything
// else pure-JS (ws, chokidar, …) is inlined.
export default defineConfig({
  build: {
    outDir: "dist-host",
    emptyOutDir: true,
    target: "node22",
    minify: false,
    lib: {
      entry: "src/kamin-host/kamin-host.ts",
      formats: ["es"],
      fileName: () => "kamin-host.mjs",
    },
    rollupOptions: {
      // One self-contained file (the payload ships + the Rust shell spawns a
      // single `kamin-host.mjs`). The role dispatcher's dynamic imports of
      // host-main/child are inlined; only the taken branch's boot runs, so the
      // child never executes the parent's service boot (and vice-versa).
      output: { inlineDynamicImports: true },
      external: [
        // Native (.node binary) + packages with a "browser" field Vite
        // would otherwise resolve to their browser build (ws → a stub
        // whose WebSocketServer isn't a constructor). Shipped in the
        // runtime's node_modules and resolved by node at runtime.
        "@homebridge/node-pty-prebuilt-multiarch",
        "ws",
        "chokidar",
        ...builtinModules,
        ...builtinModules.map((m) => `node:${m}`),
      ],
    },
  },
})
