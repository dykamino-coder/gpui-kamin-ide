# 72 chat-switch-skeleton — наша реализация

НЕ РЕАЛИЗОВАНО (брендовой «шторки» с логотипом/glow/indeterminate-полосой нет).

Замена (эквивалент цепочки «не показать белую вспышку/старый чат»):
Файлы: `crates/shell/src/root.rs:225,420,624-628,814` (switching_to), `root.rs:2758-2824` (webview_body: load-cover), `root.rs:3702-3742` (chat-cover: wv2 скрыт до первого ipc), `crates/shell/src/ui/chat_webview.rs:119-125` (WebviewAlive по первому ipc)

## Структура (gpui-дерево кратко)
- `webviews_alive: HashSet<String>` — wv2-child показывается ТОЛЬКО после первого ipc-сообщения скрипта вью (`ShellEvent::WebviewAlive`); до этого wv2 `set_visible(false)` и панель рисует gpui-плейсхолдер.
- Load-cover (root.rs:2799-2815): `div` flex-col центр, gap `SPACE_2`(8) → `codicon \u{eb19}` 22px accent_primary (#89b4fa) → текст «Loading…» FS_MD(13) text_secondary (#adb3c7).
- Переключение сессии: `switching_to: Option<String>` → спиннер на чипе сессионного таба (session_tabs), гасится по подъёму сессии (root.rs:624-628). Поверх самого вебвью НИЧЕГО не рисуется — WebView2 перерисовывает контент in-place.

## Метрики (из кода, точные)
- Cover: gap 8, иконка 22px `#89b4fa`, текст 13px `#adb3c7`. Ни логотипа, ни полосы, ни анимаций.

## Отличия от original.md той же папки
1. Нет брендового скелета вообще: логотип 64px + glow 150px + breathe/float/sweep-анимации, caption «Loading conversation…», полоса 180×3 — всё отсутствует.
2. Нет непрозрачного фона `--editor-bg` поверх iframe при переключении сессии; вместо шторки — спиннер на чипе таба + нативная перерисовка вебвью.
3. Нет transition opacity 140ms и состояния `covering`.
4. Cover у нас применяется к ЛЮБОМУ вебвью до первого ipc (не только чату), т.е. это скорее аналог 70-webview-loading-skeleton, чем 72.

## Дополнение атрибутов (цикл 10)

- отступы: паддингов нет — load-обложка чата это тот же центрированный блок без padding (`root.rs:3244-3260`), показ гейтится `webviews_alive`/`switching_to` (`root.rs:242,4436-4439`). Оригинал: `.wrap { padding: 24px }` (`ChatSwitchSkeleton.module.css:13`).
- скругления: N/A: скругления — брендовой обложки с glow-кругом (`border-radius: 50%`) и полосой-свипом (`border-radius: 999px`, `ChatSwitchSkeleton.module.css:31,62`) нет; у нашей обложки скруглённых элементов нет вовсе.
- шрифты: «Loading…» fs-md 13 (`root.rs:3256`) против `.caption { font-size: var(--fs-sm) 12px }` оригинала (`ChatSwitchSkeleton.module.css:53`) — кегль на шаг крупнее; лого/анимаций (96/64px, breathe/float/sweep) нет.
