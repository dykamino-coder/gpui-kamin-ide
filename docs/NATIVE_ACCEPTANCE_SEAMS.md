# Debug-контур приёмки native shell

Общая ветка `feat/native-acceptance-seams` начинается от `origin/main` и не
принимает fixes #183/#185/#186/#187. Seams существуют только при
`cfg(debug_assertions)`. Даже сборка release с default feature `probe` не
содержит команду `nativeAcceptance`, fixture, device-loss или чтение новых env.
CI компилирует настоящий config-модуль с `debug-assertions=no` и проверяет,
что env-reader не вызывается. Версии, `vendor/` и `crates/html` не меняются.

## Воспроизводимый запуск на GitHub Actions

Workflow `Native Windows smoke` собирает указанный Git SHA на стандартной
`windows-2022` VM. Сценарий `native-client` проверяет размер отрисованной области
GPUI, активацию встроенного расширения в отдельном Extension Host и выполнение
JavaScript штатной страницы чата в CEF. Сценарий `bridge-roundtrip` дополнительно
поднимает настоящий Bridge на loopback той же VM в режиме синтетического CLI.
Запрос `/health` проходит из CEF через штатный IPC расширения и возвращает
версию сервера из проверяемого commit. Затем клиент открывает сессию Codex
через WebSocket и отображает ответ синтетического app-server на тестовую
фразу. Сценарий `workspace-link` продолжает этот же диалог: синтетический
ответ содержит ссылку на созданный в профиле файл. Драйвер находит ссылку
в сообщении ассистента, посылает события мыши в CEF и сверяет открытый файл и
позицию 4:2 по данным GPUI. Для него в кандидате нужен debug-запрос
`nativeAcceptance/editorLocation`. Сценарий `extension-hooks` запускает
Windows-интеграцию командных hooks из исходников кандидата без GUI.
Сценарий `console-scroll` передаёт 80 строк синтетического CLI через настоящий PTY,
проверяет normal buffer, положение видимого ползунка после колеса и перетаскивания к обоим краям.
Сценарий `engine-menus` переключает тот же чат между синтетическим Codex и
Claude, повторно выбирает Opus и отдельно проверяет меню режима движка и
инструментов рабочего пространства. После выбора «Change mode in Console» проверяет
холодное открытие со свёрнутой правой колонкой, повторную команду в открытой
Console и вызов для панели, закреплённой в скрытом RightTop. Перед отправкой
Windows Shift+Tab драйвер сверяет активный слот, геометрию и paint GPUI, фокус
страницы и xterm. В отчёте сохраняются отдельные наблюдения до и после клавиши.
Сценарий `engine-receipts` дополнительно проверяет Bypass, Manual и Plan.
Синтетический CLI читает реальные флаги своего процесса и сообщает режим в
UserPromptSubmit и Stop. Клиент отображает полученный режим и сохраняет Plan
после возврата на Claude. Для сценария нужен commit с этой тестовой fixture.
Проверки не требуют подписки.

Сценарий `installed-client` принимает отдельный `installer_source`: ID запуска
PR checks, опубликовавшего `kaminide-release-<SHA>`, или тег опубликованной
версии. Передать соответствующий `candidate` (для артефакта — SHA сборки из
его имени). Штатный verifier связывает установщик с исходным деревом и
проверяет SHA-256. Свежий профиль сначала получает предыдущую опубликованную версию. Её штатный
updater скачивает проверенный пакет с loopback-канала после клика в статус-баре.
Драйвер ждёт автоматического перезапуска, сверяет InstallDir, version.txt,
DisplayVersion, обе регистрации команды Explorer, install log, ответ настоящего
Extension Host и страницу CEF. Версия из UI сохраняется в version.png для
чтения. Видимость контекстного меню в окне Explorer и полная installer-матрица
остаются отдельными проверками.
Параметр `installed_scenario` позволяет дополнительно выполнить `bridge-roundtrip`,
`workspace-link`, `console-scroll`, `engine-menus` или `engine-receipts` на установленном клиенте. В release-сценарии
ссылки положение курсора проверяется сохранением тестовой вставки в строке 4,
столбце 2 временного файла. Report фиксирует класс `hosted-windows-installed-release`.

```bash
commit=$(git rev-parse HEAD)
gh workflow run native-windows-smoke.yml -f candidate="$commit" -f scenario=bridge-roundtrip
gh run list --workflow native-windows-smoke.yml --limit 3
gh run watch "$run_id" --exit-status
gh run download "$run_id" --dir "$capture_dir"
```

До публикации релиза взять SHA сборки из имени артефакта проверок релизного PR.
GitHub собирает merge-кандидат. `pr_checks_run` задаёт ID запуска PR checks.

```bash
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
candidate=$(gh api "repos/$repo/actions/runs/$pr_checks_run/artifacts" --paginate \
  --jq '.artifacts[] | select(.expired == false) | .name | capture("^kaminide-release-(?<sha>[a-f0-9]{40})$").sha')
gh workflow run native-windows-smoke.yml -f candidate="$candidate" \
  -f scenario=installed-client -f installer_source="$pr_checks_run" -f installed_scenario=workspace-link
```

В командах `run_id` — ID нужного запуска из списка, `capture_dir` — отдельный
каталог для отчёта. Поля `candidate` и `driver` в pipeline.json указывают отдельно проверяемый commit и версию драйвера.
Прогон заканчивается ошибкой на первой обязательной границе, для которой
истёк таймаут или получен неправильный ответ. Снимок относится к окну
приложения. Срок хранения CI-артефакта — три дня.

Оба клиентских сценария подключаются к Windows-реализации Bridge. Профиль
клиента и данные сервера временные и раздельные. Результаты Linux Docker
проверяет отдельный job. Сквозной тест Windows → Linux потребует доступного
с этой же VM отдельного Linux guest в изолированном окружении и положительной
пробы loopback до запуска клиента. Адреса отдельных GitHub jobs в этом
сценарии не используются. Приёмка установленного release-кандидата, работа
обычного GPU и ввод через физические устройства остаются отдельными сценариями.

### Матрица расширения

| Область | Уровень | Критерий приёмки |
| --- | --- | --- |
| MCP | Integration, Protocol E2E, затем Browser и Windows | Холодный запуск находит отдельный и plugin MCP без Test. Проверяются поздний handshake, stderr, tools/list, реальный вызов и обновление каталога после reconnect. Постоянная ошибка видна и не вызывает непрерывных повторов. Чужой пользователь не видит инструмент. |
| Plugins и Skills | Integration, Protocol E2E, затем Windows Customize | Синтетический marketplace устанавливает плагин, у модели есть инструкция ожидаемой версии. Update/disable и смена проекта меняют содержимое. Поведение живой модели принимается отдельной полевой проверкой. |
| Hooks | Unit, Integration на Windows, затем Native E2E | Active Hooks соответствует профилю. Разрешённый hook получает события и аргументы, allow/deny, ошибка и timeout дают ожидаемое решение в обоих движках. Повторный маршрут инструмента, reconnect и смена движка сохраняют защиту. |
| Сессии | Protocol E2E, Browser, затем Native | Для нового чата и resume сверить модель, поддерживаемые уровни и точный effort в turn/start. Проверить stop, сообщения об ошибках, обновление квоты и единственный экземпляр исторических событий. |
| UI | Native Windows E2E | Проверить Customize, вкладки, фокус и клавиши, движение Console колесом и за ползунок, переход к файлу и курсору, восстановление после остановки Extension Host. Каждое действие завершается наблюдаемым состоянием в заданный срок. |
| Инсталлятор | Установленный exact candidate | Старая версия скачивает новый пакет, штатно перезапускается, версия видна в статус-баре и реестре, клиент выполняет выбранный сценарий. |
| Корпоративная среда | Полевая приёмка | Отдельно подтвердить реальную Windows-сборку, Linux-контейнер, корпоративный маршрут, живой аккаунт и ожидаемый эффект исправления. |

Для сетевого сценария сначала проверить `linuxCapability` в отчёте стандартной VM.
Он фиксирует наличие WSL, число зарегистрированных WSL 2 дистрибутивов и успешный
`uname -s` в существующем госте. Если доступен гость, следующий этап — поднять
одноразовый Linux Bridge в нём, проверить /health с Windows и провести тот же
маршрут CEF → Extension Host → WebSocket. Для отдельной пары VM требуется
изолированная сеть. Выделенный Windows runner требует обслуживания и очистки
образа между заданиями. Стенд мейнтейнера предполагает согласованный доступ и время.
[GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) ·
[Требования WSL 2 к VM](https://learn.microsoft.com/en-us/windows/wsl/faq)

## Подготовка кандидата

В собственной acceptance-ветке объединить нужный PR и shared seam commit,
записать exact SHA, собрать `cargo build -p kaminide-gpui`. Не тестировать старый
бинарь: launcher сохраняет SHA исходников и SHA-256 переданного exe; эти два
значения оператор дополнительно связывает с успешной сборкой кандидата.
Перед push/приёмкой: fmt, clippy `-D warnings`, shell tests и event-routing check.
Сборка со включённым default feature `probe` обязательна для команд ниже.

WARP и device loss включаются **внутри debug-сборки**, без D3DConfig и любых
системных настроек DirectX/графики. Launcher передаёт только переменные
окружения процесса: `-Warp` → `KAMIN_DEBUG_D3D_DEVICE=warp` (GPUI создаёт
своё D3D11-устройство на WARP-адаптере своей же DXGI factory,
`EnumWarpAdapter`), `-DeviceLoss` → `KAMIN_DEBUG_D3D_DEVICE=d3d11on12` (GPUI
создаёт D3D11On12-устройство поверх собственного D3D12-устройства первого
аппаратного адаптера). Оба seam находятся в
`crates/html/src/host/instrumentation/debug_device.rs`; release не читает переменную, CI
компилирует файл с `debug-assertions=no` и `yes`. Launcher не отключает GPU,
не меняет системные animation settings и не вызывает TDR. Окно показывается
без активации (`KAMIN_DEBUG_NO_ACTIVATE=1` → `WindowOptions.focus=false`),
поэтому прогон на интерактивном рабочем столе владельца не забирает фокус.
Launcher возвращает управление сразу после старта (`-Wait` — ждать закрытия);
закрывать только свой PID из `manifest.json`.

Каждый capture получает новую внешнюю папку `$Capture`. stdout/stderr, crash,
CEF и native logs, временные файлы и изолированный профиль находятся под ней.
По умолчанию `KAMIN_DEBUG_NO_HOST=1`: нет sidecar, миграции production store
или регистрации контекстного меню Explorer. Для настоящего Claude-сценария
явно передать `-WithHost`; Bridge token для native fixtures не нужен.

```powershell
# $Capture — новый каталог в разрешённой private inbox, вне любого worktree.
./scripts/start_native_acceptance.ps1 -Exe ./target/debug/kaminide-gpui.exe `
  -EvidenceDir $Capture -ReducedAnimations
# В другом терминале — тот же порт (по умолчанию 9357):
$env:KAMIN_PROBE_PORT = '9357'
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"status"}'
```

`-ReducedAnimations` задаёт только app-level native reduced-motion policy и
Chromium `force-prefers-reduced-motion`. Фактическое OS значение
`SPI_GETCLIENTAREAANIMATION` не изменяется; evidence должно различать OS и
debug override. `-Warp` дополнительно задаёт software policy shell, а реальный
GPUI adapter выводится в `[acceptance] adapter=... vendor=...`. Утверждать WARP
только после наблюдения `Microsoft Basic Render Driver` / Microsoft adapter.
`-SoftwareCef` отдельно включает существующий CEF software fallback.

## #183 / INC-2026-0017

Launcher сразу показывает настоящие `brand_loader`, codicon spinner и ring
spinner в sidebar, без session/Bridge. Сравнить два новых capture, обычный и
`-ReducedAnimations`: подписи/глифы видны; декоративное движение выключено
только в reduced режиме. Измерять stationary кадры/CPU без движения указателя.

```powershell
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"waiting"}'
# Штатный contributed loader: реальные 45 retry/backoff попыток, provider молчит.
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"error"}'
# Проверить hover/click Retry: штатный RetryView сбрасывает бюджет.
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"ready"}'
# Реальная доставка HTML/cover/fade; synthetic input/button/select доступны.
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"switch"}'
# Настоящий chat_cover deadline/fade применяется к synthetic view.
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"loaders"}'
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"clear"}'
```

В candidate с #183 stationary waiting обязан продвигаться через его 250 ms
one-shot scheduler до ошибки примерно за две минуты, без animated frames;
clear снимает seam и закрывает fixture browser. Проверить caption/progress,
Retry, deadline/fade, соседние keyboard/focus. Synthetic switch проверяет native
cover path; реальный session-switch остаётся отдельным сценарием карточки.

## #185 / INC-2026-0021

**Выполнять wait injection только с exact-status fix #185.** В origin/main
positive HRESULT теряется в generated Result, поэтому baseline hook сохраняет
этот дефект. При объединении #185 заменить его status expression следующим
adapter, оставив `keyed_access::with_access` и lifecycle/retry ветки без изменений:

```rust
let status = mutex.as_ref().map(|m| {
    #[cfg(debug_assertions)]
    if let Some(status) = crate::native_acceptance::faults::wait_status(id) {
        return status;
    }
    super::keyed_mutex::acquire(m)
});
```

Удалить baseline `acquire` closure из copy_frame; общий generated-wrapper helper
можно оставить только при его использовании. Это необходимый seam adapter
рядом с кодом #185, не самостоятельная реализация mutex fix.

Запустить accelerated CEF (`-SoftwareCef` не задан), команда `ready`, затем:

```powershell
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"wait","view":"kaminDebugAcceptance","status":"timeout","count":3}'
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"status"}'
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"wait","view":"kaminDebugAcceptance","status":"abandoned"}'
```

Инъекция выбирает точный view ID (popup: `kaminDebugAcceptance::popup`),
потребляется **до** настоящего AcquireSync, максимум три раза. Статус `258`/`128`
идёт в настоящую ветку PR: copy/release должны отсутствовать, timeout сохраняет
валидный prior frame, abandoned закрывает/reсоздаёт producer. В status проверить
`consumed`, в native log — `injected_status`/`acquired=false`, в D3D diagnostics —
отсутствие несогласованных CopyResource/ReleaseSync. Не считать оставшийся
pending fault подтверждением accelerated path. Повторить с popup и отдельно
`-SoftwareCef`: software input/frames работают, GPU fault не потребляется.

## #186 / INC-2026-0022

Launcher с `-DeviceLoss` просит debug-сборку создать собственное
D3D11On12-устройство (только этот процесс, без D3DConfig). Используется поддержанный [GetD3D12Device](https://learn.microsoft.com/en-us/windows/win32/api/d3d11on12/nf-d3d11on12-id3d11on12device1-getd3d12device)
и [RemoveDevice](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12device5-removedevice),
без private GPUI struct layout/vtable patches. На неподдержанном runtime
записывается `device_loss=unsupported`, что означает blocker, а не passed.

```powershell
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"ready"}'
node scripts/probe.mjs '{"cmd":"nativeAcceptance","action":"deviceLoss"}'
```

Подождать GPUI recovery: `device_loss=removed` должен сопровождаться новой
`device_changed=true` и возобновившимися CEF frames/input. Повторить после
каждого восстановления; включить popup, resize, D3D diagnostics/live objects и
return-to-baseline measurement. Seam балансирует собственный owned getter.
Отдельно прогнать `-SoftwareCef`. Инъекция относится к живому устройству окна;
она не является моделью fake device generations и не заявляет field crash fix.

## #187 / INC-2026-0001

Запустить `-Warp -ReducedAnimations -SoftwareCef`, затем `ready`; accelerated
сравнение выполнить отдельным обычным запуском без этих трёх switches.
Так CEF software path не зависит от совместимости shared handles между
аппаратным Chromium и WARP-устройством GPUI. С #187 native `[why]`,
`[views]`, adapter/browser policy строки должны атрибутировать synthetic
software/accelerated frames, native loader callbacks и реальные input/wake/pull.
OS animation value остаётся реальным; debug override указан в capture manifest.
Для idle/active-Claude comparison дополнительно нужен `-WithHost` и настоящий
сценарий владельца: fixture обеспечивает доступный локальный acceptance контур,
но не доказывает CPU improvement относительно полевого baseline 4,55 ядра.

Для каждого PR заполнить `Runtime acceptance` только после живого прогона:
exact integrated SHA, сценарий/threshold, фактическое наблюдение и immutable
task-specific private evidence URL. Добавление seams и зелёные automated checks
сами по себе не превращают исходные pending runtime gates в passed.
