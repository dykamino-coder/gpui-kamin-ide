# Совместимость Claude Code CLI

Текущий образ закрепляет **2.1.322** в `Dockerfile`. На 2026-11-10 npm
`dist-tags.latest` возвращал `2.1.322`. Выбор следует поручению владельца
использовать npm `latest`, без диапазона или auto-update. Версии самого Bridge
и workspace этим обновлением не меняются. CLI mod (`src/cli-mod`) по-прежнему
привязан к одной точной версии: `CLI_MOD_VERSION`, проверка в `bridge.js` и
tests меняются вместе с pin.

## Проверка перехода с 2.1.312 на 2.1.322

Источники: changelog 2.1.313–2.1.322 из npm package `2.1.322` и сравнение
Linux x64 binaries `2.1.312` / `2.1.322` (строки контрактов и env-переменных).

| Контракт            | Изменения upstream и решение Bridge                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI mod             | 2.1.318 оставляет вложения мода из `prompt.submit` в user turn (Bridge ничего не прикладывает). 2.1.321 автоматически включает моды, _написанные Claude_, в auto/bypass и не грузит их в `dontAsk`; это не касается `--plugin-dir` и не меняется Bridge. API `prompt.submit`/`session.append`/`turn.*`/`http.fetch` для Bridge совпадает: `claude plugin test` проходит на обеих версиях, синтетическая проба (ниже) даёт одинаковые hello, submit-ack, read-receipt, turn start/complete и abort.               |
| JSONL / resume      | Новые служебные записи `session-owner`, `resume-head-vouch`, `repl-mode` добавлены в списки не отображаемых типов webview и server segment index. Новые attachment-типы `message_pdf_reads`, `widget_context_hint`, `claude_app_notice` уже скрыты allowlist-предикатом; удалённые `file_unchanged` (Read stub) и `output_limit_note` в старых transcripts тоже остаются скрытыми. 2.1.314 исправляет resume/rewind после смены модели; 2.1.317 больше не хранит прежний thinking при resume (снят fix 2.1.288). |
| Model / effort      | Тексты cache-warning диалога (`Switch model?`, `Change effort level?`, `This conversation is cached for the current …`, `Yes, switch to …`) и `PreModelSwitch` сохранены. `/model` picker с 2.1.319/2.1.321 группирует старые модели под «More models», но Bridge передаёт `/model <id>` и по строкам picker не навигирует. 2.1.321 добавляет `claude-fable-5-2` и переводит alias `fable` на него; сохранённые выборы 5.1 переходят на `fable`.                                                                 |
| MCP                 | 2.1.319: MCP Tasks в терминале работают без fetch feature flags; `CLAUDE_CODE_DISABLE_MCP_TASKS=1` выключает их, `CLAUDE_CODE_DISABLE_MCP_TASK_BACKGROUND=1` заставляет ждать внутри tool call. Bridge снова включает `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` и удаляет унаследованные оба opt-out, чтобы длинные локальные tools оставались фоновыми задачами.                                                                                                                                             |
| Environment         | Все env-переменные из `session-env.ts`/`session-settings.ts` присутствуют в 2.1.322. 2.1.319 меняет auto mode без feature flags (Bridge использует bypass) и self-paced `/loop` (Bridge не использует).                                                                                                                                                                                                                                                                                                          |
| Statusline / limits | `rate_limits.five_hour/seven_day`, `Current session` / `Current week` в `/usage` сохранены. При essential-traffic стартовый quota probe пропускается; `account-limits.ts` сохраняет последние окна.                                                                                                                                                                                                                                                                                                              |

Gate 2.1.322 (2026-11-10, временные Podman-контейнеры `bridge-cli322-*` из
`open-claude-bridge:6.3.164` с подмонтированным binary 2.1.322):

- `claude plugin test` для `src/cli-mod`: 3/3 на 2.1.312 и на 2.1.322;
- синтетическая проба без credentials: loopback fake Messages API и fake
  Bridge UDS transport, интерактивный `claude --plugin-dir` под `script`.
  На обеих версиях: один hello, submit → ack и read receipt с uuid записи,
  turn start/complete, второй submit и abort активного turn (ack, закрытие
  запроса к API, turn complete).

Не выполнено: authenticated live turn с реальной моделью, MCP tool round trip,
SessionStart/Stop hooks, statusline, detach/resume на образе этой ветки и MCP
Tasks при essential-traffic. Сценарий: собрать образ этой ветки, в отдельном
контейнере с выданной для приёмки авторизацией пройти start → turn → MCP tool →
detach → resume → turn и фоновую MCP Task. Это не Windows/CEF-приёмка.

## Проверка перехода с 2.1.296

Источники: официальный [changelog](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md),
[hooks](https://code.claude.com/docs/en/hooks),
[MCP](https://code.claude.com/docs/en/mcp), npm package `2.1.312`, включая
встроенные исходники Linux x64 binary. Исторические fixtures и свидетельства
для `2.1.296` сохраняют свою исходную версию; они не объявляются новой приёмкой.

| Контракт            | Изменения upstream и решение Bridge                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| JSONL / resume      | 2.1.300 добавляет записи system prompt/tools/parameters; исправляет запись после torn tail. 2.1.309 исправляет Unicode tool previews; 2.1.311 — порядок print/SDK transcripts; 2.1.312 — потерю первого ответа после resume. Не считать каждую строку видимым сообщением, не переписывать неизвестные записи. Проверять start → turn → detach → resume → turn. Производительность/repair больших файлов остаётся INC-0078.                                                                                          |
| Hooks / permissions | Новая валидация `updatedInput` (2.1.300) и ограничения параллельности prompt/agent hooks (2.1.302/312) не меняют command-hook relay Bridge. `PreModelSwitch` и его `permissionDecision` сохранены. Native `Read` остаётся запрещён через CLI args, исключение из settings deny сохраняется до отдельного доказательства безопасного удаления. Не расширять автоматическое разрешение пользовательских ask/deny.                                                                                                     |
| Model / effort      | 2.1.312 убирает обещание re-read «on your next message» из cache warning. Заголовок, cache identity и целевой `Yes, switch to …` сохранены; matcher Bridge не зависит от изменённого окончания. `CLAUDE_CODE_EFFORT_LEVEL` продолжает переопределять live effort, поэтому Bridge удаляет inherited env и передаёт стартовое значение флагом.                                                                                                                                                                        |
| MCP                 | 2.1.299 включает modern negotiation, 2.1.302 исправляет пропущенные поля modern responses, 2.1.307 ограничивает повторы HTTP calls idempotent/read-only tools. В этом обновлении сохраняется явный legacy fallback `server/discover` → `initialize`; нельзя просто объявить `2026-07-28` на старой реализации. Полная modern discovery и Tasks — отдельная задача F.                                                                                                                                                |
| Environment         | `IS_DEMO`, `CLAUDE_CODE_NO_FLICKER`, resume/idle thresholds и ограничения native background/memory/marketplace сохраняются. `DISABLE_NONESSENTIAL_TRAFFIC=1` выключает fetch flags, необходимый CLI для MCP Tasks; это известная зависимость F, а не доказательство отсутствия Tasks в CLI. `DISABLE_BACKGROUND_TASKS=1` само по себе MCP Tasks не выключает.                                                                                                                                                       |
| Mouse / fullscreen  | `NO_FLICKER=1` по-прежнему выбирает fullscreen/alternate screen. 2.1.311 переносит действие scrollbar click на release. Колесо и mouse modes принадлежат приложению; normal-screen scrollback и выделение модификатором принадлежат отдельному INC-0079. Не добавлять глобальный wheel → PageUp.                                                                                                                                                                                                                    |
| Statusline / limits | Версия извлекается из payload, не сравнивается с фиксированным номером. Structured `rate_limits.five_hour/seven_day`, context/model/effort остаются совместимыми. 2.1.312 переименовывает limit messages в «5-hour limit», но `/usage` всё ещё использует `Current session` / `Current week`; текущий parser остаётся применимым. 2.1.299 исправляет значения свыше 100%; sanitizer Bridge сохраняет свою проверку диапазона. Не экспортировать transcript paths, session id, workspace или cost из raw statusline. |

## Gate обновления

Server: typecheck, lint, полный Vitest; отдельно compatibility fixtures
live-switch, statusline, plan-usage, discovery и resume. В изолированном
временном контейнере из образа этой ветки проверить фактическую версию CLI, новую
сессию, MCP tool round trip,
SessionStart/Stop command hooks, sanitized statusline, detach/resume и следующий
ответ. Сохранять только результаты/счётчики и точные SHA/версии; credentials,
полный профиль и raw transcripts не публиковать. Не считать эти проверки
живой Windows/CEF-приёмкой INC-0079 или закрытием BR-07/BR-11.

Authenticated gate для 2.1.312 пройден в отдельном временном Podman-контейнере:
SessionStart/Stop, ответ новой сессии, MCP round trip, sanitized statusline,
переподключение к живому PTY и cold resume с последующим ответом. Авторизация
предоставлена через существующий auth volume; остальные настройки и данные
сессии изолированы. Этот gate не подтверждает account flag для MCP Tasks.
