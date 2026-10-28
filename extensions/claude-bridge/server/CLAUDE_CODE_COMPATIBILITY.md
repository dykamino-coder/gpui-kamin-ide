# Совместимость Claude Code CLI

Текущий образ закрепляет **2.1.312** в `Dockerfile`. На 2026-10-28
`npm view @anthropic-ai/claude-code version` и `dist-tags.latest` возвращали
`2.1.312` (отдельный tag `stable` возвращал `2.1.308`). Выбор следует
поручению владельца использовать npm `latest`, без диапазона или auto-update.
Версии самого Bridge и workspace этим обновлением не меняются.

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
`kamin-acc-302` проверить фактическую версию CLI, новую сессию, MCP tool round trip,
SessionStart/Stop command hooks, sanitized statusline, detach/resume и следующий
ответ. Сохранять только результаты/счётчики и точные SHA/версии; credentials,
полный профиль и raw transcripts не публиковать. Не считать эти проверки
живой Windows/CEF-приёмкой INC-0079 или закрытием BR-07/BR-11.
