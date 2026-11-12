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

// Без него остановка команды на Windows молча откатывается на taskkill и
// оставляет MSYS-потомков (INC-2026-0096) — такой инсталлер не собираем.
if (!existsSync(join(release, "kamin-jobrun.exe"))) {
  console.error("нет target/release/kamin-jobrun.exe — прогони cargo build --release")
  process.exit(1)
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
  // Запускает команды инструментов в собственном Job Object (crates/jobrun,
  // INC-2026-0096). Лежит рядом с runtime/, расширение ищет его там.
  "kamin-jobrun.exe",
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
// ripgrep для инструмента Grep (Bridge): на Windows его почти никогда нет в
// PATH, и без него Grep шёл JS-фолбэком — в ~40 раз медленнее на большом
// репозитории и без glob/type/.gitignore. Версия и SHA256 закреплены; архив
// кэшируется в target/, чтобы повторная сборка не ходила в сеть.
{
  const rg = {
    url: "https://github.com/BurntSushi/ripgrep/releases/download/14.1.1/ripgrep-14.1.1-x86_64-pc-windows-msvc.zip",
    dir: "ripgrep-14.1.1-x86_64-pc-windows-msvc",
    sha256: "d0f534024c42afd6cb4d38907c25cd2b249b79bbe6cc1dbee8e3e37c2b6e25a1",
  }
  const { createHash } = await import("node:crypto")
  const cacheDir = join(root, "target", "ripgrep")
  const zip = join(cacheDir, `${rg.dir}.zip`)
  const digest = (buf) => createHash("sha256").update(buf).digest("hex")
  mkdirSync(cacheDir, { recursive: true })
  if (!existsSync(zip) || digest(readFileSync(zip)) !== rg.sha256) {
    const res = await fetch(rg.url)
    if (!res.ok) {
      console.error(`ripgrep: загрузка ${rg.url} не удалась (${res.status})`)
      process.exit(1)
    }
    const buf = Buffer.from(await res.arrayBuffer())
    if (digest(buf) !== rg.sha256) {
      console.error(`ripgrep: SHA256 архива ${digest(buf)} не совпадает с закреплённым ${rg.sha256}`)
      process.exit(1)
    }
    writeFileSync(zip, buf)
  }
  const unpacked = join(cacheDir, rg.dir)
  rmSync(unpacked, { recursive: true, force: true })
  // bsdtar из Windows 10+ читает zip.
  const untar = spawnSync("tar", ["-xf", zip, "-C", cacheDir], { stdio: "inherit" })
  if (untar.status !== 0 || !existsSync(join(unpacked, "rg.exe"))) {
    console.error("ripgrep: не удалось распаковать rg.exe")
    process.exit(1)
  }
  // Рядом с kamin-jobrun.exe: расширение ищет `<install>\rg.exe`.
  cpSync(join(unpacked, "rg.exe"), join(dist, "rg.exe"))
  cpSync(join(unpacked, "LICENSE-MIT"), join(dist, "ripgrep-LICENSE-MIT.txt"))
  console.log("[installer] ripgrep 14.1.1: rg.exe")
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
