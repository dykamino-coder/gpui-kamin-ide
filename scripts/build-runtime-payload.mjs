// R2g-2 — assemble the embedded runtime payload for the single-exe Tauri build.
//
// The shipped app can't assume node/tsx or the repo are present, so the
// Rust shell embeds a self-contained runtime and unpacks it on first run
// (see src-tauri/src/sidecar.rs prod path). This script stages that
// runtime and compresses it to one archive the Rust binary `include_bytes!`s:
//
//   runtime/
//     node.exe              ← the system node we built/verified against
//     kamin-host.mjs        ← the standalone host bundle (build:host:tauri)
//     node_modules/         ← only the host's native/browser-field deps
//     builtin-extensions/   ← shipped VSIX-shaped builtins
//   → tar → zstd → src-tauri/payload/runtime.tar.zst
//
// Run after `build:host:tauri`. Wired into `beforeBuildCommand` (R2g-4).
//
// Incremental: the expensive work (npm install, copying ~80 MB node, zstd) is
// skipped when nothing it depends on changed — output is byte-identical, so
// this is safe for production AND keeps renderer-only rebuilds (which don't
// touch the payload) near-instant + lets cargo skip re-embedding the blob.
//   * runtime deps are installed into a persistent cache, reinstalled only when
//     their ranges / the node version change (`.deps-cache/.hash`).
//   * the final archive is gated on a hash of all its real inputs
//     (`runtime.tar.zst.hash`).
// Env knobs (DEV ONLY — production keeps the defaults):
//   KAMIN_PAYLOAD_ZSTD_LEVEL  zstd level (default 19; dev uses ~3 for speed)
//   KAMIN_PAYLOAD_FORCE=1     ignore the caches and rebuild everything

import { spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { dirname, join, relative } from "node:path"
import { fileURLToPath } from "node:url"
import { constants as zlibConstants, zstdCompressSync } from "node:zlib"

import { create as tarCreate } from "tar"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const payloadDir = join(root, "payload")
const stagingDir = join(payloadDir, ".staging")
const runtimeDir = join(stagingDir, "runtime")
const tarPath = join(stagingDir, "runtime.tar")
const outPath = join(payloadDir, "runtime.tar.zst")
const outHashPath = `${outPath}.hash`
// Persistent across builds — the runtime deps rarely change, so we cache the
// resolved node_modules and reinstall only when the dep ranges / node change.
const depsCacheDir = join(payloadDir, ".deps-cache")
const depsModules = join(depsCacheDir, "node_modules")
const depsHashPath = join(depsCacheDir, ".hash")

const hostBundle = join(root, "dist-host", "kamin-host.mjs")
const builtinSrc = join(root, "builtin-extensions")

// The host's only non-bundled deps (externals in the standalone Vite
// config): a native addon + two packages whose "browser" field would
// otherwise resolve to broken browser builds. node resolves these from
// the runtime's node_modules at launch.
const RUNTIME_DEPS = ["@homebridge/node-pty-prebuilt-multiarch", "ws", "chokidar"]
// zstd level 19: near-max ratio (smaller embedded blob → smaller exe). DEV
// builds drop this via env for speed; production keeps 19.
const ZSTD_LEVEL = Number(process.env.KAMIN_PAYLOAD_ZSTD_LEVEL) || 19
const FORCE = process.env.KAMIN_PAYLOAD_FORCE === "1"
const CHECK = process.argv.includes("--check")

function fail(message) {
  console.error(`[payload] ${message}`)
  process.exit(1)
}

function mb(bytes) {
  return `${(bytes / 1_048_576).toFixed(1)} MB`
}

/** Stable content hash of one file. */
function hashFile(p) {
  return createHash("sha256").update(readFileSync(p)).digest("hex")
}

/** Stable content hash of a directory tree (sorted relpath + bytes). */
function hashTree(dir) {
  const h = createHash("sha256")
  const walk = (d) => {
    for (const name of readdirSync(d).sort()) {
      const full = join(d, name)
      const st = statSync(full)
      if (st.isDirectory()) walk(full)
      else h.update(relative(dir, full)).update(readFileSync(full))
    }
  }
  walk(dir)
  return h.digest("hex")
}

// Pin the runtime deps to the exact ranges the app already builds against,
// so the shipped runtime can't drift from what dev/CI verified.
function resolveDepRanges() {
  const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"))
  const deps = manifest.dependencies ?? {}
  const out = {}
  for (const name of RUNTIME_DEPS) {
    const range = deps[name]
    if (!range) fail(`${name} missing from package.json dependencies`)
    out[name] = range
  }
  return out
}

/** Identity of the installed deps: their ranges + the node/platform they're for. */
function depsHash() {
  return createHash("sha256")
    .update(JSON.stringify(resolveDepRanges()))
    .update(process.version).update(process.platform).update(process.arch)
    .digest("hex")
}

// Fresh, dev-free install of just the runtime deps → a minimal node_modules
// with their transitive closure resolved correctly. Cached: reinstalled only
// when the dep ranges / node version change.
function ensureRuntimeDeps() {
  const want = depsHash()
  if (!FORCE && existsSync(depsModules) && existsSync(depsHashPath) && readFileSync(depsHashPath, "utf8") === want) {
    console.log("[payload] runtime deps cache hit — skip npm install")
    return
  }
  rmSync(depsCacheDir, { recursive: true, force: true })
  mkdirSync(depsCacheDir, { recursive: true })
  writeFileSync(
    join(depsCacheDir, "package.json"),
    `${JSON.stringify({ name: "kamin-runtime", private: true, dependencies: resolveDepRanges() }, null, 2)}\n`,
  )
  console.log("[payload] installing runtime deps (npm i --omit=dev)…")
  // Scripts stay ENABLED: node-pty-prebuilt-multiarch's install step lays
  // down the prebuilt .node binary for this platform.
  const result = spawnSync(
    "npm",
    ["install", "--omit=dev", "--no-audit", "--no-fund", "--no-package-lock"],
    { cwd: depsCacheDir, stdio: "inherit", shell: true },
  )
  if (result.status !== 0) fail("npm install failed for the runtime deps")
  pruneForeignPrebuilds(depsModules)
  writeFileSync(depsHashPath, want)
}

// node-pty-prebuilt-multiarch ships prebuilt binaries for every OS/arch
// under prebuilds/<platform>-<arch>; we only run on the host platform.
// (On Windows the live binary is build/Release/*.node — kept untouched —
// and prebuilds holds only the foreign Linux blobs, so all get dropped.)
function pruneForeignPrebuilds(modulesDir) {
  const ptyPrebuilds = join(modulesDir, "@homebridge", "node-pty-prebuilt-multiarch", "prebuilds")
  if (!existsSync(ptyPrebuilds)) return
  const keep = `${process.platform}-${process.arch}`
  let removed = 0
  for (const entry of readdirSync(ptyPrebuilds)) {
    if (entry === keep) continue
    rmSync(join(ptyPrebuilds, entry), { recursive: true, force: true })
    removed += 1
  }
  console.log(`[payload] pruned ${removed} foreign node-pty prebuild dir(s), kept ${keep}`)
}

function assembleRuntime() {
  mkdirSync(runtimeDir, { recursive: true })
  // Windows-first by design (project memory: WebKit on mac/Linux is a
  // later concern). The runtime ships the current `node` as `node.exe`,
  // and sidecar.rs spawns `node.exe` — both assume Windows. A mac/Linux
  // single-exe build would name this `node` and drop the `.exe` here.
  cpSync(process.execPath, join(runtimeDir, "node.exe"))
  cpSync(hostBundle, join(runtimeDir, "kamin-host.mjs"))
  cpSync(depsModules, join(runtimeDir, "node_modules"), { recursive: true })
  // Dev/test-only fixtures stay in the repo (loader.test.ts drives them) but are
  // NOT shipped — the production app loads ONLY the real builtin (claude-bridge).
  const NOT_SHIPPED = new Set(["hello-world", "welcome", "icon-theme-fixture"])
  cpSync(builtinSrc, join(runtimeDir, "builtin-extensions"), {
    recursive: true,
    filter: (src) => {
      const rel = relative(builtinSrc, src)
      if (!rel) return true // the builtin-extensions root itself
      const top = rel.split(/[\\/]/)[0]
      return !NOT_SHIPPED.has(top)
    },
  })
}

async function pack() {
  // `portable: true` strips uid/gid/mtime so the archive (and thus the
  // embedded bytes) is reproducible across machines.
  await tarCreate({ file: tarPath, cwd: runtimeDir, portable: true, gzip: false }, readdirSync(runtimeDir))
  const raw = readFileSync(tarPath)
  const compressed = zstdCompressSync(raw, {
    params: { [zlibConstants.ZSTD_c_compressionLevel]: ZSTD_LEVEL },
  })
  mkdirSync(payloadDir, { recursive: true })
  writeFileSync(outPath, compressed)
  console.log(`[payload] tar ${mb(raw.length)} → zstd-${String(ZSTD_LEVEL)} ${mb(compressed.length)}  ${outPath}`)
}

/** Hash of everything that determines the archive bytes. */
function payloadHash() {
  return createHash("sha256")
    .update(hashFile(hostBundle))
    .update(hashTree(builtinSrc))
    .update(depsHash())
    .update(`zstd:${String(ZSTD_LEVEL)}`)
    .update(`node:${process.version}`)
    .digest("hex")
}

async function main() {
  if (!existsSync(hostBundle)) fail("dist-host/kamin-host.mjs missing — run `npm run build:host:tauri` first")
  if (!existsSync(builtinSrc)) fail("builtin-extensions/ missing")

  const want = payloadHash()
  if (CHECK) {
    if (!existsSync(outPath) || !existsSync(outHashPath)) {
      fail("runtime payload missing — run `npm run build:payload`")
    }
    if (readFileSync(outHashPath, "utf8") !== want) {
      fail("runtime payload is stale — run `npm run build:payload`")
    }
    console.log(`[payload] verified current (${mb(statSync(outPath).size)})`)
    return
  }

  ensureRuntimeDeps()

  // Skip the expensive tar+zstd when the inputs are unchanged — the existing
  // archive is byte-identical, so cargo also skips re-embedding it.
  if (!FORCE && existsSync(outPath) && existsSync(outHashPath) && readFileSync(outHashPath, "utf8") === want) {
    console.log(`[payload] unchanged — skip pack (${mb(statSync(outPath).size)})`)
    return
  }

  rmSync(stagingDir, { recursive: true, force: true })
  mkdirSync(stagingDir, { recursive: true })
  assembleRuntime()
  await pack()
  writeFileSync(outHashPath, want)
  rmSync(stagingDir, { recursive: true, force: true })
  console.log(`[payload] done — ${mb(statSync(outPath).size)} embedded payload`)
}

main().catch((err) => fail(err?.stack ?? String(err)))
