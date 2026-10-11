// The embedded runtime must reproduce tested resolutions, including nested
// and optional dependency edges; ranges alone cannot establish that identity.
import assert from "node:assert/strict"
import test from "node:test"
import { RUNTIME_DEPS, runtimeDependencyPlan } from "../runtime-dependencies.mjs"

function fixture() {
  const dependencies = Object.fromEntries(RUNTIME_DEPS.map((name) => [name, "^1.0.0"]))
  const packages = { "": { dependencies } }
  for (const name of RUNTIME_DEPS) packages[`node_modules/${name}`] = entry("1.0.1")
  return { manifest: { dependencies }, lock: { lockfileVersion: 3, packages } }
}

function entry(version, extra = {}) {
  return { version, resolved: `https://registry.npmjs.org/example/${version}.tgz`,
    integrity: `sha512-${version}`, ...extra }
}

test("runtime roots are exact locked versions and unrelated dev packages stay out", () => {
  const { manifest, lock } = fixture()
  lock.packages["node_modules/test-only"] = entry("9.0.0", { dev: true })
  const plan = runtimeDependencyPlan(manifest, lock)
  assert.equal(plan.manifest.dependencies.ws, "1.0.1")
  assert.equal(plan.lock.packages["node_modules/test-only"], undefined)
  assert.equal(plan.lock.packages[""], plan.manifest)
})

test("nested dependency resolution retains the checked version and integrity", () => {
  const { manifest, lock } = fixture()
  lock.packages["node_modules/ws"].dependencies = { shared: "^2" }
  lock.packages["node_modules/shared"] = entry("1.0.0")
  lock.packages["node_modules/ws/node_modules/shared"] = entry("2.0.0")
  const plan = runtimeDependencyPlan(manifest, lock)
  assert.equal(plan.lock.packages["node_modules/ws/node_modules/shared"].version, "2.0.0")
  assert.equal(plan.lock.packages["node_modules/ws/node_modules/shared"].integrity, "sha512-2.0.0")
  assert.equal(plan.lock.packages["node_modules/shared"], undefined)
})

test("transitive and optional locked peers enter the runtime identity", () => {
  const { manifest, lock } = fixture()
  lock.packages["node_modules/ws"].peerDependencies = { optional: "*", absent: "*" }
  lock.packages["node_modules/ws"].peerDependenciesMeta = {
    optional: { optional: true }, absent: { optional: true },
  }
  lock.packages["node_modules/optional"] = entry("1.0.0", { dev: true })
  const before = runtimeDependencyPlan(manifest, lock)
  assert.equal(before.lock.packages["node_modules/optional"].dev, undefined)
  lock.packages["node_modules/optional"].version = "1.0.1"
  assert.notEqual(JSON.stringify(before), JSON.stringify(runtimeDependencyPlan(manifest, lock)))
})

test("manifest drift and missing required dependency fail before installing", () => {
  const { manifest, lock } = fixture()
  manifest.dependencies = { ...manifest.dependencies, ws: "^2.0.0" }
  assert.throws(() => runtimeDependencyPlan(manifest, lock), /manifest and lock disagree/)
  manifest.dependencies = lock.packages[""].dependencies
  lock.packages["node_modules/ws"].dependencies = { missing: "*" }
  assert.throws(() => runtimeDependencyPlan(manifest, lock), /missing required by/)
})

test("unlocked local package edges cannot masquerade as verified registry dependencies", () => {
  const { manifest, lock } = fixture()
  lock.packages["node_modules/ws"] = { link: true, resolved: "../unverified" }
  assert.throws(() => runtimeDependencyPlan(manifest, lock), /not a locked registry package/)
})
