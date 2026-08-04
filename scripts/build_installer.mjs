// Сборка дистрибутива + NSIS-инсталлера GPUI-порта.
//
// Раскладка (Runtime-режим шелла — см. crates/shell/src/host/connect.rs):
//   dist-installer/
//     kaminide-gpui.exe        ← target/release
//     libcef.dll + паки/локали ← CEF-набор из target/release
//     runtime/                 ← распакованный kamin-ide payload (kaminhost.exe
//                                = переименованный node, kamin-host.mjs,
//                                node_modules, builtin-extensions)
//   → makensis → KaminIDE_<ver>_x64-setup.exe
//
// Требует: cargo build --release уже прогнан; kamin-ide payload собран
// (npm run build:host:tauri && node scripts/build-runtime-payload.mjs).

import { spawnSync } from "node:child_process"
import {
  cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync,
} from "node:fs"
import { createRequire } from "node:module"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import { zstdDecompressSync } from "node:zlib"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const kaminIde = process.env.KAMIN_DEV_REPO ?? root
const release = join(root, "target", "release")
const dist = join(root, "dist-installer")
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
const payload = join(kaminIde, "payload", "runtime.tar.zst")
if (!existsSync(payload)) {
  console.error(`нет ${payload} — прогони npm run build:payload`)
  process.exit(1)
}
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

// NSIS (MUI2): нормальный визард — welcome/директория/прогресс/finish с
// запуском, иконка, полные поля в «Приложениях», закрытие запущенного
// приложения перед установкой (апгрейд поверх живого).
const appIcon = join(root, "assets", "app", "icon.ico").replaceAll("/", "\\\\")
// Брендинг: тёмная header-битмапа с логотипом (жалоба «стандартный серый») —
// генерится из icon.ico PowerShell'ом, лежит в assets/app.
const headerBmp = join(root, "assets", "app", "installer-header.bmp").replaceAll("/", "\\\\")
const uninstKey = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\KaminIDE-GPUI"
const nsi = `
!define APP "KaminIDE"
!define VER "${version}"
Unicode true
Name "\${APP} \${VER}"
OutFile "${join(root, `KaminIDE_${version}_x64-setup.exe`).replaceAll("/", "\\\\")}"
InstallDir "$LOCALAPPDATA\\Programs\\KaminIDE-GPUI"
; Обновлять ТУ ЖЕ папку, что и прошлая установка (юзер мог выбрать свою) —
; иначе апгрейд ставится рядом, а ярлыки/пины продолжают запускать старую.
InstallDirRegKey HKCU "Software\\KaminIDE-GPUI" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"
!insertmacro GetSize
!insertmacro GetParameters
!insertmacro GetOptions

; Журнал установки (диагностика тихих провалов самообновления):
; последовательность шагов в $TEMP\\kaminide-install.log, append.
!macro KLOG text
  FileOpen $9 "$TEMP\\kaminide-install.log" a
  FileSeek $9 0 END
  FileWrite $9 "[\${VER}] \${text}$\\r$\\n"
  FileClose $9
!macroend

Function .onInit
  ; Побег из Job приложения. Апдейтер ≤1.0.16 спавнит нас ВНУТРИ своего Job
  ; (KILL_ON_JOB_CLOSE без BREAKAWAY_OK): наш же taskkill приложения закрывал
  ; Job и убивал установку на полпути — «скачал до 100%, закрылся, версия
  ; старая». Перезапускаем себя одноразовой задачей Планировщика (процесс
  ; вне Job) и выходим. /KAMINTRAMP у перезапущенного — стоп рекурсии.
  \${GetParameters} $R0
  !insertmacro KLOG "onInit exe=$EXEPATH params=[$R0]"
  ClearErrors
  \${GetOptions} $R0 "/KAMINTRAMP" $R1
  IfErrors 0 tramp_skip_logged
  System::Call 'kernel32::IsProcessInJob(i -1, i 0, *i .R2) i .R3'
  !insertmacro KLOG "inJob=$R2"
  IntCmp $R2 0 tramp_done
  ; Провал любого шага schtasks → tramp_done: установка в Job хоть с шансом
  ; на успех лучше молчаливого Quit без установки.
  nsExec::ExecToStack 'schtasks /Create /F /TN "KaminIDE_SelfUpdate" /TR "\\"$EXEPATH\\" $R0 /KAMINTRAMP" /SC ONCE /ST 23:59'
  Pop $R3
  !insertmacro KLOG "schtasks create rc=$R3"
  StrCmp $R3 "0" 0 tramp_done
  nsExec::ExecToStack 'schtasks /Run /TN "KaminIDE_SelfUpdate"'
  Pop $R3
  !insertmacro KLOG "schtasks run rc=$R3 — quit, ждём трамплин"
  StrCmp $R3 "0" 0 tramp_done
  ; Без явного 0 Quit отдаёт код 2 — апдейтер показывал ложную ошибку
  ; «installer exited early: 2» и не выходил.
  SetErrorLevel 0
  Quit
  tramp_skip_logged:
  !insertmacro KLOG "trampolined instance — установка напрямую"
  tramp_done:
FunctionEnd
!define MUI_ICON "${appIcon}"
!define MUI_UNICON "${appIcon}"
; Брендинг: логотип в шапке каждой страницы + подпись вместо «Nullsoft…».
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "${headerBmp}"
!define MUI_BGCOLOR "1e1f29"
!define MUI_TEXTCOLOR "e1e4e8"
!define MUI_INSTFILESPAGE_COLORS "e1e4e8 1e1f29"
BrandingText "KaminIDE \${VER}"
; Одноклик (Discord/VS Code-стиль): запустил exe → прогресс → приложение
; открылось. Никаких welcome/выбора папки — путь дефолтный, всё остальное
; (карточка в «Приложениях», деинсталлятор) на месте.
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Russian"
AutoCloseWindow true

Section "Install"
  ; Одноразовые задачи (трамплин .onInit + прошлый relaunch) больше не нужны.
  nsExec::ExecToStack 'schtasks /Delete /F /TN "KaminIDE_SelfUpdate"'
  Pop $R3
  nsExec::ExecToStack 'schtasks /Delete /F /TN "KaminIDE_Relaunch"'
  Pop $R3
  ; Апгрейд поверх запущенного: закрыть приложение И сайдкары (kaminhost =
  ; переименованный node; живой ребёнок лочит runtime/ — File тогда молча
  ; оставляет старые файлы, «установщик не обновляет версию»).
  !insertmacro KLOG "install section start"
  ExecWait 'taskkill /F /IM kaminide-gpui.exe' $0
  ExecWait 'taskkill /F /IM kaminhost.exe' $0
  ; CEF-дети: живой kaminide-web.exe держит файлы в runtime/ — «файлы заняты»
  ; у сотрудников с DLP (Job добивает их при смерти главного, но не при
  ; подвисшем главном или осиротевших с прошлого краша).
  ExecWait 'taskkill /F /IM kaminide-web.exe' $0
  !insertmacro KLOG "taskkill done"
  ; Ждём РЕАЛЬНОГО освобождения файлов, а не фиксированные 1.5с: на машинах с
  ; DLP/антивирусом хэндлы распакованных файлов живут дольше, и File /r падал
  ; в тихий Abort — «скачал до 100%, приложение закрылось и не вернулось,
  ; версия старая» (прод-репорт). Проба = переименование главного exe:
  ; удаётся — файлы свободны. До 20 попыток по 1с.
  StrCpy $2 0
  wait_unlock:
    IntOp $2 $2 + 1
    ClearErrors
    Rename "$INSTDIR\\kaminide-gpui.exe" "$INSTDIR\\kaminide-gpui.exe.old"
    IfErrors 0 unlocked
    IntCmp $2 20 unlock_failed
    Sleep 1000
    Goto wait_unlock
  unlocked:
    Rename "$INSTDIR\\kaminide-gpui.exe.old" "$INSTDIR\\kaminide-gpui.exe"
    !insertmacro KLOG "unlocked за $2 попыток"
    Goto do_install
  unlock_failed:
    ; Файлы так и заняты: вернуть юзеру РАБОТАЮЩЕЕ приложение (старая версия
    ; цела) и выйти с ненулевым кодом — апдейтер покажет ошибку, а не тишину.
    !insertmacro KLOG "unlock_failed — восстановление старого exe"
    Exec 'explorer.exe "$INSTDIR\\kaminide-gpui.exe"'
    SetErrorLevel 5
    Abort "Файлы заняты — закрой KaminIDE и запусти установку снова."
  do_install:
  SetOutPath "$INSTDIR"
  ClearErrors
  File /r "${dist.replaceAll("/", "\\\\")}\\*"
  IfErrors 0 files_ok
    !insertmacro KLOG "File /r FAILED — восстановление старого exe"
    Exec 'explorer.exe "$INSTDIR\\kaminide-gpui.exe"'
    SetErrorLevel 5
    Abort "Не удалось записать файлы — закрой KaminIDE и запусти установку снова."
  files_ok:
  !insertmacro KLOG "files_ok"
  ; Маркер фактически установленной версии (быстрая проверка руками).
  FileOpen $1 "$INSTDIR\\version.txt" w
  FileWrite $1 "\${VER}"
  FileClose $1
  WriteRegStr HKCU "Software\\KaminIDE-GPUI" "InstallDir" "$INSTDIR"
  ; Контекстное меню папок — пишет ИНСТАЛЛЕР, не только self-heal приложения:
  ; self-heal «последний запущенный выигрывает» оставлял пункт на старом
  ; exe (Tauri-версия или прежняя установка) до первого запуска новой —
  ; жалоба «новое приложение не перезаписывает собой открытие в KaminIDE».
  ; Сброс _Fingerprint заставляет self-heal перерегистрироваться при запуске.
  WriteRegStr HKCU "Software\\Classes\\Directory\\shell\\OpenWithKaminIDE" "" "Open with KaminIDE"
  WriteRegStr HKCU "Software\\Classes\\Directory\\shell\\OpenWithKaminIDE" "Icon" "$INSTDIR\\kaminide-gpui.exe"
  WriteRegStr HKCU "Software\\Classes\\Directory\\shell\\OpenWithKaminIDE\\command" "" '"$INSTDIR\\kaminide-gpui.exe" "%V"'
  WriteRegStr HKCU "Software\\Classes\\Directory\\Background\\shell\\OpenWithKaminIDE" "" "Open with KaminIDE"
  WriteRegStr HKCU "Software\\Classes\\Directory\\Background\\shell\\OpenWithKaminIDE" "Icon" "$INSTDIR\\kaminide-gpui.exe"
  WriteRegStr HKCU "Software\\Classes\\Directory\\Background\\shell\\OpenWithKaminIDE\\command" "" '"$INSTDIR\\kaminide-gpui.exe" "%V"'
  DeleteRegValue HKCU "Software\\Classes\\Directory\\shell\\OpenWithKaminIDE" "_Fingerprint"
  CreateShortcut "$SMPROGRAMS\\\${APP}.lnk" "$INSTDIR\\kaminide-gpui.exe"
  CreateShortcut "$DESKTOP\\\${APP}.lnk" "$INSTDIR\\kaminide-gpui.exe"
  WriteUninstaller "$INSTDIR\\uninstall.exe"
  WriteRegStr HKCU "${uninstKey}" "DisplayName" "\${APP}"
  WriteRegStr HKCU "${uninstKey}" "DisplayVersion" "\${VER}"
  WriteRegStr HKCU "${uninstKey}" "Publisher" "dykamino.studio"
  WriteRegStr HKCU "${uninstKey}" "DisplayIcon" "$INSTDIR\\kaminide-gpui.exe"
  WriteRegStr HKCU "${uninstKey}" "UninstallString" "$INSTDIR\\uninstall.exe"
  WriteRegDWORD HKCU "${uninstKey}" "NoModify" 1
  WriteRegDWORD HKCU "${uninstKey}" "NoRepair" 1
  ; Размер в «Приложениях» (КБ).
  \${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${uninstKey}" "EstimatedSize" "$0"
  ; Одноклик: приложение стартует само, окно инсталлера закрывается.
  ; Перезапуск ЧЕРЕЗ Планировщик: прямой Exec делает приложение ребёнком
  ; инсталлера (в Job задачи — умрёт с её завершением), а explorer-трюк
  ; (explorer.exe "путь") на проверке вовсе не запускал exe. Задача
  ; Планировщика порождает процесс вне наших Job — механика уже доказана
  ; трамплином самого инсталлера.
  nsExec::ExecToStack 'schtasks /Create /F /TN "KaminIDE_Relaunch" /TR "\\"$INSTDIR\\kaminide-gpui.exe\\"" /SC ONCE /ST 23:59'
  Pop $R3
  nsExec::ExecToStack 'schtasks /Run /TN "KaminIDE_Relaunch"'
  Pop $R4
  !insertmacro KLOG "install done — relaunch via schtasks (create=$R3 run=$R4)"
  StrCmp $R4 "0" relaunch_done
  ; Фолбэк: хоть какой-то запуск лучше «не перезапустился».
  Exec '"$INSTDIR\\kaminide-gpui.exe"'
  relaunch_done:
SectionEnd

Section "Uninstall"
  ExecWait 'taskkill /F /IM kaminide-gpui.exe' $0
  Sleep 500
  RMDir /r "$INSTDIR"
  Delete "$SMPROGRAMS\\\${APP}.lnk"
  Delete "$DESKTOP\\\${APP}.lnk"
  DeleteRegKey HKCU "${uninstKey}"
  DeleteRegKey HKCU "Software\\Classes\\Directory\\shell\\OpenWithKaminIDE"
  DeleteRegKey HKCU "Software\\Classes\\Directory\\Background\\shell\\OpenWithKaminIDE"
SectionEnd
`
// НЕ в dist — иначе File /r утащит сам .nsi в инсталлер.
const nsiPath = join(root, "installer.nsi")
// UTF-8 BOM обязателен: makensis без BOM читает .nsi в ANSI-кодпейдже —
// русские строки прогресса/ошибок превращались в кракозябры у сотрудников.
writeFileSync(nsiPath, "﻿" + nsi)
const makensis = join(process.env.LOCALAPPDATA, "tauri", "NSIS", "makensis.exe")
const r = spawnSync(makensis, [nsiPath], { stdio: "inherit" })
if (r.status !== 0) process.exit(r.status ?? 1)
// Подписи .sig больше нет: это был рудимент tauri-plugin-updater — GPUI-клиент
// сигнатуру никогда не проверял, а «ключ» лежал в репо с пустым паролем
// (театр безопасности + утечка при публикации). Сервер с 6.3.92 отдаёт
// манифест без .sig.
console.log(`[installer] готово: KaminIDE_${version}_x64-setup.exe`)
