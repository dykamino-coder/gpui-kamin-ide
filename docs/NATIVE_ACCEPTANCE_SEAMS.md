# Debug-контур приёмки native shell

Общая ветка `feat/native-acceptance-seams` начинается от `origin/main` и не
принимает fixes #183/#185/#186/#187. Seams существуют только при
`cfg(debug_assertions)`. Даже сборка release с default feature `probe` не
содержит команду `nativeAcceptance`, fixture, device-loss или чтение новых env.
CI компилирует настоящий config-модуль с `debug-assertions=no` и проверяет,
что env-reader не вызывается. Версии, `vendor/` и `crates/html` не меняются.

## Подготовка кандидата

В собственной acceptance-ветке объединить нужный PR и shared seam commit,
записать exact SHA, собрать `cargo build -p kaminide-gpui`. Не тестировать старый
бинарь: launcher сохраняет SHA исходников и SHA-256 переданного exe; эти два
значения оператор дополнительно связывает с успешной сборкой кандидата.
Перед push/приёмкой: fmt, clippy `-D warnings`, shell tests и event-routing check.
Сборка со включённым default feature `probe` обязательна для команд ниже.

Windows Graphics Tools / `d3dconfig.exe` нужны только для WARP и device loss.
Launcher использует отдельное имя exe, требует **пустой** список D3DConfig apps,
сохраняет исходные `force-warp`/`force-d3d11on12`, добавляет только полный путь
тестового exe и восстанавливает настройки/удаляет запись после закрытия окна.
Он не отключает GPU, не меняет системные animation settings и не вызывает TDR.
Не прерывать launcher до закрытия окна. После аварии launcher удалить его
точную запись через `d3dconfig apps --remove <path>` и восстановить параметры
из private `d3dconfig.log`; не использовать `--reset` или `apps --clear`.

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

Launcher с `-DeviceLoss` включает D3D11On12 только для acceptance exe.
Используется поддержанный [GetD3D12Device](https://learn.microsoft.com/en-us/windows/win32/api/d3d11on12/nf-d3d11on12-id3d11on12device1-getd3d12device)
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

Запустить `-Warp -ReducedAnimations`, затем `ready`. С #187 native `[why]`,
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
