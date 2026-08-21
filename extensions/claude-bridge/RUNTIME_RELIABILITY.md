# Claude Bridge runtime reliability backlog

## Scope and evidence policy

Здесь ведутся только проблемы KaminIDE/Claude Bridge runtime. Содержимое и
business logic сторонних plugins находятся вне scope; общий Bridge UI и relay
остаются в scope, даже если дефект впервые проявился на конкретном plugin.

Статусы:

- **ready** — причина или нарушенный контракт локализованы достаточно для
  отдельного change PR;
- **investigation** — симптом подтверждён, но исправление без дополнительных
  данных было бы догадкой;
- **deferred** — изменение осознанно не планируется до указанного условия;
- **verify** — код пока не меняется, нужен целевой runtime-прогон.

Наблюдения ниже зафиксированы 20 августа 2026 года на KaminIDE 1.0.53. Source
аудит выполнен на `origin/main` commit `5b5d93d`.

## Confirmed incident facts

Windows `crash.log` содержит пять `TS_PROCESS_OOM` для CEF views в трёх main
process:

- `claudeBridgeChat` + `claudeBridgeConsoleView`;
- `claudeBridgeChat` + `claudeBridgeTodosView`;
- `claudeBridgeChat`, после чего зафиксирован новый main process.

Это подтверждает renderer Out of Memory, но не доказывает единственную причину
OOM. Текущий runtime уже ограничивает часть retained JSONL/subagent state;
остаточный источник или сочетание источников требуют измерения.

Для затронутой длинной session ранее был выгружен snapshot:

| Метрика | Наблюдаемое значение |
| --- | ---: |
| Размер JSONL | 47 662 635 bytes |
| Строки JSONL | 18 001 |
| Максимальная строка | 643 475 bytes |
| Compact boundaries | 10 |

Snapshot был получен до более позднего продолжения session, а `crash.log` не
содержит timestamp и crash-time memory/JSONL size. Поэтому эти числа описывают
известную проблемную session, но **не являются порогом падения**. Размер файла
на диске также не равен retained JS/CEF memory: один payload может существовать
в нескольких parsed/projected/view representations.

Дополнительно подтверждено:

- после restart server session продолжала работать в Console, а Chat composer
  оставался в `Connecting to bridge...`;
- после tab switch и после disconnect/reconnect Chat иногда оставался в
  `Loading conversations...` до движения курсора или более позднего UI event;
- parent process публикует `kamin:exthost:respawned`, но runtime не содержит
  consumer этого события для восстановления contributed webviews;
- `host.log` открывается с `flags: "w"` и теряет дорестартовую историю;
- строки `crash.log` не имеют timestamp, а собранный incident bundle не содержал
  отдельного memory watchdog report.
- при прокрутке длинной session вверх Chat заметно меняет позицию viewport и
  scrollbar thumb в момент подгрузки предыдущего окна; вниз тот же эффект не
  наблюдается. Source audit подтверждает нарушенный anchor contract, описанный
  в BR-16, но Windows CEF runtime ещё должен измерить величину displacement.

Agent Teams и hook approval имеют отдельные подтверждённые границы:

- Bridge принудительно включает experimental Agent Teams, а уведомление
  teammate `finished`/`idle` не содержит его отчёт;
- повторный `SendMessage` доставляет teammate reports в Console и Chat, поэтому
  наблюдавшаяся потеря была в постановке задачи lead agent, не в Bridge
  transport или renderer;
- hook approval modal показывает `handler.command`, но скрывает `handler.args`:
  для `node hooks/guard.mjs` пользователь видит только `node`;
- длинный matcher выводится одной строкой без читаемой структуры и bounded
  wrapping, хотя от его содержания зависит область автоматически запускаемого
  hook.

Релизный audit также выявил operational gap. Опубликованный image `6.3.130`
появился до release commit и не содержит source revision label. GitHub workflow
затем завершился зелёным, но пропустил build, потому что version tag уже
существовал. GitHub Release `kaminide-latest`, который workflow обязан скачать
при реальной сборке, отсутствует. Зелёный skip не доказывает, что image собран
из одобренного release HEAD.

## Session size decision

Безопасный числовой предел сейчас неизвестен. Из одного affected sample нельзя
вывести размер, ниже которого session гарантированно безопасна, или размер, при
котором она обязательно упадёт.

До исследования действует только доказательная эксплуатационная политика:

1. Session, уже вызвавшая renderer OOM либо воспроизводимое падение при своей
   загрузке, считается retired; продолжение переносится в новую session через
   краткий handoff. Отдельный native extension-host fail-fast без такой связи
   сам по себе не является основанием считать session повреждённой.
2. `/compact` не считается уменьшением JSONL на диске или гарантией освобождения
   всех retained UI structures.
3. Значения из таблицы — known-risk sample, не green/red threshold.
4. Автоматический warning, hard limit и обещание безопасного размера не
   добавляются до измерений.

Архитектурное sharding/parallel rendering длинной истории осознанно не
планируется. Сначала измеряется envelope; затем выбирается минимальная мера:
операционный лимит, byte-based eviction/windowing или точечное устранение
retention. Полная переработка допускается только если меньшая мера не держит
измеренный envelope.

## Ordered work

### BR-01 — Durable incident diagnostics

**Status:** ready. **Dependency:** none. **Acceptance:** automated merge gate +
post-merge production observation; Windows crash reproduction не блокирует
merge диагностического PR.

Изменение:

- bounded rotation вместо безусловной потери предыдущего `host.log`;
- timestamp, app/build version, process/view id и termination status в crash
  records;
- sanitized tab-scoped connection transitions и причина reconnect/error;
- bounded memory/retention counters на pressure/crash boundary;
- bounded rolling pre-crash samples, потому что heap умершего CEF renderer
  после termination уже недоступен;
- отсутствие tokens, prompts, hook payloads и file contents в telemetry.

Tests проверяют rotation, schema, bounds и redaction. Production observation
подтверждает, что следующий incident bundle содержит pre-crash цепочку.

### BR-02 — Calibrate the long-session memory envelope

**Status:** investigation. **Dependency:** BR-01. **Acceptance:** отдельный
research artifact, не functional PR.

На Windows воспроизводятся одинаковые transcript prefixes разного byte/entry
состава: крупные tool results, subagent streams и compacted history. Для одного
и нескольких открытых views фиксируются CEF process memory, retained counters,
load/switch latency и факт crash/recovery. Методика должна быть повторяемой;
результат обязан отдельно назвать supported workstation profile и запас, по
которому выбран operational limit.

Deliverable: доказанный рабочий диапазон либо вывод, что file size не годится
как predictor и нужен другой observable metric.

### BR-03 — Long-session mitigation

**Status:** deferred. **Dependency:** BR-02.

До измерения не выбираются sharding, новый renderer или произвольный size cap.
После BR-02 оформляется отдельный change PR с минимальным достаточным решением.

### BR-04 — Recover webviews after extension-host respawn

**Status:** ready. **Dependency:** none. **Acceptance:** automated + Windows
runtime merge gate.

Shell должен обработать `kamin:exthost:respawned`, заново получить contributions
и восстановить view providers/state без полного restart приложения. Нужны tests
event routing и Windows-сценарий: принудительный child respawn, затем Chat,
Console и соседняя contributed view возвращаются и показывают выбранную session.
Повторный respawn не создаёт duplicate listeners/status items, stale events
предыдущего child generation игнорируются, а open documents, active editor,
selections и LSP state повторно seed-ятся в новый child.

### BR-05 — Rehydrate authoritative connection state

**Status:** ready. **Dependency:** none. **Acceptance:** automated + Windows
runtime merge gate.

Нужно устранить расхождение host/session state и Chat tab state:

- не терять connection event, пришедший до tab snapshot;
- не затирать более новое состояние поздним `listTabs`/`tab:list-changed`;
- не переводить authenticated session в UI `connecting` после временного
  `session:error`;
- при webview mount/reload выдавать один versioned authoritative snapshot.

Tests: event-before-tab, stale snapshot, transient error, cold app restart при
живой server session и несколько tabs. Старый пятисекундный error timer не
меняет новое connection generation; authenticated state и `sessionId`
сохраняются, composer остаётся enabled, а stale `listTabs` не откатывает
revision.

### BR-06 — Webview update stalls until pointer activity

**Status:** investigation. **Dependency:** BR-01 желателен. **Acceptance
будущего fix:** automated + Windows runtime merge gate.

Подтверждены два проявления одного класса, но общая причина пока не доказана:

1. switch из session с активной работой в другую session;
2. disconnect → reconnect текущей session.

В обоих случаях Chat может остаться в loading, пока Console уже обновляется;
движение курсора либо поздний event запускает видимый update. Это одна задача,
а не две root-cause гипотезы.

До правки нужно записать sequence tab/replay/connection events и CEF paint/frame
invalidation. Windows acceptance выполняет оба сценария без движения мыши,
смены focus и ручного resize; loading обязан завершиться сам.

### BR-07 — Surface native Claude attention in Chat

**Status:** ready для минимального safe UX; structured approval требует spike.
**Dependency:** none. **Acceptance:** automated + Windows runtime merge gate.

Hook relay корректно возвращает `permissionDecision: "ask"`, после чего Claude
Code показывает native TUI только в Console. Но `Notification` также включает
неблокирующие idle nudges и questions: одного факта notification недостаточно,
чтобы объявить permission prompt. Chat не имеет protocol request id,
question/options и не может безопасно ответить обычным composer.

Первый независимый PR:

- не теряет `notificationType`/`notificationMessage`;
- после узкой classification из BR-11 показывает tab-scoped нейтральный banner
  «Claude ожидает действия в Console» только для blocking state;
- открывает/focus Console по кнопке;
- не позволяет composer случайно стать raw TUI answer;
- переживает tab switch и reconnect.

Structured allow/deny widget делается отдельно только после e2e spike с
закреплённой Claude Code version и `PermissionRequest` hook. ANSI/TUI parsing не
используется как protocol.

### BR-08 — Explain unexpected automatic `/reload-skills`

**Status:** investigation. **Dependency:** диагностические поля из BR-01 можно
реализовать узко в этом PR. **Acceptance:** automated + post-merge production
observation. Windows runtime merge gate потребуется отдельному behavioral fix,
если telemetry подтвердит дефект.

Текущий source contract описан в `SKILLS_SYNC.md`: tab switch не планирует
reload, changed skills sync планирует, а pending maintenance может выполниться
после reattach. Нужно добавить revision/reason telemetry и сопоставить её с
полевым transcript. До этого автоматический reload не удаляется.

### BR-09 — Make Agent Teams report delivery explicit

**Status:** ready для soft hardening. **Dependency:** none. **Acceptance:**
automated + authenticated Windows runtime merge gate.

Agent Teams остаются включёнными по умолчанию. Bridge system prompt должен
зафиксировать общий контракт делегирования:

- lead ставит bounded task и ожидаемый deliverable каждому teammate;
- teammate до перехода в idle отправляет bounded self-contained report и ссылки
  на созданные artifacts через
  `SendMessage` получателю `team-lead`;
- lead не считает `finished`/`idle` отчётом;
- при idle без report lead делает один recovery request, а не запускает нового
  teammate и не ждёт бесконечно.

Unit test проверяет наличие и порядок инструкции относительно user-provided
instructions. Windows live gate создаёт три teammates с уникальными markers и
отдельно подтверждает: (1) 3/3 reports пришли с первой попытки без recovery при
tab switch/reconnect, (2) искусственно потерянный report вызывает ровно один
recovery request без duplicate teammate и бесконечного ожидания. Автоматический
выбор team вынесен в BR-15. Блокирующий `TeammateIdle` hook и чтение mailbox не
входят в первый PR.

### BR-10 — Show the effective hook in approval UI

**Status:** ready. **Dependency:** none. **Acceptance:** automated + Windows UI
runtime merge gate.

Approval hash и повторный review после изменения hook сохраняются. Modal должна
без исполнения показать canonical pre-rewrite declaration (`command` + `args`
или другой handler shape), анализировать
опасные tokens во всех его частях, redacted отображать secrets/env/authorization
и читабельно отображать matcher. Новый неизвестный hook не должен быть заранее
выбран только потому, что эвристика не нашла dangerous token. Нужны bounded
wrapping/overflow, copy или equivalent full-text access, а также
hover/click/focus/keyboard checks.

Tests покрывают `command`, `prompt`, `agent`, `http`, `mcp_tool`, `node` с script
path в `args`, shell metacharacters, опасный argument, redaction и очень длинный
matcher. Relay URL/token после proxy rewrite не отображаются. Windows gate:
plugin update меняет hook hash, modal показывает полную declaration и matcher,
approve/reject сохраняются.

### BR-11 — Inventory native CLI-only blocking states

**Status:** investigation. **Dependency:** none; classification precedes BR-07.
**Acceptance:** research artifact, затем отдельные PR по подтверждённым protocol
gaps.

MCP permission, `AskUserQuestion` и plan уже имеют structured widgets. Нужно
проверить остальные native Claude CLI states: permission/auth/trust/plugin/team
prompts и определить, где достаточно общего tab-scoped blocking state, а где
нужен отдельный protocol/widget. ANSI/TUI parsing не считается допустимым
универсальным transport.

### BR-12 — Establish the server and builtin skills baseline

**Status:** investigation/verify. **Dependency:** none. **Acceptance:**
versioned inventory с источником каждого skill (deployment/CLI builtin/user/
project/plugin), exact roots и mounts, без содержимого, tokens и secrets.

Ранее agent report сообщил об отсутствии deployment-owned
`bridge-activity-report`, `my-activity`, `analyze-token` и части Anthropic
builtins, одновременно подтвердив plugin skills. Эти server skills отсутствуют
в repository history, поэтому source of truth и причина исчезновения кодом не
доказаны.

На актуальном release нужен raw new-session inventory и аудит container/global
`~/.claude`, CLI builtin sources и deployment volumes. Если server-managed
skills лежали внутри client-owned exact snapshot, их нужно вынести в отдельный
read-only overlay/root и восстановить из deployment backup. До инвентаря нельзя
объявлять проблему закрытой plugin sync PR или создавать вымышленные assets.

### BR-13 — Retire the legacy bridge-sync mount safely

**Status:** deferred operational migration. **Dependency:** подтверждение
миграции всех deployments. **Acceptance:** backup, migration marker/count/hash,
два успешных restart, rollback probe и доказательство отсутствия чтения legacy
volume.

Новые snapshots используют `/app/data/bridge-sync`, но compose пока монтирует
legacy `/home/bridge/bridge-sync` для one-time migration. Volume и mount нельзя
удалять вручную, пока каждый deployment не подтвердил перенос и не определена
backup/rollback policy. После этого cleanup оформляется отдельным PR и runbook;
он не совмещается с runtime fixes.

### BR-14 — Make release artifact provenance verifiable

**Status:** ready как отдельный process/CI PR. **Dependency:** none.
**Acceptance:** automated merge gate + dry-run доказательство workflow.

Release pipeline должен fail closed, если version tag уже существует, но его
source revision нельзя связать с одобренным release HEAD. Manual image upload не
заменяет pipeline; image получает revision/source labels и attestations, а
workflow проверяет digest/labels после publication. Installer source Release
создаётся и проверяется до merge согласно `CONTRIBUTING.md`; отсутствие
`kaminide-latest` не должно маскироваться зелёным skip. Actions pin-ятся на
полные commit SHA; installer asset version и digest проверяются перед build.

### BR-15 — Evaluate automatic Agent Teams selection

**Status:** investigation/eval. **Dependency:** BR-09.

Нужен versioned набор small, parallelizable и strictly sequential prompts,
лимиты teammate count/token cost и критерии: parallel tasks используют bounded
team, small/sequential задачи не создают лишних teammates, итог содержит все
reports без дублей. Только после стабильного eval выбирается дополнительная
system instruction или orchestration layer; недетерминированное обещание «всегда
правильно оценить объём» не входит в BR-09.

### BR-16 — Stabilize upward history scroll anchoring

**Status:** ready. **Dependency:** none. **Acceptance:** automated + Windows CEF
runtime merge gate.

Текущий scroll-up path нарушает собственный anchor contract в двух проверяемых
местах:

- `useChatScrollPin` вызывает `captureAnchor()` до `onNearTop`, поэтому stale
  anchor остаётся даже когда новый render window/page не запущен из-за lock,
  `reachedStart`, `SCROLL_UP_MAX` или отсутствующего `_pos`;
- `restoreAnchor()` не хранит исходный `scrollHeight` и проверяет
  `scrollHeight <= scrollHeight - scrollTop`. При любом `scrollTop > 0` условие
  ложно даже без роста документа, поэтому первая посторонняя DOM mutation может
  быть ошибочно принята за завершившийся prepend.

Дополнительно один trigger монтирует до 400 строк с
`content-visibility:auto` и временным `contain-intrinsic-size: auto 80px` на
wrapper и внутренней card. Реальная высота длинных messages/tool results
уточняется при приближении к viewport, меняя `scrollHeight`. Одновременно
оставлен browser `overflow-anchor:auto` и выполняется ручная запись
`scrollTop`, то есть единственного владельца коррекции позиции сейчас нет.

Fix PR должен:

- arm anchor только после подтверждения, что конкретный prepend/render-window
  growth действительно начат, и связать его с request/generation;
- выбрать один способ anchoring. Предпочтительный spike — stable keyed visible
  entry + pixel offset: после commit найти тот же DOM node и компенсировать
  изменение его `getBoundingClientRect().top`; browser anchoring для этого
  scroller явно отключить;
- игнорировать unrelated streaming/widget mutations и stale page responses;
- сохранить downward scroll, bottom pin, tab scroll memory и resident-store
  bounds без увеличения memory envelope.

Automated tests моделируют variable-height rows, unrelated mutation до ответа,
несколько wheel events во время одного load, no-op около начала transcript и
два последовательных prepend. Windows gate на длинной session повторяет
scroll-up через несколько 400-row boundaries и фиксирует viewport displacement
до/после каждого prepend; выбранный anchor не должен сдвигаться больше чем на
2 px, а scroll вниз и live streaming остаются плавными.

## Recommended PR order

1. BR-10 informed hook approval UI.
2. BR-05 authoritative connection state.
3. BR-09 Agent Teams soft reporting contract.
4. BR-01 diagnostics.
5. BR-04 recovery после extension-host respawn.
6. BR-14 release provenance guard идёт независимо и не блокирует runtime chain.
7. Повторный Windows-прогон tab switch и disconnect/reconnect. BR-06 создаётся
   как fix PR только если симптом сохранился; отдельная задача для reconnect не
   заводится.
8. BR-16 upward history anchoring идёт отдельным UI PR и не блокирует connection
   recovery chain.
9. BR-11 inventory native blockers, затем минимальный BR-07.
10. BR-08 без удаления существующего maintenance contract.
11. BR-12 deployment skills baseline; BR-13 независимо ждёт подтверждения
    legacy migration на всех deployments.
12. BR-15 Agent Teams selection eval.
13. BR-02 и только затем решение по BR-03.

Каждый PR остаётся change PR без version bump. Release и production rollout
выполняются отдельно по `CONTRIBUTING.md`.
