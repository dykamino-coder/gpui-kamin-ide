# Ревизия завершённости runtime-задач — 2026-09-06

Поручение: закрыть остатки уже реализованных задач, исправить статусы и
передать недостающую приёмку maintainer. Новые functional fixes, работа через
Computer Use на машине владельца и release не входят в этот проход.

## Snapshot и проверяемый результат

Исходный `origin/main`: `818ae0833d0ce491b45c3a4afa7495e31f7dba80`, после
fetch и сверки с GitHub. Открытых PR — 0, GitHub Issues (включая closed) — 0;
задачи ведутся в `RUNTIME_RELIABILITY.md`, `RUNTIME_EXECUTION.md` и INC-карточках.
Проверены все 31 BR-карточка и три существующих incidents. История задачи
«Исследовать проблему в origin main» использована как контекст; статусы
сверены с актуальным source, PR и записанной приёмкой.

Из 20 первоначально открытых BR-задач ни одна не имеет одновременно полной
реализации в `main` и необходимого evidence для закрытия. Их остатки теперь
указаны непосредственно в карточках. В отдельных случаях код уже частично
сделан: BR-25 baseline и BR-26/27 выполнены, BR-08 имеет maintenance revision,
BR-04 имеет respawn event, BR-24 имеет local approval timeout. Это не закрывает
оставшийся контракт соответствующей задачи.

BR-30 был помечен `done`, хотя отдельный Windows filesystem smoke в #31 не
выполнялся. Этот остаток выполнен через [PR #47](https://github.com/dykamino-coder/gpui-kamin-ide/pull/47):
существующие tests запущены на GitHub Windows runner. После merge сделан новый
fetch, сверены изменённые файлы с PR и проверен Windows run точного `main`
`686cc92b6c935e8ffb2416cf7b3b6f22c6f19ba2`. Runtime source не менялся.

## Снятые преждевременные `done`

| ID | Код уже в main | Какого evidence не хватает |
| --- | --- | --- |
| BR-05 | #14, authoritative connection state | Send после recovery, close tab внутри reconnect, max-sessions и session-not-found на server из проверяемого main; [историческая приёмка](https://github.com/dykamino-coder/gpui-kamin-ide/pull/14#issuecomment-5494274741) прямо перечисляет пропуски и server другого build |
| BR-09 | #15, Agent Teams report contract | 3/3 reports при tab switch/reconnect и отдельный один recovery; [live gate не выполнялся](https://github.com/dykamino-coder/gpui-kamin-ide/pull/15#issuecomment-5494415820) |
| BR-10 | #13, hook approval UI | Фактические focus/keyboard interactions; [приёмка](https://github.com/dykamino-coder/gpui-kamin-ide/pull/13#issuecomment-5493712704) содержит NOT DRIVEN для containment |
| BR-29 | #37, hover→rename helper и id-scoped anchor | Input focus/typing и настоящие keyboard/hitbox paths; [PR #37](https://github.com/dykamino-coder/gpui-kamin-ide/pull/37) явно ограничивает probe evidence |

Это очередь `verify` существующего кода. Отсутствие evidence не доказывает новый
дефект, а source review не заменяет runtime gate. Maintainer с подходящим
Windows environment получает пачку `RV-2026-09-06`; подробные сценарии и
ожидаемый artifact находятся в исходных карточках. Если gate не проходит,
нужно сохранить незавершённый статус и связать bounded defect; functional
исправление не входит в эту пачку.

## Сохранённые результаты и открытые остатки

- BR-01: diagnostics #16 выполнены; следующий редкий incident — observation,
  OOM не объявлен исправленным.
- BR-14: provenance #43–#44 выполнена; первый настоящий automatic release
  остаётся observation release maintainer, с отдельной авторизацией release.
- BR-18: teardown relay fix #20 выполнен; исходный termination trigger ждёт
  BR-17 logs. Ранее незаведённое наблюдение об отменённом local SessionEnd hook
  оформлено отдельным BR-18A для classification, без нового fix.
- BR-26/#36 и BR-27/#34: атомарный replay и partition выполнены. Их Windows
  evidence использует forced repaint для обхода BR-31 и не закрывает BR-25
  completion.
- BR-28/#35: diagnostic outcome not reproduced сохранён в пределах семи
  проверенных случаев. Это не заявление об исправлении всех layouts.
- BR-30/#31+#47: implementation и Windows filesystem acceptance выполнены.
- BR-02 освобождён от уже завершённого prerequisite BR-30; сами Windows memory
  измерения ещё нужны. BR-06 ждёт повторного прогона после BR-31. BR-12/13/22
  требуют конкретного evidence владельца; BR-03/07/15/16 сохраняют зависимости.
- INC-2026-0001 остаётся investigation; INC-2026-0002 — confirmed, без BR-31 fix;
  INC-2026-0003 — rejected/not reproduced в записанной матрице.

Итого после ревизии: **7 done и 24 незавершённых из исходных 31 BR-задач**, плюс
новая verification-карточка BR-18A. BR-21A–E остаются планируемыми child
deliverables после decision PR, а не выполненными задачами. Историческая пачка
RB-2026-09-A обозначена как законченный execution pass с незавершённой
приёмкой; это не повторное объявление всего runtime backlog закрытым.

## Выполненные проверки

На неизменённом source `818ae083`, macOS / Node 22.23.1, все команды завершились
с exit 0. Всего **99 tests**; ниже только фактически запущенные suites.

| Команда | Результат |
| --- | --- |
| `npm test -- src/kamin-host/incident-log.test.ts src/kamin-host/rolling-log.test.ts` | 2 files / 12 tests |
| `npm --prefix extensions/claude-bridge/server test -- src/core/pty/bridge-default-claude-md.test.ts src/core/pty/session-settings-skills.test.ts src/core/hooks/hook-relay-teardown.test.ts src/core/hooks/hook-relay-secret.test.ts` | 4 files / 16 tests |
| `npm --prefix extensions/claude-bridge/server test -- src/core/pty/session-teardown.test.ts src/core/pty/session-error.test.ts` | 2 files / 10 tests |
| `npm --prefix extensions/claude-bridge/extension test -- src/main/ws/connection-state.test.ts src/incident-diagnostics.test.ts` | 2 files / 8 tests |
| `npm --prefix extensions/claude-bridge/webview test -- src/signals/tab-connection-reconcile.test.ts src/components/customize/hooks/hook-approval-display.test.ts src/signals/agent-replay.test.ts src/signals/agent-partition.test.ts src/hooks/useAgentTree.test.ts` | 5 files / 41 tests |
| `npm run test:release` | 12 tests |

PR #47: `actionlint` 1.7.12 и `git diff --check` — PASS. Windows / Node 22.23.2:

- [candidate run 34030125517](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/34030125517):
  `53bbcb5` = head `2c7584b` + base `818ae083`, 2 files / 12 tests PASS;
- [main run 34030247540](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/34030247540):
  `686cc92b6c935e8ffb2416cf7b3b6f22c6f19ba2`, 2 files / 12 tests PASS;
- [main quality run](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/34030247586)
  — PASS;
- [release context run](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/34030262420):
  validation success, `publish verified release` skipped — публикации нет.

Данная coordination-правка меняет только docs. Для неё применяются scope и
whitespace CI gates; повторные Rust/UI/build suites не требуются. Отсутствующий
Windows UI, authenticated CLI или deployment artifact не отмечается как PASS.
Versions, runtime/generated artifacts и lockfiles не менялись; release не нужен.
