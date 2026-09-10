# 80 — Claude Bridge UX: инвентарь паритета

Этот файл фиксирует контракт совместимости, а не утверждает отсутствие
дефектов. Текущие runtime-инциденты и порядок исправлений ведутся в
`extensions/claude-bridge/RUNTIME_RELIABILITY.md`.

Источник: `extensions/claude-bridge/**` → `builtin-extensions/claude-bridge/` (extension.js + chat.html/tools.html/customize.html). Расширение — чистый VSCode-API + вебвью; Tauri-неосведомлённое. Весь серверный WS — в Node-хосте расширения. **GPUI ничего в нём не меняет** — но обязан обеспечить всё, что оно потребляет (вебвью-хостинг из 70, vscode.* из 60). Ниже — инвентарь для проверки «ничего не отвалилось».

## Вклады (contributes)
- viewsContainers: activitybar `claudeBridge` (Claude Bridge); auxiliarybar `claudeBridgePlan`/`Todos`/`Agents`/`Console` (пиннятся независимо); customize `claudeBridgeCustomize`
- views (все webview):
  - claudeBridgeChat («Claude») → chat.html, role chat, visibility-tracked
  - PlanView/TodosView/AgentsView/ConsoleView → один tools.html (id вью из URL iframe'а), role tools; Console = сырой терминал Claude CLI активной сессии
  - 10 customize-страниц (CzSettings/Skills/Agents/Mcp/Hooks/Plugins/Monitors/Sync/Logs/Stats) → customize.html, role customize, без visibility-wiring
- registerView: enableScripts, пустые localResourceRoots, retainContextWhenHidden:true, visibility-report для chat+tools (скрытые доганяются на reveal)
- Команды: openChat (fallback workbench.view.extension.claudeBridge), openTools, reconnect, regenerateTitle (динамич., из нативного контекст-меню сессии → /rename), openMcpSettings (динамич.)
- Статус-бар: 2 элемента слева (prio 50/49): $(plug) N зелёный connected / $(error) N красный errored (скрыт при 0); $(sync~spin) N/M при коннекте; поллинг 2500ms; клик → CzMcp; цвета #3fb950/#f85149
- **Тема**: contributes.themes → `claude-bridge-dark` (uiTheme vs-dark, themes/claude-bridge-dark.json) — участвует в Appearance-пикере и contributed-theme системе

## Чат-вебвью (структура — знать, чтобы тестить паритет)
- Header: CwdDisplay, FolderCrumb, ConnectionStatusBadge, DiagnosticButton, DisconnectButton, DownloadJsonlButton, MemoryInfoButton, OpenExplorerButton, OpenVscodeButton, PanelTripletToggle, PlanProgress, ReconnectButton, RegenerateTitleButton, SessionIds, SessionStats, ToggleServerPathButton, ToggleViewerButton, HeaderViewToggles
- Content: jsonl-панель (JsonlViewer + ToolCounterToast) и/или terminal-панель (xterm); тогглы viewer/jsonl персистятся
- InputBar-зона: ActivityIndicator (спиннер + превью thinking + цвет агента) → WidgetsPanel → InputBar
- Композер: ScrollDownPill, ActiveFileStatusBar, AttachButton, VoiceButton, PromptTextarea, SendButton, InputControls, SlashAutocomplete; Enter=send, Shift+Enter=NL, ↑/↓ история (сид из JSONL); busy-гейт Send↔Stop
- Селекторы: Model (opus-4-8[1m]/opus-4-8/sonnet-5/haiku-4-5/fable-5), Effort (low/medium/high/xhigh/max), Permissions (default/acceptEdits/plan/auto/dontAsk/bypassPermissions — инференс по JSONL назад)
- Attach active file: ХОСТ пушит активный файл+selection редактора в композер (extension/src/editor-context.ts). ⚠ В GPUI это питается от kamin:editor:active/selections — редактор ОБЯЗАН их слать (60-exthost контракт)
- Queue-виджет («Send now» = interrupt), SubagentButtonsRow (чипы бегущих агентов → SubagentFullscreen), elicitation-виджеты: AskUserWidget (вопросы + plan-approval Approve/Reject + PlanToggle), ElicitationWidget, McpElicitationWidget, PermissionWidget
- Стриминг: JsonlViewer + все рендеры инструментов; каналы streaming-entry/delta/status
- Скролл: MutationObserver-пиннинг (AT_BOTTOM 80px), инфинит-скролл вверх (NEAR_TOP 400px, anchor-restore), память скролла по табам, __scrollChatToBottom

## Мост вебвью↔хост расширения
bridge-transport (inv/snd/sub поверх acquireVsCodeApi) ↔ BridgeHost (vendored ipcMain). Role-gating: HEAVY_SESSION_CHANNELS (jsonl/streaming/pty) не идут в customize; STREAMING_ONLY — не в tools; скрытые вью помечаются stale и ресинкаются на reveal. ⚠ Это ЕЩЁ одна причина, почему visibility-report обязан работать в GPUI-вебвью-слое.

## Сервер-коннект (хост расширения, не трогаем)
ConfigStore <globalStorage>/open-claude-bridge-config.json — serverUrl ВЛОЖЕН под `config`: `{config:{serverUrl: ws://localhost:3456, token}}` (плоское чтение — только легаси-фолбэк); URL-нормализация localhost→127.0.0.1 + /ws/session; session:create/resume (cols 120 rows 40, protocolVersion:1); reconnect-бэкофф 1→30s, heartbeat 15/30s, establish-таймаут 25s, ghost-resume брейкер; transcript mirror на диске (быстрый пейнт + older-page); MCP-менеджер внешних серверов (stdio/http/sse/ws + oauth)

## Проверка паритета Бриджа в GPUI (ручной чек-скрипт)
- [ ] Чат открывается, реплей истории, стриминг живого ответа
- [ ] Send/Stop, очередь, Send now-interrupt
- [ ] Селекторы model/effort/permissions применяются (смена = рестарт PTY с --resume)
- [ ] AskUserQuestion табы+options+текст; ExitPlanMode Approve/Reject
- [ ] Subagent-чипы + fullscreen; Plan/Todos/Agents/Console панели
- [ ] Console показывает живой PTY-вывод
- [ ] Attach active file из GPUI-редактора (editor-context цепочка)
- [ ] MCP статус-бар элементы + клик в CzMcp
- [ ] 10 customize-страниц открываются; role-gating не шлёт heavy-каналы
- [ ] Тултипы из вебвью рисуются шеллом; тема пробрасывается
- [ ] Voice input (Whisper URL из kamin-host) работает
