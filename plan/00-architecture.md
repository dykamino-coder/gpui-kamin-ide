# 00 — Архитектура

## Процессная модель

```
gpui-kamin-ide.exe (Rust, GPUI)                ← НОВОЕ: главное окно, весь chrome нативно
  ├─ spawn: runtime\node.exe kamin-host.mjs    ← БЕЗ ИЗМЕНЕНИЙ (сайдкар, как в Tauri)
  │     KAMIN_HOST_TRANSPORT=stdio
  │     --builtin-dir --data-dir --cache-dir --open-folder
  │     stdout: {"kind":"evt","channel":"kamin-host:ready","payload":{extensions,wsPort,wsToken}}
  │     └─ forkExtHost → exthost child (vscode.* API, VSIX)   ← БЕЗ ИЗМЕНЕНИЙ
  ├─ WS-клиент: ws://127.0.0.1:<wsPort>/rpc?token=<wsToken>   ← Rust-реализация протокола
  ├─ gpui-wry вебвью-оверлеи: http://127.0.0.1:<hostPort>/__webview/<id>?token=
  │     (WebView2 — panels НЕ фризят; фризил только главный WebView2-рендерер)
  │     ⚠ роут /__webview НЕТ на main (сейчас Tauri kaminwebview://); порт с cef-migration — см. plan/70, 98 Q4
  └─ HTTP к open-claude-bridge серверу не напрямую — как и сейчас, через extension
```

Доказано ранее (Chrome/Servo миграция): рендерер шелл-агностичен, весь data plane = WS к kamin-host. GPUI-приложение — новый клиент того же протокола.

## Слои Rust-приложения

1. **shell/** — окно, титлбар, drag-región, window-controls, персист bounds.
2. **theme/** — токены (1:1 с CSS-переменными kamin-ide), light/dark, contributed themes.
3. **ws/** — клиент протокола kamin-host: запрос/ответ (id-корреляция) + подписки на события; типы = зеркала TS-типов.
4. **state/** — сторы (аналог signals/): registry, sessions, fs-tree, layout, diagnostics…
5. **ui/** — регионы и компоненты (см. 40-components.md), на gpui-component.
6. **webview/** — gpui-wry обвязка: позиционирование оверлеев по layout-ректам, postMessage-релей, скрытие при перекрытии попапами (тултипы рисует ХОСТ — паттерн уже есть в kamin-ide).
7. **editor/** — gpui-component code editor + мост к LSP-потокам kamin-host (провайдеры уже там).
8. **terminal/** — нативный терминал ↔ kamin:pty:* (протокол не меняется).
9. **sidecar/** — spawn/respawn kamin-host, unpack single-exe payload по хэшу (перенос логики из src-tauri/sidecar.rs).
10. **updater/** — перенос tauri-updater флоу (endpoint из runtime serverUrl, подпись).
11. **diag/** — freeze-диагностика: freeze_watchdog (prod/pong/verdict), native_stack, webview_watchdog (ProcessFailed wry), heap_sampler (opt-in), diag_log (единый лог), log_reset (вайп на новой версии). Детали в 10-shell-window.

## Что НЕ переписывается

- kamin-host (node): services, exthost, LSP-хостинг, PTY, sessions, JSONL, webview-HTTP.
- Билд-пайплайн payload (build-runtime-payload.mjs) — переиспользуется.
- open-claude-bridge server + Docker-паблишинг.
- VSIX-расширения, включая claude-bridge — работают как есть.

## Ключевые риски

| Риск | Митигейшн |
|---|---|
| gpui pre-1.0, breaking changes | пин версии; gpui-component уже трекает upstream |
| gpui-wry experimental | у нас Tauri-multiwebview опыт того же режима (оверлей); фолбэк — прямой webview2-rs COM |
| Замена Monaco → gpui-component editor | LSP уже с хоста; фича-гэп свести в 40-components (peek, minimap, multi-cursor…) |
| Замена xterm → нативный терминал | протокол pty не меняется; референс alacritty_terminal |
| Вебвью-оверлей поверх GPUI | попапы над вебвью — хостом (паттерн уже в проде kamin-ide) |
