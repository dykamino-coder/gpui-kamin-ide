// Сборка kaminide-setup: cargo-бинарь + приклейка payload
// (tar.zst каталога dist-installer) с футером [len:u64 LE]["KMNSETUP"].
// Выход: KaminIDE_<version>_x64-setup.exe в корне репо (как у NSIS-эпохи).
import { execFileSync } from "node:child_process"
import { appendFileSync, copyFileSync, readFileSync, statSync, writeFileSync, mkdirSync, rmSync } from "node:fs"
import { join, dirname } from "node:path"
import { fileURLToPath } from "node:url"

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const version = /version = "([^"]+)"/.exec(readFileSync(join(root, "Cargo.toml"), "utf-8"))[1]
const dist = join(root, "dist-installer")
const exeSrc = join(root, "target", "release", "kaminide-setup.exe")
const out = join(root, `KaminIDE_${version}_x64-setup.exe`)

// Guard: dist-installer должен быть свежее Cargo.toml? Нет — dist свежесть
// гарантирует вызывающий конвейер; здесь проверяем только наличие главного exe.
statSync(join(dist, "kaminide-gpui.exe"))

console.log("[setup] cargo build -p kaminide-setup --release")
execFileSync("cargo", ["build", "--release", "--offline", "-p", "kaminide-setup"], {
  cwd: root, stdio: "inherit",
})

// Инсталлер кладёт СЕБЯ в дистрибутив (UninstallString указывает на него).
copyFileSync(exeSrc, join(d