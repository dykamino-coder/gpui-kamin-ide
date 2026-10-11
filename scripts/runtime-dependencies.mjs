// Select the runtime dependency closure from the checked npm lock, preserving
// exact versions, integrity hashes and nested Node resolution paths.
export const RUNTIME_DEPS = ["@homebridge/node-pty-prebuilt-multiarch", "ws", "chokidar"]

export function runtimeDependencyPlan(manifest, lock) {
  if (lock.lockfileVersion !== 3 || !lock.packages?.[""]) {
    throw new Error("runtime payload requires a v3 package-lock.json")
  }
  const root = lock.packages[""]
  const dependencies = {}
  const selected = new Map()

  function resolve(owner, name) {
    let directory = owner
    while (true) {
      const candidate = `${directory ? `${directory}/` : ""}node_modules/${name}`
      if (lock.packages[candidate]) return candidate
      if (!directory) return null
      const index = directory.lastIndexOf("node_modules/")
      directory = index < 0 ? "" : directory.slice(0, index).replace(/\/$/, "")
    }
  }

  function visit(location) {
    if (selected.has(location)) return
    const entry = lock.packages[location]
    if (entry.link || !entry.version || !entry.integrity || !entry.resolved) {
      throw new Error(`runtime dependency is not a locked registry package: ${location}`)
    }
    const copy = { ...entry }
    delete copy.dev
    delete copy.devOptional
    selected.set(location, copy)
    const edges = {
      ...entry.dependencies,
      ...entry.optionalDependencies,
      ...entry.peerDependencies,
    }
    for (const name of Object.keys(edges).sort()) {
      const target = resolve(location, name)
      const optional = Object.hasOwn(entry.optionalDependencies ?? {}, name)
        || entry.peerDependenciesMeta?.[name]?.optional === true
      if (!target) {
        if (!optional) throw new Error(`runtime lock is missing ${name} required by ${location}`)
        continue
      }
      visit(target)
    }
  }

  for (const name of RUNTIME_DEPS) {
    if (!manifest.dependencies?.[name]
      || manifest.dependencies[name] !== root.dependencies?.[name]) {
      throw new Error(`runtime manifest and lock disagree for ${name}; run npm install`)
    }
    const location = resolve("", name)
    if (!location) throw new Error(`runtime lock is missing ${name}`)
    dependencies[name] = lock.packages[location].version
    visit(location)
  }

  const runtimeManifest = { name: "kamin-runtime", private: true, dependencies }
  const packages = { "": runtimeManifest }
  for (const location of [...selected.keys()].sort()) packages[location] = selected.get(location)
  return {
    manifest: runtimeManifest,
    lock: { name: runtimeManifest.name, lockfileVersion: 3, requires: true, packages },
  }
}
