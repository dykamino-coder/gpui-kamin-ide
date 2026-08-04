// Собрать собственный Rust-инсталлер: crate kaminide-setup + приклеить payload
// (tar.zst каталога dist-installer) с футером [len:u64 LE]["KMNSETUP"].
// Замена NSIS (мигал консолями + ловился AV-эвристиками).
//
// Порядок:
//   1. build_installer.mjs с KAMIN_ASSEMBLE_ONLY=1 — собирает dist-installer/
//      (шелл+CEF+runtime+builtin-extensions), NSIS не гонит.
//   2. cargo build -p kaminide-setup — стаб-exe без payload.
//   3. Копия стаба → dist-installer/kaminide-setup.exe (нужен для /uninstall).
//   4. tar.zst(dist-installer) приклеить к свежей копии стаба + футер.
// Выход: KaminIDE_<version>_x64-setup.exe в корне.

import { execFileSync } from "node:child_process"
import { readFileSync, writeFileSync, statSync, copyFileSync, rmSync } from "node:fs"
import { join, dirname } from "node:path"
import { fileURLToPath } from "node:url"
import { create as tarCreate } from "tar"
import { zstdCompressSync, constants as zc } from "node:zlib"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const version = /version\s*=\s*"([^"]+)"/.exec(readFileSync(join(root, "Cargo.toml"), "utf8"))[1]
const dist = join(root, "dist-installer")
const stub = join(root, "target", "release", "kaminide-setup.exe")
const out = join(root, `KaminIDE_${version}_x64-setup.exe`)
const MAGIC = Buffer.from("KMNSETUP")

// 1. Собрать dist-installer/ (проверенная сборка из build_installer.mjs).
console.log("[setup] assembling dist-installer/ …")
execFileSync("node", [join(root, "scripts", "build_installer.mjs")], {
  stdio: "inherit",
  env: { ...process.env, KAMIN_ASSEMBLE_ONLY: "1" },
})

// 2. Собрать стаб.
console.log("[setup] building kaminide-setup crate …")
execFileSync("cargo", ["build", "--release", "--offline", "-p", "kaminide-setup"], {
  stdio: "inherit",
  cwd: root,
})
// Guard: стаб не старше Cargo.toml (та же логика, что у NSIS-гварда).
if (statSync(stub).mtimeMs < statSync(join(root, "Cargo.toml")).mtimeMs) {
  console.error("[setup] kaminide-setup.exe СТАРШЕ Cargo.toml — cargo build не пересобрал")
  process.exit(1)
}

// 3. Копия стаба в dist для /uninstall (без payload — распакованный exe в
//    каталоге установки только удаляет).
copyFileSync(stub, join(dist, "kaminide-setup.exe"))

// 4. tar.zst каталога dist-installer.
console.log("[setup] tar+zstd dist-installer/ …")
const tarTmp = join(root, "setup-payload.tar")
await tarCreate({ file: tarTmp, cwd: dist, portable: true }, ["."])
const tarBuf = readFileSync(tarTmp)
rmSync(tarTmp)
const zst = zstdCompressSync(tarBuf, { params: { [zc.ZSTD_c_compressionLevel]: 19 } })
console.log(`[setup] payload ${(tarBuf.length / 1e6).toFixed(1)}MB → zstd ${(zst.length / 1e6).toFixed(1)}MB`)

// 5. Приклеить: [stub][zst][len u64 LE][MAGIC].
const footer = Buffer.alloc(16)
footer.writeBigUInt64LE(BigInt(zst.length), 0)
MAGIC.copy(footer, 8)
writeFileSync(out, Buffer.concat([readFileSync(stub), zst, footer]))
console.log(`[setup] готово: ${out} (${(statSync(out).size / 1e6).toFixed(1)}MB)`)
