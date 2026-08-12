// Build the two decomposed Bridge webview bundles. vite-plugin-singlefile sets
// inlineDynamicImports (single-chunk), which forbids multiple inputs in ONE
// rollup pass — so we run a separate single-input build per entry, then copy
// each inlined html into builtin-extensions/claude-bridge/ (the shipped artifact).
import { build } from "vite"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const dir = path.dirname(fileURLToPath(import.meta.url))
const dest = path.resolve(dir, "../../../builtin-extensions/claude-bridge")

// Decomposed webview entries that ship: chat (center tool) + tools (side panel:
// sub-agents + plan + todos) + customize (Bridge's Settings/Skills/MCP/… block,
// rendered inside KaminIDE's own Customize area). KaminIDE's native sessions
// replace the Bridge sidebar (see docs/BRIDGE_VSIX_INTEGRATION.md).
for (const entry of ["chat", "tools", "customize"]) {
  await build({
    configFile: path.resolve(dir, "vite.config.ts"),
    root: dir,
    build: {
      outDir: path.resolve(dir, `dist/${entry}`),
      emptyOutDir: true,
      rollupOptions: { input: path.resolve(dir, `${entry}.html`) },
    },
  })
  const builtPath = path.resolve(dir, `dist/${entry}/${entry}.html`)
  let html = fs.readFileSync(builtPath, "utf8")
  // esbuild emits Highlight.js' PHP whitespace character class as a multiline
  // template literal containing a real space + TAB before the newline. A
  // generic trailing-whitespace cleanup then silently changes `[ \t\n]` into
  // `[\n]`. Store the equivalent escaped JS string in the shipped artifact so
  // formatting tools cannot alter its runtime value.
  html = html.split("`[ \t\n]`").join('"[ \\t\\n]"')
  fs.writeFileSync(path.join(dest, `${entry}.html`), html, "utf8")
  console.log(`[claude-bridge] webview → ${entry}.html`)
}
