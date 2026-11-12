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
import { crc32, inflateRawSync, zstdDecompressSync } from "node:zlib"

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
    url: "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-x86_64-pc-windows-msvc.zip",
    dir: "ripgrep-15.2.0-x86_64-pc-windows-msvc",
    sha256: "71b2fef860abe467217a538ff31de02f5258807c0129f771846f87bd029aafc5",
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
  // Распаковка средствами node: внешний `tar` зависит от PATH (GNU tar из
  // Git принимает `D:\...` за удалённый хост), а архив уже проверен по SHA256.
  const files = unzipEntries(readFileSync(zip), {
    [`${rg.dir}/rg.exe`]: "rg.exe",
    [`${rg.dir}/LICENSE-MIT`]: "ripgrep-LICENSE-MIT.txt",
  })
  // Рядом с kamin-jobrun.exe: расширение ищет `<install>\rg.exe`.
  for (const [name, data] of Object.entries(files)) writeFileSync(join(dist, name), data)
  console.log("[installer] ripgrep 15.2.0: rg.exe")
}

/** Извлекает из zip перечисленные записи (stored или deflate) по central
 *  directory. Возвращает { целевое имя: Buffer }; без всех записей — выход. */
function unzipEntries(buf, wanted) {
  const fail = (why) => {
    console.error(`ripgrep: не удалось распаковать архив: ${why}`)
    process.exit(1)
  }
  let eocd = buf.length - 22
  while (eocd >= 0 && buf.readUInt32LE(eocd) !== 0x06054b50) eocd--
  if (eocd < 0) fail("нет конца central directory")
  const count = buf.readUInt16LE(eocd + 10)
  let at = buf.readUInt32LE(eocd + 16)
  const out = {}
  for (let i = 0; i < count; i++) {
    if (buf.readUInt32LE(at) !== 0x02014b50) fail("повреждена central directory")
    const method = buf.readUInt16LE(at + 10)
    const crc = buf.readUInt32LE(at + 16)
    const packed = buf.readUInt32LE(at + 20)
    const size = buf.readUInt32LE(at + 24)
    const nameLen = buf.readUInt16LE(at + 28)
    const extraLen = buf.readUInt16LE(at + 30)
    const commentLen = buf.readUInt16LE(at + 32)
    const local = buf.readUInt32LE(at + 42)
    const name = buf.toString("utf8", at + 46, at + 46 + nameLen)
    at += 46 + nameLen + extraLen + commentLen
    const target = wanted[name]
    if (!target) continue
    if (buf.readUInt32LE(local) !== 0x04034b50) fail(`нет локального заголовка ${name}`)
    const start = local + 30 + buf.readUInt16LE(local + 26) + buf.readUInt16LE(local + 28)
    const raw = buf.subarray(start, start + packed)
    const data = method === 0 ? Buffer.from(raw) : method === 8 ? inflateRawSync(raw) : fail(`метод ${method} в ${name}`)
    if (data.length !== size || crc32(data) !== crc) fail(`контрольная сумма ${name}`)
    out[target] = data
  }
  for (const [name, target] of Object.entries(wanted)) if (!out[target]) fail(`нет ${name}`)
  return out
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
