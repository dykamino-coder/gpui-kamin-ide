# 90 — Сборка, апдейтер, дистрибуция

Источники: `src-tauri/src/{sidecar,runtime,updater,context_menu,bridge_uninstall}.rs`, `scripts/build-runtime-payload.mjs`, `tauri.conf.json`, память build_pipeline.

## Single-exe: embed + unpack (переносится 1:1)
- Payload: `npm run build:host:tauri` + `build:payload:tauri` → runtime.tar.zst (runtime/ = node.exe + kamin-host.mjs + native node_modules + builtin-extensions; zstd-19 ~29MB) — скрипт ПЕРЕИСПОЛЬЗУЕТСЯ
- include_bytes! → распаковка в <app_local_data>/runtime/<version>-<FNV-1a-хэш архива> (смена payload без бампа версии → новая распаковка; известный инцидент c85fba2 — хэш-ключ обязателен)
- Atomic staging+rename; GC старых сиблингов (щадит <60s — конкурентные запуски)
- Спаун: kaminhost.exe (переименованный node.exe — анти `taskkill /IM node.exe`) kamin-host.mjs, stdio NDJSON, Job Object KILL_ON_JOB_CLOSE, CREATE_NO_WINDOW, рестарт-супервизор (лимит 3), парсинг kamin-host:ready → {port,token}

## Апдейтер (переносится по логике)
- Endpoint РАНТАЙМ-производный от serverUrl Бриджа: ws/wss→http/https, срез /ws-сегмента (с сохранением mount-префикса), + /updates/kaminide/{{target}}/{{arch}}/{{current_version}} (шаблон Tauri, updater.rs:77)
- Подпись minisign (pubkey в конфиге), http разрешён (внутренний сервер); createUpdaterArtifacts
- updater_check → UpdateInfo {available,version,current,notes,date} (up-to-date ≠ ошибка); updater_install → download+install с событиями updater:progress/download-finished → verify → restart
- UI: VersionUpdateItem в статус-баре (3 состояния) — см. 40-components
- Серверная сторона (/updates/kaminide/*) НЕ меняется; ключ .tauri/kaminide-updater.key — формат артефактов должен остаться совместимым ИЛИ сервер учит новый формат (решение при имплементации — GPUI-шелл сам себе апдейтер: скачивание exe/nsis + подпись)

## «Open with KaminIDE» + single-instance (переносится 1:1)
- HKCU Directory\shell + Directory\Background\shell OpenWithKaminIDE, команда "<exe>" "%V"; self-heal на каждом запуске (fingerprint exe path+mtime), SHChangeNotify при смене
- Single-instance: второй запуск → argv форвард, фокус/анминимайз/показ, событие open-folder → сайдкар --open-folder
- bridge_uninstall: детект/удаление легаси Electron Bridge (Squirrel, ключи, %APPDATA%) — сохранить

## Сборка GPUI-приложения
- cargo build: mimalloc, release-профиль (lto=fat, opt-level=s, panic=abort, codegen-units=1, strip)
- Пайплайн (наследует build_pipeline): bump → renderer-ассеты не нужны (нативный UI!) → ВЕБВЬЮ БРИДЖА нужны: build.mjs расширения + webview (память build_bridge_webview_separate — до сборки шелла!) → payload → cargo → инсталлер (NSIS-аналог) → installer/ → build.sh bridge → podman tag/push
- Дистрибуция: Bridge /download = инсталлер; Docker dykamino/open-claude-bridge; НИКОГДА --no-cache; Dockerfile bake-gotcha (только KaminIDE_*-setup.exe*)
- Версии в 3 местах (пер-проектная схема сохранится: package.json аналог → Cargo.toml + conf)

## Перф-бюджеты (жёсткие, из памяти)
- <800ms cold start, <90MB idle. Нативный GPUI обязан бить Tauri-цифры; замер в CI (perf:cold-start / perf:ram аналоги)

## Чеклист паритета (packaging)
- [ ] Payload-скрипт переиспользован; unpack по version+hash; rename-трюк; Job Object
- [ ] Апдейтер: endpoint-производная + подпись + прогресс + restart; серверная совместимость решена
- [ ] Context menu + single-instance + open-folder цепочка
- [ ] bridge_uninstall
- [ ] Пайплайн сборки задокументирован и работает end-to-end
- [ ] Перф-бюджеты в CI
