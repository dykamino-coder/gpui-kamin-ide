// Сборка РАСКЛАДКИ дистрибутива GPUI-порта — `dist-installer/`.
//
// Инсталлер из неё делает `scripts/build_setup_rust.mjs` (свой распаковщик на
// Rust). NSIS отсюда удалён: его стаб ловился эвристиками антивирусов как
// дроппер (Kaspersky на 1.0.47) и мигал консолями.
//
// Раскладка (Runtime-режим шелла — см. crates/shell/src/host/connect.rs):
//   dist-installer/
//     kaminide-gpui.exe        ← target/release
//     libcef.dll + паки/локали ← CEF-набор из target/release
//     runtime/                 ← распакованный kamin-ide payload (kaminhost.exe
//                                = переименованный node, kamin-host.mjs,
//                                node_modules, builtin-extensions)
//
// Требует: cargo build --release уже прогнан; kamin-ide payload собран
// (npm run build:host:tauri && node scripts/build-runtime-payload.mjs).

import {
  cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync,
} from "node:fs"
import { spawnSync } from "node:child_process"
import { createRequire } from "node:module"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import { zstdDecompressSync } from "node:zlib"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const kaminIde = process.env.KAMIN_DEV_REPO ?? root
const release = join(root, "target", "release")
const dist = join(root, "dist-installer")
const payload = join(kaminIde, "payload", "runtime.tar.zst")
const version = /version\s*=\s*"([^"]+)"/.exec(readFileSync(join(root, "Cargo.toml"), "utf8"))[1]

if (!existsSync(join(release, "kaminide-gpui.exe"))) {
  console.error("нет target/release/kaminide-gpui.exe — прогони cargo build --release")
  process.exit(1)
}
// exe обязан быть НОВЕЕ Cargo.toml: инцидент 1.0.5 — cargo упал по сети
// после бампа версии, инсталлер молча запаковал СТАРЫЙ бинарь (юзер ставил
// «1.0.5», получал 1.0.4). Свежесть = единственный дешёвый инвариант.
{
  const { statSync } = await import("node:fs")
  const exeM = statSync(join(release, "kaminide-gpui.exe")).mtimeMs
  const tomlM = statSync(join(root, "Cargo.toml")).mtimeMs
  if (exeM < tomlM) {
    console.error(`target/release/kaminide-gpui.exe СТАРШЕ Cargo.toml (бамп версии не собран) — прогони cargo build --release`)
    process.exit(1)
  }
}

if (!existsSync(payload)) {
  console.error(`нет ${payload} — прогони npm run build:payload`)
  process.exit(1)
}
const payloadCheck = spawnSync(
  process.execPath,
  [join(kaminIde, "scripts", "build-runtime-payload.mjs"), "--check"],
  { cwd: kaminIde, stdio: "inherit" },
)
if (payloadCheck.status !== 0) {
  console.error("runtime payload не соответствует текущим host/builtin sources")
  process.exit(1)
}

rmSync(dist, { recursive: true, force: true })
mkdirSync(dist, { recursive: true })

// Шелл + CEF-набор. Списком, не глобом — чтобы случайный мусор из
// target/release (pdb, тестовые exe) не уехал в инсталлер.
const shipFiles = [
  "kaminide-gpui.exe",
  "libcef.dll", "chrome_elf.dll", "icudtl.dat",
  "chrome_100_percent.pak", "chrome_200_percent.pak", "resources.pak",
  "v8_context_snapshot.bin", "snapshot_blob.bin",
  "libEGL.dll", "libGLESv2.dll",
  "d3dcompiler_47.dll", "dxcompiler.dll", "dxil.dll",
  "vk_swiftshader.dll", "vk_swiftshader_icd.json", "vulkan-1.dll",
]
let copied = 0
for (const f of shipFiles) {
  const src = join(release, f)
  if (existsSync(src)) { cpSync(src, join(dist, f)); copied++ }
}
for (const d of ["locales", "Resources"]) {
  const src = join(release, d)
  if (existsSync(src)) cpSync(src, join(dist, d), { recursive: true })
}
console.log(`[installer] shell+CEF: ${copied} файлов`)

// runtime/ из payload-архива kamin-ide (staging после сборки удалён — берём
// сам runtime.tar.zst, это и есть прод-артефакт).
const tarBuf = zstdDecompressSync(readFileSync(payload))
const tarTmp = join(root, "runtime.tar.tmp")
writeFileSync(tarTmp, tarBuf)
const requireKamin = createRequire(join(kaminIde, "package.json"))
const tar = requireKamin("tar")
// Архив хранит СОДЕРЖИМОЕ runtime/ без префикса — распаковываем в подкаталог.
mkdirSync(join(dist, "runtime"), { recursive: true })
await tar.extract({ file: tarTmp, cwd: join(dist, "runtime") })
rmSync(tarTmp)
// Анти-`taskkill /IM node.exe`: как в Tauri-проде, node переименован.
const nodeExe = join(dist, "runtime", "node.exe")
if (existsSync(nodeExe)) renameSync(nodeExe, join(dist, "runtime", "kaminhost.exe"))
console.log("[installer] runtime/ распакован")

console.log(`[installer] dist-installer/ собран, версия ${version}`)
