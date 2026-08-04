// Bundle the Claude Bridge extension host → builtin-extensions/claude-bridge/
// extension.js (CJS, the runtime artifact). esbuild aliases every `electron`
// import in the vendored main-process code to src/shim/electron.ts (the
// ipcMain-over-postMessage shim); `vscode` and ws's optional native addons stay
// external (resolved at runtime by the ext-host / ws's own try/catch).
import esbuild from "esbuild"
import path from "node:path"
import { fileURLToPath } from "node:url"

const dir = path.dirname(fileURLToPath(import.meta.url))
const shim = path.resolve(dir, "src/shim/electron.ts")
const outfile = path.resolve(dir, "../../../builtin-extensions/claude-bridge/extension.js")

await esbuild.build({
  entryPoints: [path.resolve(dir, "src/index.ts")],
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node18",
  outfile,
  external: ["vscode", "kaminide", "bufferutil", "utf-8-validate"],
  alias: { electron: shim },
  logLevel: "info",
})

console.log(`[claude-bridge] bundled → ${outfile}`)
