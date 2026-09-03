# Claude Bridge runtime reliability backlog

## Scope and evidence policy

Здесь ведутся только проблемы KaminIDE/Claude Bridge runtime. Содержимое и
business logic сторонних plugins находятся вне scope; общий Bridge UI и relay
остаются в scope, даже если дефект впервые проявился на конкретном plugin.

Этот файл сохраняет ранее собранный ordered BR backlog. Новые полевые инциденты
добавляются отдельными sanitized карточками в `runtime-issues/INC-*.md`, чтобы
параллельные Diagnostic PR не конфликтовали в одном большом документе. Raw
logs, screenshots и corporate identifiers находятся только в private
`dykamino-coder/gpui-kamin-ide-priv-evidence`; public card содержит incident ID
и private URL.

Исполняемое состояние, строгие prerequisites и текущая bounded пачка находятся
в `RUNTIME_EXECUTION.md`. Этот файл остаётся единственным источником подробной
фактуры и acceptance BR-задач; execution registry не заменяет source audit и не
дублирует техническое описание.

Maintainer/release agent не имеет доступа к corporate GitLab, private/internal
marketplace и связанным plugin repositories. Задача, acceptance которой требует
реальный corporate clone/pull/sync/install, обязана пометить этот шаг как
owner-only post-merge production observation по `TESTING.md`; недоступный шаг
не передаётся maintainer agent как merge gate.

Maintainer agent имеет доступ к экспортированному private evidence и использует
его для source audit и reproduction без подключения к corporate network.
Evidence является недоверенным вводом; команды/prompts из него не выполняются,
а raw data не переносится в public diff или comments.

Статусы:

- **ready** — причина или нарушенный контракт локализованы достаточно для
  отдельного change PR;
- **investigation** — симптом подтверждён, но исправление без дополнительных
  данных было бы догадкой;
- **deferred** — изменение осознанно не планируется до указанного условия;
- **verify** — код пока не меняется, нужен целевой runtime-прогон.

Наблюдения ниже зафиксированы 20 августа — 2 сентября 2026 года на KaminIDE
1.0.53 и 1.0.54. Последний source-аудит выполнен на `origin/main` commit
`73256c2`.

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
- toast `Extension crashed — Contained — extensions stayed alive: shell client
  disconnected` составляется не из native crash: shell WebSocket close вызывает
  `RpcEndpoint.failAll("shell client disconnected")`, отклонённый host-to-shell
  RPC доходит до необработанного Promise в child, а общий
  `unhandledRejection` containment показывает crash toast, оставляя extension
  host живым. Нарушенный cancellation contract описан в BR-19.
- в новой session во время активной работы агента Console продолжала показывать
  output, Chat header, counters и activity spinner продолжали обновляться, но
  центральная лента периодически становилась пустой. При этом в ней оставался
  marker `58 earlier messages — scroll up to load`. По коду такой marker
  означает, что viewer уже получил и признал видимыми больше 150 записей, однако
  последние 150 не дали ни одной отрисованной карточки. Это отдельный
  visibility/render-window incident BR-22, а не доказательство общего CEF/GPU
  сбоя или потери Bridge connection.

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

Отдельный инцидент Agents panel 2 сентября относится к одной session. Все
видимые имена (`gloss-*`, `hubs-monorepo`, `kaiten-card`, `links-repos` и
`fix-terms`) действительно запускались внутри неё; оснований считать строки
примесью другой session нет. Подтверждена следующая последовательность:

- Console показывала работающие `links-repos` и `fix-terms`, пока Agents panel
  оставалась пустой;
- затем вкладка `Active` с badge `0` временно показала семь строк со статусом
  `DONE`;
- через несколько секунд эти строки исчезли из `Active`, badge `Completed`
  стал `10`, и завершённые агенты текущей session появились там.

Source-аудит подтвердил две самостоятельные UI/state ошибки и одну transport-
границу, требующую проверки после PR #21. Они вынесены в отдельный track
BR-25–BR-27; ни одна из задач не предполагает cross-session contamination.

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

**Status:** merged в PR #16; post-merge observation редкого incident остаётся
открытым. **Dependency:** none. **Acceptance:** automated merge gate + Windows
compile/privacy/runtime gate; воспроизведение редкого crash через 10–15 часов
остаётся post-merge production observation и не блокирует merge.

Изменение:

- bounded rotation долговечного sanitized `incident.log`; raw `host.log`
  ограничен текущим запуском и не переносится в backup generations;
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

**Status:** merged в PR #14. **Dependency:** none. **Acceptance:** automated +
Windows runtime merge gate.

Нужно устранить расхождение host/session state и Chat tab state:

- не терять connection event, пришедший до tab snapshot;
- не затирать более новое состояние поздним `listTabs`/`tab:list-changed`;
- трактовать существующий `session:error` как terminal lifecycle/protocol
  failure: server отправляет причину, закрывает WS, а client проходит штатный
  reconnect/re-auth без ложного `connected` timer. Будущий non-fatal сигнал
  должен иметь отдельный message type, например `session:notice`;
- при webview mount/reload выдавать один versioned authoritative snapshot.

Tests: event-before-tab, stale snapshot, terminal error + reconnect, cold app
restart при живой server session и несколько tabs. Старый пятисекундный error
timer отсутствует; stale manager generation и `listTabs` не откатывают более
новую authority/revision, а fatal error не оставляет false-green composer.

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

**Status:** merged в PR #15; automatic selection остаётся отдельным BR-15.
**Dependency:** none. **Acceptance:** automated + authenticated Windows runtime
merge gate.

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

**Status:** merged в PR #13. **Dependency:** none. **Acceptance:** automated +
Windows UI runtime merge gate.

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

**Status:** waiting/ready after classification. **Dependency:** BR-22 должен
сначала классифицировать пустое render window; после этого BR-16 выполняется
отдельно. **Acceptance:** automated + Windows CEF runtime merge gate.

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

### BR-17 — Persist privacy-safe Bridge server logs

**Status:** ready как отдельный operational PR. **Dependency:** none.
**Acceptance:** automated filesystem tests + isolated Linux Docker/Podman
runtime gate; Windows UI acceptance не требуется.

Server logger пишет относительно `process.cwd()` в `logs/`; в production image
с `WORKDIR /app` это `/app/logs`. Текущий compose не монтирует этот путь, поэтому
логи остаются в writable layer контейнера и исчезают при его recreation.

Change PR должен:

- добавить отдельный persistent volume для `/app/logs` и проверить права записи
  непривилегированного runtime user;
- задать документированные rotation и retention bounds, чтобы volume не рос
  бесконечно;
- сохранять безопасные lifecycle/correlation metadata, достаточные для
  различения explicit end, detach grace, idle/max-lifetime reap, duplicate
  resume и PTY exit; существующий default `info` не должен требовать постоянного
  включения всего raw debug output;
- не писать Bearer tokens, prompt bodies, tool inputs, file contents и другие
  произвольные payloads. Нельзя просто сделать persistent весь raw stdout/stderr;
- добавить runbook для просмотра, копирования, backup и удаления логов без
  остановки активных PTY, где это возможно.

Runtime gate запускается только в disposable CI/local/staging compose project с
отдельным именем, volume и непродуктивным port. Он создаёт диагностическую
запись, пересоздаёт только свой test container и доказывает, что запись
сохранилась; затем проверяет rotation/retention, отсутствие секретов в
allowlisted lifecycle record и восстановление записи после повторного старта.
Агенту запрещено выполнять `stop`, `restart`, `down`, `rm`, `volume rm`, recreate
или deploy для существующего production contour. Production rollout и
последующая read-only проверка выполняются отдельно maintainer'ом только после
merge и явного решения о выкладке. Mount legacy `bridge-sync` и его migration
эта задача не меняет.

#### Evidence required before implementation

Для исходного инцидента с `SessionEnd ... Unknown session`, `Session exited with
code 129` и последующим reconnect не хватает server-side причины удаления
сессии. Client JSONL и KaminIDE logs подтверждают наблюдаемый результат, но не
различают reaper, истёкший detach grace, explicit/admin end, duplicate resume и
server/container restart.

Для текущего инцидента maintainer может сохранить с Linux host следующие
артефакты до следующей плановой выкладки. Этот read-only сбор не требует и не
разрешает restart/recreate контейнера. Команды не печатают настройки или Bearer
token:

```bash
podman inspect claude-bridge \
  --format '{{.Id}} {{.Image}} {{.Config.Image}} {{.State.StartedAt}} {{.RestartCount}}'
podman exec claude-bridge printenv CLAUDE_PROXY_LOG_LEVEL
podman exec claude-bridge sh -lc \
  'TZ=UTC find /app/logs -maxdepth 2 -type f -printf "%TY-%Tm-%TdT%TH:%TM:%TSZ %s %p\n" | sort'
podman cp claude-bridge:/app/logs ./bridge-server-logs
podman logs --since '2026-08-21T09:10:00Z' \
  --until '2026-08-21T09:40:00Z' claude-bridge \
  > bridge-container-2026-08-21T0910Z.log 2>&1
```

Для другого инцидента UTC window заменяется на 10–15 минут до и после его
точного времени. Logger использует ISO UTC timestamps. Если runtime использует
Docker, те же команды выполняются с `docker` вместо `podman`.

Перед передачей проверяются как минимум:

- `/app/logs/sessions/*.log` и `/app/logs/errors/YYYY-MM-DD.log`;
- container stdout/stderr за тот же UTC window;
- точные local time + timezone, Claude conversation ID и отдельный Bridge
  runtime session ID;
- container ID, image digest/tag, server version, start time/restart count и
  фактический `CLAUDE_PROXY_LOG_LEVEL` без вывода остальных environment values;
- был ли перед событием disconnect/reconnect, tab switch, explicit session end,
  dashboard kill, deploy или container restart.

В передаваемом архиве удаляются Bearer tokens, Authorization headers, prompt/tool
payloads и содержимое файлов. Сырые логи не вставляются целиком в PR или chat;
достаточны redacted excerpts с сохранёнными timestamp, event name, runtime
session ID, reason, age/idle durations и exit code/signal.

При текущем compose default `CLAUDE_PROXY_LOG_LEVEL=info`, тогда как
`Reaping session`, `Detach grace expired`, `Destroying session` и `PTY exited`
пишутся через `debugLog`. Поэтому отсутствие этих строк в старых logs не
доказывает отсутствие события и ретроспективно восстановить причину может быть
невозможно. BR-17 должен сделать перечисленные безопасные lifecycle fields
доступными на default level; постоянно включать и сохранять весь raw debug log
не является решением.

Сам mount начнёт использоваться production service только при следующем
обычном deployment этого service, потому что volume declaration применяется при
создании контейнера. Это ожидаемый rollout effect, а не действие implementation
или review agent; отдельно «грохать все контейнеры» задача не требует.

### BR-18 — Keep SessionEnd relay available and secret-safe during teardown

**Status:** fix merged (PR #20, обновлён от `origin/main` с #23); причина
запуска teardown остаётся investigation и использует evidence из BR-17.
**Dependency:** BR-17 для классификации исходного termination trigger.
**Acceptance:** automated + authenticated Windows runtime merge gate — оба
выполнены на exact head PR #20 2026-09-03 (dev shell от `origin/main`
`3e9854c`, server image из head PR, Claude Code 2.1.236): explicit end и
disconnect с истёкшим detach grace; relay `SessionEnd` обслужен без `Unknown
session`, каждый hook вызван не более одного раза, MCP-запросы в окне teardown
получают 409, после finalize — 404; в session settings, Console, JSONL и logs
литерального credential нет.

Остаточное наблюдение gate (не часть BR-18, для следующей классификации):
при explicit end клиент закрывает WS сразу после `session:end`, поэтому
`SessionEnd` hook с `host: local` доставляется relay-ем, но исполняться на
клиенте уже не может и отменяется на PTY exit; server-side hook исполняется.

Source audit подтверждает самостоятельный teardown defect:

- `destroySession()` вызывает `cancelSessionLocalExecs()` и
  `clearHookSession()` до `pty.kill()`;
- Claude CLI запускает `SessionEnd` уже во время выхода, но relay lookup к этому
  моменту не находит session и отвечает `Unknown session`;
- default `node-pty` termination через `SIGHUP` отображается как exit code 129.
  Это доказывает forced teardown, но само по себе не определяет его caller;
- текущая user-visible hook command содержит relay `Authorization` value. Такой
  credential нельзя показывать в Console/JSONL или хранить в generated hook
  declaration.

Change PR должен оставить только ограниченное teardown окно: exiting session
больше не принимает обычные MCP/local exec requests, но её ранее
аутентифицированный `SessionEnd` relay остаётся доступен до первого успешного
вызова, PTY exit или короткого timeout. Cleanup idempotent, token ownership не
ослабляется, повторный вызов и reused session ID не получают старую
регистрацию. Relay credential передаётся через secret-safe runtime channel, а не
интерполируется в отображаемую command/settings; tests доказывают отсутствие
Bearer/token в declaration, logs и JSONL.

Automated tests покрывают explicit end, detach-grace teardown, no-hook exit,
timeout, duplicate callback, чужой token и cleanup после PTY exit. Windows gate
запускает новый chat с approved `SessionEnd` hook, выполняет штатное завершение
и disconnect с истечением grace: hook вызывается не более одного раза, Console
не показывает `Unknown session` или credential, а новые requests после начала
teardown отклоняются. Почему исходная длинная session вошла в teardown,
определяется отдельно по BR-17 logs и не угадывается этим fix.

### BR-19 — Treat shell disconnect as lifecycle cancellation, not extension crash

**Status:** ready. **Dependency:** implementation starts from the refreshed main
after the current draft PR chain; no dependency on BR-17 evidence.
**Acceptance:** automated + Windows runtime merge gate.

Source chain reproduces the screenshot text exactly:

- `ws-server.ts` handles shell WebSocket close by calling
  `endpoint.failAll("shell client disconnected")`. Rejecting all pending calls
  is necessary: otherwise `showQuickPick`, editor operations and secret relay
  can wait forever and leak pending RPC entries;
- the rejection crosses `HOST_REQUEST_RENDERER` back into the extension-host
  child as an ordinary `Error` without a lifecycle/cancellation type;
- internal fire-and-forget surfaces currently discard Promises without rejection
  handlers. Confirmed examples include editor decorations/selections and the
  stateful `createInputBox().show()` / `createQuickPick().show()` paths;
- `child-crash.ts` treats every post-boot `unhandledRejection` as an extension
  crash and emits `Contained — extensions stayed alive: ...`. Therefore the
  displayed toast is a false crash classification, while the final clause
  correctly reports that the child process was deliberately kept alive.

The fix must preserve `failAll` liveness while introducing a typed, generation-
scoped peer-disconnect/cancellation result. Every internal fire-and-forget RPC
boundary handles only that expected lifecycle cancellation; awaited VS Code-like
APIs settle deterministically as cancelled/hidden according to their contract.
Do not globally swallow `unhandledRejection` and do not match the human-readable
error string: genuine extension errors and unexpected RPC failures must still
reach crash containment.

Automated tests disconnect the shell with pending editor decoration/selection,
quick input and representative awaited calls; pending maps clear, no expected
disconnect becomes an unhandled rejection or crash notification, and a genuine
unexpected rejection still does. A rapid disconnect/reconnect test proves that
an old client generation cannot cancel or settle a call owned by the new one.
The Windows gate disconnects/reconnects the shell while extensions and Bridge
sessions remain active: no false `Extension crashed` toast appears, interactive
operations either cancel or recover, and a deliberately thrown extension error
is still surfaced.

### BR-20 — Restore complete Claude plan usage windows

**Status:** ready. **Dependency:** none. **Acceptance:** automated + isolated
authenticated Linux container/browser runtime gate; production rollout is not
part of the implementation PR.

The Account card does not call a documented Anthropic quota API and does not
derive plan utilization from local tokens. `usage-capture.ts` starts a separate
interactive `claude --dangerously-skip-permissions /usage`, concatenates PTY
redraw output and parses human TUI text with regular expressions written for an
older `Sonnet only` layout. On the exact observed Claude Code 2.1.236 output:

- `Current session` parses successfully;
- the promo line between the all-model reset and the next section prevents the
  all-model weekly expression from matching;
- `Current week (Fable)` cannot populate the hard-coded `weekSonnet` field;
- partial success suppresses `_raw` diagnostics and the UI silently omits both
  missing rows.

The screenshots at 23% and 26% were captured about one hour apart with the same
reset time, so that delta is not evidence of a calculation defect. A future
comparison must capture native `/usage` and dashboard values within ten seconds
after refresh for the same account and reset window.

The fix must establish a capability-based plan-usage contract:

- represent limits as typed dynamic windows, not fixed `weekSonnet` fields;
- always support the five-hour and seven-day all-model windows when Claude
  reports them, and show an optional model-specific Fable window when the
  active CLI exposes it;
- record `observedAt`, source, Claude Code version, reset timestamp, freshness
  or stale state and partial/unavailable reason for every capture;
- prefer the documented Claude Code statusline `rate_limits.five_hour` and
  `rate_limits.seven_day` structured contract for the common windows. It does
  not expose a documented Fable field, so model-specific usage remains an
  optional capability and must fail closed instead of being guessed;
- do not integrate an undocumented private OAuth endpoint as a stable API and
  do not calculate subscription percentages from JSONL token totals;
- if a TUI compatibility reader remains for optional windows, derive the final
  terminal state rather than matching the first occurrence in concatenated
  redraw frames, tolerate promo text, missing/reordered/new sections and
  arbitrary model labels, and retain bounded diagnostics on every partial parse;
- never replace a complete last-known snapshot with an unlabelled partial
  result. The UI shows unavailable/stale state rather than silently deleting a
  previously supported row.

Unit fixtures cover current promo + Fable output, the previous Sonnet layout,
no-promo and extra-section variants, ANSI redraw with changing percentages,
partial/error output, cache/force-refresh races and unknown future model labels.
The isolated runtime gate records native `/usage`, the dashboard JSON and the
Account card within ten seconds and compares percentages plus reset timestamps;
it also proves graceful rendering when an optional window is absent. Secrets
and raw OAuth credentials are never stored in evidence.

### BR-21 — Define and reconcile dashboard analytics semantics

**Status:** investigation umbrella; the concrete aggregation defects below are
ready to split into bounded change PRs after the metric contract is approved.
**Dependency:** none. **Acceptance:** deterministic DuckDB/JSONL fixtures for
every child PR + isolated dashboard runtime gate; production data is read-only
validation only.

The lower Usage chart and Stats cards are local analytics built from
`~/.claude/projects/**/*.jsonl` through `jsonl-sweeper.ts` and DuckDB. They are
not the source of the Account quota percentages. Source audit found several
independent correctness defects:

1. Subagent JSONL rows store `parent_session_id`, but subagents deliberately do
   not receive their own `session_tokens` row. Every dashboard query joins only
   `e.session_id = st.session_id`; `parent_session_id` is unused. Agent Teams
   messages, model usage and tokens are therefore excluded completely, although
   they should be attributed to the parent without increasing top-level Session
   cardinality.
2. Claude Code emits several assistant JSONL rows with the same `message_id`
   for one turn. `getUserTimeSeries()` deduplicates them, while overview model
   totals, assistant-message count, hourly/heatmap tokens, user cost and other
   totals sum raw rows. Cards and graph can therefore disagree and the affected
   values are multiplied by content-block count.
3. `Session tokens` is a sum of last-context snapshots, but its query ignores
   the selected `7d` or `30d` cutoff while the neighbouring counters and model
   rows apply it.
4. The top Sessions card reads `s.userMessages`, but live
   `session:updated` events patch only `inputCount`/MCP fields. A newly created
   session has no `userMessages` field in the client row, so `User msg` remains
   at its initial snapshot until a full dashboard reconnect.
5. Stats requests start the sweeper without awaiting it, cache the previous DB
   snapshot for 30 seconds, and the open Usage chart/Stats cards do not poll or
   subscribe to data revisions. A completed catch-up sweep does not refresh the
   page that requested it.
6. The server implements compact `agg=hm&tz=...` overview payloads and exact
   local-day `dailySessions`, but the web client never requests or consumes
   them. It continues to fetch the legacy per-session hourly payload that the
   compact mode was introduced to replace.
7. So-called per-token cards use mutable, non-unique `user_name` as identity.
   Token rename splits history and duplicate display names merge independent
   tokens. Token UUID must be the key and name must remain presentation data.
8. Day/month chart buckets are aggregated in UTC before the browser labels
   them as local; events around local midnight cannot be reassigned correctly
   after aggregation. Streak/grid date arithmetic also assumes every local day
   is exactly 86,400,000 ms and breaks across DST.
9. Model grouping uses exact raw IDs while the UI normalizes only the displayed
   label. Multiple IDs may render with the same label but remain separate in
   shares and `Favorite model`.
10. Cost estimation recognizes only hard-coded Haiku/Opus/Sonnet families and
    silently prices an unknown or Fable model as Sonnet. The dashboard must use
    an explicit versioned price catalog and mark an unknown price unavailable;
    an invented fallback is not valid analytics.
11. The legacy `/api/dashboard/stats` per-user map copies global model totals
    into every user and fills placeholder request/start/error values. Even if
    the current page no longer consumes it, it remains a public-looking route
    that can produce false data and must be removed, isolated as legacy or
    brought under the same canonical metric contract.

Current field meanings must be preserved or deliberately renamed while the
contract is written:

| Surface | Current meaning |
| --- | --- |
| Active Sessions / Users | resident server PTY sessions and distinct names among them; detached sessions remain resident during grace |
| Top `MCP Calls` | initiated relay attempts in resident PTYs, including failed/timeout calls; resets with the PTY |
| Usage chart | deduplicated `input + cache_creation + output`; cache reads excluded |
| `Session tokens` | sum of the last effective input/context snapshot per session, not cumulative throughput |
| Model `in` / share | raw `input + cache_read + cache_creation`, then `(in + out)` share |
| Heatmap / Peak hour | intensity and peak based on human user-message rows |
| Favorite model | raw model ID with greatest effective input + output volume |

Before implementation the product contract must decide rolling versus local
calendar ranges, whether model preference is based on calls/new tokens/context
or cost, how subagent internal prompts are labelled, and whether the `All`
heatmap intentionally shows only 26 weeks while its counters cover all history.
The implementation is then split at least into:

1. one reusable assistant-turn relation deduplicated by
   `(session_id, message_id)` with a documented UUID fallback, plus parent-based
   subagent attribution and token-UUID identity;
2. consistent range/model/timezone filters and compact payload consumption;
3. versioned freshness/invalidation and live top-card counters;
4. versioned model aliases and price provenance with explicit unknown handling;
5. labels/tooltips that distinguish context snapshot, new-token flow,
   cumulative API throughput, attempts and authoritative plan quota.

Fixtures include duplicate streaming rows, top-level + multiple subagents,
compact boundaries, token rename and duplicate display names, `all/30d/7d`,
UTC−/UTC+ midnight, DST, model aliases and partial JSONL append. Required
invariants include no duplicate turn usage, subagent usage attributed exactly
once without adding a top-level Session, range consistency across neighbouring
metrics, `All` equal to the sum of token-UUID slices, and a visible data revision
after catch-up ingestion. A cost fixture covers known, Fable and unknown model
IDs and forbids silent family fallback. Production audit records
raw-versus-distinct assistant counts and subagent rows before/after the current
join without mutating data.

### BR-22 — Keep live chat render window populated by drawable rows

**Status:** investigation. **Dependency:** diagnostic capture из текущего UI;
BR-01 желателен для корреляции с runtime events. **Acceptance будущего fix:**
automated differential/render tests + Windows CEF runtime gate.

Наблюдаемый 24 августа screenshot локализует отказ уже после загрузки данных:

- Console и activity state продолжают обновляться, поэтому agent/PTY не
  остановились и весь application renderer не перестал рисовать;
- `58 earlier messages` создаётся только при
  `visibleMerged.length - renderCap === 58`, где начальный `renderCap` равен
  150;
- между marker и live activity spinner нет ни одной message/tool card. Значит,
  viewer считает хвост окна видимым, но `JsonlEntry` возвращает `null` для всего
  смонтированного хвоста либо эквивалентно теряет уже подготовленные vnode.

Source уже содержит защиту от этого класса ошибок: неизвестные entry types,
bookkeeping rows, невидимые attachments и несколько system subtypes должны быть
отфильтрованы до windowing. Но контракт остаётся раздвоенным: `entryIsVisible`
решает, что занимает слот, а `JsonlEntry` отдельно решает, что реально рисуется.
Кроме того, `recentTip` допускает записи от последнего assistant до конца в
обход `entryIsVisible`, проверяя только `NON_RENDERING_ENTRY_TYPES`. Поэтому
точная причина текущего случая пока не доказана: это может быть новый
entry/subtype/attachment shape, один из `recentTip` bypass paths либо stale
derived/vnode cache. Исправлять произвольно выбранный вариант без dump нельзя.

До implementation нужен diagnostic, сохранённый кнопкой со stethoscope прямо
во время пустого состояния, и второй dump после самовосстановления той же
session. Уже существующий `DiagnosticButton` записывает store/drop summary,
`mergedCount`, `visibleMergedCount`, `visibleByType` и последние 40 visible rows;
в evidence также фиксируются точное local time + timezone, tab/session id,
было ли active streaming/tool burst/Agent Teams и происходили ли tab switch,
reconnect или compaction. Payload previews перед передачей redacted; tokens,
prompts и tool output не публикуются.

Будущий fix обязан сделать один predicate/source of truth для «занимает слот и
рисуется», а не пополнять очередной несвязанный deny-list. Tests строят окно из
более чем 150 строк каждого non-rendering/unknown/system/attachment/sidechain
shape, длинный live tool burst, stub→canonical hand-off и cache hit/miss. Для
непустой loaded session invariant: mounted tail содержит хотя бы последнюю
drawable conversation row; поток структурных событий не может циклически
переключать её на пустое окно. Windows gate держит новую session с активным
streaming и сериями user-tools минимум 10 минут: Console и Chat продолжают
обновляться, центральная лента не мигает и не исчезает.

До event trace BR-22 не объединяется с BR-06: BR-06 связан с update после tab
switch/reconnect и pointer activity, а текущий incident возникал периодически в
уже активной новой session. BR-22 нужно классифицировать до BR-16, потому что оба
будущих change PR затрагивают `JsonlViewer` render/window contract и иначе
создадут лишний конфликт или скроют регрессию друг друга.

### BR-23 — Make session-complete notifications transient and turn-scoped

**Status:** confirmed source defect. **Dependency:** implementation обновляется
от финального merged connection-state PR перед изменением
`handle-server-message.ts` или committed Bridge artifacts. **Acceptance
будущего fix:** automated lifecycle/protocol tests + Windows native-toast gate.

Screenshot 31 августа подтверждает, что `Session finished — Tab … is ready`
остаётся на экране без countdown. Это не modal и не случайная остановка timer:
Bridge вызывает `vscode.window.showInformationMessage(text, "Open")`, а native
shell классифицирует любой `shell.showMessage` с хотя бы одним action как
`sticky: true`. Поэтому для такого toast намеренно не создаётся 8-секундный
timer и countdown bar. Для сравнения, `Anthropic busy` идёт без action и shell
автоматически закрывает его через свои 8 секунд; заявленные webview `duration:
6000` при маршрутизации через shared notification API сейчас теряются.

Наблюдение «при Agent Teams уведомление может повторяться по мере завершения
агентов» не объясняется прямой обработкой `SubagentStop`: bridge status hook
явно считает его informational и не отправляет `session:activity`. Но найден
отдельный источник повторов внутри одного main turn. Server публикует и
авторитетные hook-driven состояния (`UserPromptSubmit`/`Stop`), и эвристические
OSC-title состояния; `handle-server-message.ts` передаёт в `SessionIdleTracker`
оба вида без `hookDriven`. Tracker не знает turn identity и после каждого
debounced `working -> idle` снова разрешает toast, если позже увидел новый
`working`. Поэтому OSC idle/resume blips во время orchestration способны
породить несколько `Session finished` до единственного main `Stop`. Точное
равенство количества toast числу subagents кодом не гарантировано, но повторное
срабатывание в одном turn разрешено и противоречит уже заявленному
hook-authoritative activity contract.

Fix не должен делать все notifications с actions transient: elicitation и
approval ожидают решения пользователя и обязаны оставаться sticky. Нужен явный
contract именно для completion toast: `Open` остаётся рабочим, toast сам
закрывается, а ожидающий `shell.showMessage` request при timeout завершается
`undefined/null` и не течёт. Idle notification создаётся не более одного раза
на завершение main turn; `SubagentStop` его не создаёт; после появления
hook-driven `UserPromptSubmit` эвристический OSC idle не завершает turn, а
hook-driven `Stop` завершает. Fallback для server без lifecycle hooks описывается
и тестируется отдельно. Существующие suppression для displayed active tab,
reconnect settle и закрытого tab сохраняются.

Automated tests покрывают: main turn с несколькими `SubagentStart/Stop` и OSC
idle/resume blips даёт один completion; два последовательных main turns дают по
одному; transient action toast отвечает host request на click, dismiss и
timeout; question/elicitation остаётся sticky; duration не теряется между
webview, extension host и shell. Windows gate проверяет countdown, автозакрытие,
`Open`, hover pause и отсутствие серии toast в живой Agent Teams session.

### BR-24 — Bound and reconcile lost webview invoke replies

**Status:** confirmed incident; root-cause investigation. **Dependency:** BR-01
diagnostics желательны; implementation только после текущей последовательности
PR #12–#16. **Acceptance будущего fix:** automated transport/lifecycle tests +
Windows CEF runtime gate.

Windows acceptance PR #13 воспроизвёл 3 раза из 5: mutating call
`hooks:set-plugin-approval` завершился host-side, approval store был записан и
sync залогирован, но соответствующий `invoke-reply` не дошёл до webview. Promise
остался в `pending` без deadline, а full-screen approval modal завис на
`Saving…`. PR #13 добавил только feature-local 15-секундный bound, возвращение
управления dialog и reconciliation через повторное чтение pending approvals.
Это сохраняет approval UI рабочим, но не исправляет общий transport: любой
другой `inv()` всё ещё способен ждать бесконечно.

Причина потери frame пока не доказана. Текущий код не различает `postMessage`
failure, hidden/disposed webview, renderer reload и reply, пришедший после
смены document generation. Поэтому задача не объявляет простое добавление
глобального timeout полным исправлением. Сначала нужны privacy-safe counters и
correlation по invoke id/channel, document generation и результату
`source.postMessage`, без args/result payload. Диагностика должна отличать
«handler не завершился», «reply send rejected/returned false», «renderer был
заменён» и «reply просрочен/неизвестен».

Transport contract обязан ограничивать каждый pending invoke и очищать его при
webview teardown/reload. Read-only idempotent операции могут повторяться только
по явной policy. Mutating operation после timeout нельзя слепо повторять:
запись могла состояться, как в #13, поэтому caller получает indeterminate
outcome и выполняет domain-specific read-back/reconciliation. Late/duplicate
reply не должен резолвить новый request с переиспользованным id или оставлять
утечку. Отдельно определяется UX для обычных panels и blocking dialogs.

Tests покрывают normal reply, handler reject, dropped/false `postMessage`,
renderer reload до reply, late и duplicate reply, pending cleanup и mutating
call с успешной записью при потерянном ответе. Windows gate повторяет
disposable approval scenario и несколько read-only invokes при tab switch,
hide/show, extension-host reconnect и CEF reload; ни один promise или modal не
остаётся бесконечно pending, а повторная mutation не выполняется автоматически.

### BR-25 — Verify Agents view delivery and rehydration after reveal

**Status:** baseline done (INC-2026-0002); completion gate waiting.
**Dependency:** BR-31, затем BR-27 и BR-26. **Acceptance:** instrumented
Windows Agent Teams runtime gate; новый fix PR создаётся только если симптом
сохраняется.

Agents panel — отдельная `tools.html` CEF webview со своим `useBridgeListeners`
и собственной копией agent state. В KaminIDE 1.0.54 shell не сообщает exthost
обычные hide/show transitions этой view: `kamin:webview:viewState` отправляется
при creation/reap, но не при каждом уходе панели с экрана. Поэтому
`WebviewView.visible` может остаться `true`, `BridgeHost` не фиксирует
пропущенный hidden-view stream как stale и reveal не обязан вызвать
`resyncActive()`. PR #21 добавляет недостающий visibility lifecycle, однако он
не меняет Agents state machine и сам по себе не заявляет этот инцидент
исправленным.

Скриншоты подтверждают потерю актуального представления, но без event trace не
доказывают, что visibility gap — единственная причина первоначально пустой
панели. После #21 нужно в одной session:

1. запустить не менее двух teammates при скрытой Agents panel;
2. открыть панель во время их работы и получить все `running` rows без движения
   мыши, повторного toggle и ожидания reap;
3. повторить hide/show, tab switch и открытие старой session с завершёнными и
   работающими агентами;
4. записать только bounded metadata: view id/visibility, resync generation,
   `jsonl-status` replay start/complete, число agent lifecycle rows по batch и
   итоговые counts без prompts, reports и tool payloads.

Если после #21 running rows всё ещё отсутствуют, отдельный implementation PR
локализует потерю между host cache, Agent view fan-out и parser generation. До
этого добавлять произвольный polling или повторный replay по timer нельзя.

**Baseline result (2026-09-03, `origin/main` 8a1e6f9, INC-2026-0002).** Gate
выполнен с двумя teammates при скрытой панели, hide/show, tab switch,
пересозданием после reap и открытием старой session после перезапуска
приложения; записаны только bounded metadata. Потеря локализована не в host
cache, fan-out или parser generation, а в shell delivery pump: кадры доходят
до per-view outbox, но страница забирает их только на тике насоса
перерисовки, который просыпается лишь по кадру CEF — см. BR-31. Visibility
lifecycle #21 работает. Отдельно подтверждено, что завершённые teammates
остаются `RUNNING` в live и после replay (BR-27). Completion gate повторяется
после BR-31, BR-27 и BR-26 по процедуре R6 из INC-2026-0002.

### BR-26 — Publish Agent replay state atomically

**Status:** fixed (Change/Fix PR `fix/agents-replay-staging`); Windows CEF gate 2026-09-03 (INC-2026-0002, evidence addendum): sampler DOM каждые 50 мс; при hide/show resync с двумя running teammates (два цикла `replayComplete false→true`) ни одного изменения DOM — rows не исчезали и не перестраивались; после reap+reveal новый renderer сразу показал полный snapshot (`Active 2/Completed 8`, затем `0/10`) без пустого или частичного состояния; завершения пришли двумя согласованными update. Насос доставки будился принудительной перерисовкой чата (BR-31 workaround).
**Dependency:** BR-25 baseline и BR-27 (done); повторная проверка BR-25
ждёт BR-31. **Acceptance:** automated replay-generation tests + Windows CEF
runtime gate.

Каждый `replayJsonlToRenderer()` сначала отправляет
`jsonl-status { replayComplete:false }`. Agents listener немедленно удаляет
`tabAgentTrees[tabId]`, после чего до 4 500 cached rows приезжают yielding
chunks по 150. `AgentsToolPanel` не проверяет `tabJsonlLive` и рендерит дерево
после каждого частичного batch. В результате уже отображавшийся список может
стать пустым, затем показать только раннюю часть replay и ещё раз перестроиться
после `replayComplete`. Комментарий в `buildAgentTreeNodes()` о том, что panel
не показывает промежуточный replay, относится только к sidebar tree и не
выполняется самой `AgentsToolPanel`.

Fix должен иметь generation-scoped staging state: replay собирается отдельно,
а опубликованный snapshot меняется атомарно только после соответствующего
`replayComplete`. Во время resync panel сохраняет последний согласованный
snapshot либо показывает один явный loading state, но не чередует empty/partial
lists. Более старый или прерванный replay не может опубликоваться поверх нового;
live entries, пришедшие на границе, не теряются и не удваиваются.

Tests покрывают cold hydration, resync при двух running agents, 10 завершённых
agents, несколько chunks, live entry между последним chunk и completion, два
перекрывающихся replay generation и empty genuine session. Windows gate
повторяет последовательность четырёх скриншотов: panel не мигает, running rows
не исчезают, а завершённая history появляется одним согласованным update.

**Fix.** `signals/agent-replay.ts` держит replay per tab как generation-scoped
staging: `jsonl-status { replayComplete:false }` открывает поколение и больше
не удаляет опубликованное дерево, все `jsonl-entries` до `replayComplete`
копятся в staging, а на completion snapshot строится один раз из записей,
упорядоченных `orderEntries()` (chunks приходят newest-first), и публикуется
одной заменой `tabAgentTrees[tab]`. Устаревшее поколение (новый replay начался
раньше) не публикуется; live entry, пришедшая на границе, входит в тот же
snapshot ровно один раз; закрытие таба сбрасывает staging. Panel во время
resync показывает последний согласованный snapshot.

### BR-27 — Derive Active and Completed from one lifecycle partition

**Status:** fixed (Change/Fix PR `fix/agents-panel-lifecycle-partition`);
Windows Agent Teams gate 2026-09-03 (INC-2026-0002, evidence addendum): три teammates (sleep 20/75/45 с) завершались в порядке alpha3 → gamma3 → beta3; вкладки показывали `Active 3/Completed 5` → `2/6` → `1/7` → `0/8` во время работы, сразу после завершения, после cleanup 5 с и после reap+replay без дублей; badge всегда равнялся числу rows. Насос доставки будился принудительной перерисовкой чата (BR-31 workaround).
**Dependency:** BR-25 baseline (done); BR-26 строится поверх этого snapshot.
**Acceptance:** automated state/renderer tests + authenticated Windows Agent
Teams gate.

Текущий `AgentsToolPanel` использует разные правила для badge и содержимого:

- `activeCount` считает только `status === "running"`;
- вкладка `Active` рендерит всех members команды, пока сама команда не
  `disbanded`, и все standalone rows независимо от agent status;
- `completedCount` не считает terminal members активной команды;
- `scheduleCleanup()` лишь через 5 секунд переносит `done/error/terminated` из
  live tree в `tabAgentHistory`.

Это точно объясняет состояние `Active 0` с семью `DONE` rows и их последующий
переезд в `Completed`. Fix вводит одну derived partition над одним snapshot:
каждый agent текущей session находится ровно в одном из `Active` или
`Completed`; badge равен числу реально отрисованных rows; terminal agent
появляется в `Completed` сразу, а cleanup меняет только storage/retention и не
видимую классификацию. `done`, `error`, `terminated` и disbanded team сохраняют
различимые labels; повторный replay не дублирует rows.

Fixtures обязаны проходить через реальный wire projection. Сейчас server
`leanEntries()` удаляет весь `toolUseResult`, хотя agent parser читает из него
bounded lifecycle status и completion counters. Это отдельная подтверждённая
contract inconsistency, но не доказанная причина данных скриншотов. Исправление
не должно возвращать тяжёлый duplicate payload целиком: нужен либо узкий
lifecycle DTO для Agent/Task, либо parser, основанный только на полях, которые
действительно сохраняются на wire.

Runtime evidence (INC-2026-0002): в Windows gate завершённые teammates
остались `RUNNING` (`Active 2`, `Completed 0`) и в live, и после replay через
27 минут; wire entries содержали только `Agent` tool_use без terminal signal.
Fix обязан закрывать lifecycle по полям, реально присутствующим на wire.

**Fix.** Причина «RUNNING навсегда» была не в wire projection, а в parser:
`parseAgentEntries()` выходил из итерации (`continue`) для user-записей со
строковым `content` — именно так CLI пишет `<teammate-message>` с
`idle_notification`/`shutdown_approved` и `<task-notification>` background
agent'ов, поэтому terminal signal никогда не применялся. Теперь строковые
записи разбираются; synthetic-запись teammate, созданная уведомлением,
пришедшим при replay раньше своего `Agent` tool_use, сливается в основную
запись вместо перезаписи на `running`. `AgentsToolPanel` строит `Active` /
`Completed` из одной derived partition (`signals/agent-partition.ts`):
агент ровно в одной вкладке, badge равен числу отрисованных rows, terminal
agent попадает в `Completed` сразу, cleanup через 5 с меняет только
storage (tree → history) и не видимую классификацию. Fixtures тестов повторяют
wire-формы из INC-2026-0002 без `toolUseResult`.

Automated tests покрывают running→done/error/terminated, idle notification,
`teammate_spawned`, disband, cleanup before/after 5 seconds и repeated replay.
Invariant после каждого update: `Active badge === rendered active rows`,
`Completed badge === rendered completed rows`, пересечение множеств пусто.
Windows gate запускает несколько teammates, завершает их в разном порядке и
проверяет вкладки во время работы, сразу после завершения и после cleanup.

### BR-28 — Measure sidebar geometry during session hover

**Status:** closed — optical/expected indent (INC-2026-0003, paired artifact
2026-09-03). **Dependency:** none. **Acceptance:** отдельный bounded Windows
GPUI evidence artifact, не speculative functional PR — выполнено.

Screenshot 2 сентября показывает session actions pill у `Front-Back для
проект...` и визуально отличающийся промежуток перед `35 inactive sessions`.
Source не подтверждает, что fly-out добавляет место в sessions list:

- `overlay_pill()` рендерится в отдельном overlay window как `absolute`;
- `anchor_probe()` также `absolute` и не должен участвовать в layout строки;
- session row имеет фиксированную высоту 24 px, а контейнер sessions — постоянный
  gap 2 px;
- inactive toggle всегда имеет собственный left padding 18 px, chevron 12 px и
  gap 6 px, поэтому его label по дизайну начинается правее session label;
- единственное hover-driven изменение внутри layout — unpinned `pin_btn()`,
  который раскрывается с width 0 до 20 px и перераспределяет горизонтальный
  бюджет только hovered row.

По одному screenshot нельзя отличить реальный vertical reflow от постоянного
indent, изменения truncation/time/pin allocation или GPUI flex regression.
Исследование снимает paired bounds без движения/scroll между кадрами: session
row, label, time, pin, group sessions container, inactive toggle, scroll
viewport, `anchor_probe` и overlay pill — до hover, на hover и после leave.
Evidence фиксирует logical/physical px, DPI, sidebar width, scrollbar presence,
open/pinned state и положение строки у нижней границы viewport. Матрица включает
pinned/unpinned, open/inactive, с/без scrollbar и несколько project groups.

Если sibling bounds не меняются, задача закрывается как optical/expected indent
с документированным сравнением; произвольная правка padding запрещена. Если
меняются — отдельный change PR локализует первый ancestor с изменившимся
height/y и добавляет regression probe. Этот incident не объединяется с BR-29
без такого evidence: общий hover subsystem ещё не доказывает общую layout cause.

**Result (2026-09-03, INC-2026-0003).** Paired main-window frames до hover /
на hover / после leave (hover через probe `WM_MOUSEMOVE`, без pointer и
scroll; overlay-окно с pill в кадр не входит) для семи случаев: open
unpinned, pinned, active у нижней границы viewport, plain open, inactive row
в раскрытой группе и сам inactive toggle; scrollbar присутствует, 15 project
groups, scale 1.25. Во всех случаях между before и hover менялась только
полоса самой hovered row (раскрытие pin 0→20 px, highlight), ни одна полоса
выше или ниже не сдвинулась, after-кадр равен before. Sibling bounds не
меняются — задача закрыта как optical/expected indent; padding не правится,
BR-29 идёт без sibling-layout fix.

### BR-29 — Make hover-to-rename transition atomic

**Status:** fixed (Change/Fix PR `fix/sidebar-rename-transition`); Windows GPUI gate 2026-09-03 (INC-2026-0003, evidence addendum): hover строки → overlay `hover_pill`; BeginRename через probe (путь fly-out/контекст-меню/F2) при ещё hovered строке → overlay states пусты, инпут `session-rename-input` на месте; повторный enter на переименовываемую строку пилюлю не возвращает; hover соседней строки во время rename показывает её пилюлю и убирает по leave; double-click тоже открывает rename без пилюли; paired bounds по процедуре BR-28 на этой сборке: меняется только полоса hovered row. Фокус инпута probe не измеряет (не менялся).
**Dependency:** paired baseline из BR-28 (INC-2026-0003, done).
**Acceptance:** automated state/geometry tests + Windows GPUI sidebar gate.

Переход в inline rename сейчас не владеет teardown hover actions:

- `ShellEvent::BeginRename` закрывает только `session_menu`, ставит
  `renaming_session` и создаёт input, но не очищает `hover_pill`,
  `hover_pill_anchor`, `hover_pill_panel` или generation;
- rename branch в `session_row()` возвращается до установки `on_hover` и
  `anchor_probe`, поэтому заменённый hovered node может не прислать leave;
- process-global `pill_anchor()` хранит последнюю геометрию как unscoped
  `Option<[f32; 4]>` и сам не очищается при исчезновении anchor;
- passive overlay продолжает рисовать actions, пока одновременно сохранены
  `hover_pill` и старая anchor geometry. Общий mouse-down dismiss уменьшает
  частоту проявления, но не покрывает F2, double-click, замену node и ordering
  capture/target events как явный контракт rename.

Fix вводит один state transition helper: перед `BeginRename` он инвалидирует
pending close generation, очищает оба hover sources и связанную geometry, затем
показывает input. Anchor geometry становится id/generation-scoped либо явно
сбрасывается вместе с state, чтобы координаты одной строки нельзя было
использовать для другой. Rename не зависит от случайного `mouseleave` или
глобального mouse-down listener.

Tests покрывают rename из fly-out button, double-click и F2 после anchor hover и
panel hover, delayed leave предыдущего node, cancel/commit/blur, смену session и
scroll во время transition. Invariant: при `renaming_session = id` для `id` нет
actions overlay и stale anchor; input не перекрыт, не обрезан pill hitbox и
сразу имеет focus. Windows gate повторяет оба screenshot-сценария и отдельно
сверяет bounds из BR-28 до/после fix.

**Fix.** `state/rename_transition.rs` — один helper `RootView::begin_rename()`:
сначала `dismiss_hover_pill()` (оба hover source, generation+1, сброс
geometry якоря), затем `renaming_session`/инпут; все входы (fly-out кнопка,
double-click, F2, контекст-меню, probe) идут через него. Hover-enter для
переименовываемой строки игнорируется до commit/cancel. Якорь пилюли
(`ui/sessions/anchor.rs`) стал id-scoped: `PillAnchor { id, bounds }`,
overlay рисует пилюлю только по `anchor_for(hover_pill)` и никогда для строки
в rename. `DismissHoverPill` использует тот же teardown.

### BR-30 — Keep incident-log records atomic across rotation

**Status:** fixed в change PR пачки `RB-2026-09-A` (record-aware
`RollingLogWriter.writeRecord`, единый emitter path в `incident-log.ts`);
BR-02 может опираться на машинно-разбираемый `incident.log` после merge.
**Dependency:** none. **Acceptance:** automated filesystem/rotation tests —
rotation boundary внутри ASCII и multibyte records, поколения читаются по
отдельности, oversized record заменяется bounded marker `record-dropped`;
Windows smoke проверяет только запись и чтение логов и не требует доступа к
корпоративному marketplace.

`RollingLogWriter.write()` строго заполняет оставшиеся байты текущего файла и
только затем выполняет rotation. Для raw `host.log` это ожидаемый bounded
streaming contract, но все writers `incident.log` передают целую строку
`[incident] <json>\n`. Если свободного места меньше длины такой строки, начало
record остаётся в `.1`, а окончание попадает в новый `incident.log`; ни один
fragment больше не является валидной JSONL-записью. Дефект воспроизводится на
текущем `main` с `maxBytes = 32`: rotation boundary проходит внутри одного JSON
record. Существующие tests проверяют byte cap на произвольных chunks, но не
атомарность structured records и не UTF-8 boundary.

Fix сохраняет текущий byte-bounded `write()` для raw stream и добавляет
record-aware path для incident events. Если bounded record помещается в пустое
поколение, но не помещается в остаток текущего, writer сначала выполняет
rotation, а затем записывает record целиком. Поведение для record больше
`maxBytes` задаётся явно и не ослабляет retention cap; все incident emitters
используют один новый path.

Tests ставят rotation boundary внутри ASCII и multibyte records, читают каждое
сохранённое поколение отдельно и подтверждают, что каждая непустая строка имеет
полный `[incident] ` prefix, валидный JSON и корректный UTF-8. Отдельно остаются
зелёными текущие raw-chunk, retention и restart tests.

BR-23–BR-30 зафиксированы docs-only и не меняют runtime или generated
artifacts. Agents track не содержит предположения о данных из другой session.

## Current open PR boundary

PR #12–#24 и PR #26 находятся в `origin/main`; PR #20 (BR-18) обновлён от
`origin/main` с #23, прошёл automated и Windows runtime gate и объединён. PR #25
(сырые diagnostic logs с рабочей машины) закрыт без merge; его допустимые
материалы перенесены в private incident `INC-2026-0001`. PR #27 с новым flow
также находится в `origin/main`. На момент формирования
`RUNTIME_EXECUTION.md` открытых PR нет.

Особая зависимость Agents track — PR #21: сначала его полный automated и
Windows lifecycle gate, затем BR-25 verification. PR #21 не считается
автоматическим доказательством исправления Agents panel. BR-27 после этого идёт
перед BR-26; каждый implementation PR начинается от нового `origin/main` и
пересобирает generated artifacts из объединённых sources, без `ours`/`theirs`.

## Scheduling constraints

Текущая пачка и состояние каждой задачи определены в `RUNTIME_EXECUTION.md`.
Ниже остаются только отношения между tracks; отдельный нумерованный scheduler в
этом файле больше не ведётся, чтобы два списка не расходились.

- Agent Teams UI: #21 (merged) → BR-25 verification → BR-27 → BR-26; BR-15 только
  после стабильности delivery/view lifecycle;
- long session: BR-01 (merged) → BR-30 → BR-02 → BR-03;
- history UI: BR-22 classification → BR-16, а BR-06 создаётся только при
  повторном воспроизведении после текущей очереди;
- native attention: BR-11 → BR-07;
- analytics: BR-20 отдельно от BR-21, затем bounded BR-21 child PRs по
  утверждённому metric contract;
- deployment skills: BR-12; BR-13 ждёт отдельного подтверждения migration и не
  выполняет destructive container cleanup.
- native sessions sidebar: BR-28 paired geometry capture → BR-29; отдельный
  layout fix создаётся только если BR-28 докажет sibling reflow.

BR-04, BR-08, BR-14, BR-17, BR-19, BR-23 и BR-24 не образуют общую строгую
цепочку. Они не смешиваются в один PR и перед началом повторно проверяются на
file overlap с уже открытыми branches.

Каждый PR остаётся change PR без version bump. Release и production rollout
выполняются отдельно по `CONTRIBUTING.md`.

### BR-31 — Wake the webview delivery pump on host posts

**Status:** ready; подтверждён source defect (INC-2026-0002). **Dependency:**
none; BR-25 completion зависит от него. **Acceptance:** automated test на
wake без кадра + Windows CEF runtime gate (reveal без pointer).

`web::deliver()` в `crates/shell/src/web/mod.rs` кладёт кадр в per-view outbox
и ставит id в `PULL_PENDING`, но пробуждение насоса (`WAKE`) происходит
только из `repaint_requested()` — по кадру или resize CEF.
`flush_pending_pulls()` в `crates/shell/src/web/pump.rs` выполняется лишь на
тике насоса, поэтому страница, которая ничего не рисует, не получает
`__kaminPull()` и не забирает ни события, ни invoke-ответы, пока какое-нибудь
вью не перерисуется: движение мыши, стриминг, пересоздание соседнего вью.
Именно это даёт «пустая панель агентов до движения мыши», зависшие invoke из
статичных панелей и ложную картину «exthost завис».

Fix: `deliver()` и любой другой путь, ставящий `PULL_PENDING`, будит насос тем
же каналом, что `repaint_requested()`, либо получает собственный event-driven
flush; polling или timer не добавляются. Tests: unit test, что постановка в
`PULL_PENDING` приводит к flush без кадра; Windows gate — процедура R6 из
INC-2026-0002: reveal пересозданного Agents view без pointer даёт rows и
invoke reply не позже 1 с в статичной странице.
