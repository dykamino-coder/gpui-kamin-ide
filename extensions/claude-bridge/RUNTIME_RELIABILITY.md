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

Наблюдения ниже зафиксированы 20–21 августа 2026 года на KaminIDE 1.0.53. Source
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
- toast `Extension crashed — Contained — extensions stayed alive: shell client
  disconnected` составляется не из native crash: shell WebSocket close вызывает
  `RpcEndpoint.failAll("shell client disconnected")`, отклонённый host-to-shell
  RPC доходит до необработанного Promise в child, а общий
  `unhandledRejection` containment показывает crash toast, оставляя extension
  host живым. Нарушенный cancellation contract описан в BR-19.

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
Windows compile/privacy/runtime gate; воспроизведение редкого crash через
10–15 часов остаётся post-merge production observation и не блокирует merge.

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

**Status:** ready. **Dependency:** none. **Acceptance:** automated + Windows
runtime merge gate.

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

**Status:** ready для relay lifecycle fix; причина запуска teardown остаётся
investigation и использует evidence из BR-17. **Dependency:** none для fix,
BR-17 для классификации исходного termination trigger. **Acceptance:** automated
+ authenticated Windows runtime merge gate.

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

## Current draft PR integration order

Все перечисленные PR остаются draft. `mergeable` относительно сегодняшнего
`main` не гарантирует корректность после предыдущего merge; checks и branch
protection в GitHub сейчас отсутствуют. Maintainer agent сливает строго по
одному и не закрывает PR без merge:

1. PR #12 — canonical docs/testing/backlog. Сначала включить текущие BR-17,
   BR-18, BR-19 и dependency template changes.
2. PR #13 — BR-10 hook approval UI; выполнить его Windows gate.
3. Обновить PR #14 от `origin/main`, пересобрать committed Bridge artifacts и
   повторить automated + Windows gates. Это обязательно после #13, потому что
   оба PR меняют `builtin-extensions/claude-bridge/chat.html`. Затем слить #14.
4. Обновить и проверить PR #15 на свежем `origin/main`, выполнить authenticated
   Agent Teams gate и слить #15.
5. Обновить PR #16 после #14/#15, пересобрать artifacts и повторить все checks,
   включая Windows Rust/runtime gate. #16 пересекается с #14 по connection
   state, shared types, `useInit`, host parent и generated artifacts; затем
   слить #16.

После каждого merge maintainer делает `fetch`, проверяет новый `origin/main` и
только затем обновляет следующий PR. Generated files не разрешаются через
`ours`/`theirs`: они пересобираются из объединённых sources.

## Recommended next task order

1. BR-18 secret-safe SessionEnd teardown relay.
2. BR-19 expected shell-disconnect cancellation без ложного crash toast.
3. BR-17 persistent server logs как независимый operational PR.
4. BR-04 recovery после extension-host respawn.
5. BR-14 release provenance guard идёт независимо и не блокирует runtime chain.
6. Повторный Windows-прогон tab switch и disconnect/reconnect. BR-06 создаётся
   как fix PR только если симптом сохранился; отдельная задача для reconnect не
   заводится.
7. BR-16 upward history anchoring идёт отдельным UI PR и не блокирует connection
   recovery chain.
8. BR-11 inventory native blockers, затем минимальный BR-07.
9. BR-08 без удаления существующего maintenance contract.
10. BR-12 deployment skills baseline; BR-13 независимо ждёт подтверждения legacy
   migration на всех deployments.
11. BR-15 Agent Teams selection eval.
12. BR-02 и только затем решение по BR-03.

Каждый PR остаётся change PR без version bump. Release и production rollout
выполняются отдельно по `CONTRIBUTING.md`.
