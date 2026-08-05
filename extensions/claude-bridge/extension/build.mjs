// Bundle the Claude Bridge extension host → builtin-extensions/claude-bridge/
// extension.js (CJS, the runtime artifact). The ported handlers import the
// explicit `@kaminide/host-compat` facade; `vscode` and ws's optional native
// addons stay external (resolved by the extension host / ws's own try/catch).
import esbuild from "esbuild"
import path from "node:path"
import { fileURLToPath } from "node:url"

const dir = path.dirname(fileURLToPath(import.meta.url))
const hostCompat = path.resolve(dir, "src/shim/host-compat.ts")
const outfile = path.resolve(dir, "../../../builtin-extensions/claude-bridge/extension.js")

await esbuild.build({
  entryPoints: [path.resolve(dir, "src/index.ts")],
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node18",
  outfile,
  external: ["vscode", "kaminide", "bufferutil", "utf-8-validate"],
  alias: { "@kaminide/host-compat": hostCompat },
  logLevel: "info",
})

console.log(`[claude-bridge] bundled → ${outfile}`)
