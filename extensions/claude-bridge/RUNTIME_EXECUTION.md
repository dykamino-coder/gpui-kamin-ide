# Исполняемый реестр Claude Bridge runtime

Этот файл отвечает только за маршрутизацию работ из
[`RUNTIME_RELIABILITY.md`](RUNTIME_RELIABILITY.md) и отдельных карточек
[`runtime-issues/INC-*.md`](runtime-issues/). Подробные факты, причины,
ограничения и acceptance остаются в исходных карточках и здесь не дублируются.

Полный принятый runtime backlog в `origin/main` состоит из BR-реестра ниже и
всех незакрытых INC-карточек, включая ещё не выбранные в пачку. Автор задачи
сам завершает её [регистрацию](../../CONTRIBUTING.md#task-registration) через
merge Diagnostic PR. Отсутствие PR реализации или строки INC в текущей пачке
не означает отсутствие задачи. Открытый PR только с постановкой — ещё не
завершённая автором регистрация; он не заменяет карточку в `main`.

Общее поручение `делай задачи` запускает выбор из всего принятого backlog и
открытых PR результатов по
[maintainer flow](../../docs/MAINTAINER_PR_FLOW.md#general-backlog-run).
Мейнтейнер сам оформляет новую пачку coordination PR, сохраняя историю,
ограничения и остатки прежней. Команда `Выполни текущую runtime-пачку по
правилам репозитория` выбирает только текущий раздел ниже; `продолжай`
возобновляет ранее порученный snapshot. Изменение правил запуска не продвигает
задачи и не меняет состав или статус существующих пачек.

## Значения полей

- `ready` — следующий artifact можно делать от свежего `origin/main`;
- `waiting` — сначала должен завершиться указанный prerequisite;
- `blocked` — нужен внешний evidence или решение владельца;
- `deferred` — работа намеренно не начинается до условия из BR-карточки;
- `done` — результат находится в `main` и обязательная приёмка подтверждена
  (для Diagnostic task допустим доказанный outcome без fix); отдельное production
  observation не возвращает задачу в очередь реализации. Пропущенный Windows
  merge gate остаётся `verify`, даже если functional код уже merged.

Тип результата:

- `change` — один bounded Change/Fix PR с кодом и tests;
- `verify` — Diagnostic PR с воспроизводимым runtime artifact; произвольный fix
  до результата запрещён;
- `research` — Diagnostic/decision PR, который фиксирует контракт или данные;
- `observation` — только заранее определённая полевая проверка;
- `none` — новых PR по задаче сейчас не требуется.

## Текущая пачка `RV-2026-09-29` — реализация слита, приёмка открыта

**Status:** verification pending. **Snapshot:** `origin/main` `53439c554b5f979bfbddbf9a857c0342a6353fac`
(релиз KaminIDE 1.0.59 / server 6.3.136). Открытых PR на момент snapshot нет.
**Запуск:** владелец поручил обновить версию приложения, слить очередь PR и
заняться проблемами, описанными в репозитории — общий запуск по
[maintainer flow](../../docs/MAINTAINER_PR_FLOW.md#general-backlog-run).
Ограничений `review only`, `без merge`, `без release` владелец не ставил.

Выбор сделан из принятого backlog: 32 BR-карточки и 51 карточка
`runtime-issues/INC-*.md`, из которых 48 без PR решения. В пачку взяты только
задачи со статусом `ready`. Исходная запись ошибочно приравняла Windows CI job
к обязательному Windows runtime gate: job собирает и тестирует код, но не
выполняет сценарии в живом интерфейсе.

1. **BR-19** — `change`: разрыв shell-соединения обрабатывается как отмена
   жизненного цикла, а не как падение расширения. Типизированная отмена с
   областью поколения вместо `failAll` + классификация post-boot unhandled
   rejection.
2. **BR-31** — `change`: пробуждение насоса доставки webview на посты хоста.
   Строгий prerequisite для завершения BR-25.
3. **BR-04** — `change`: восстановление webview после respawn extension-host —
   обработчик `kamin:exthost:respawned` и повторный посев provider/state.

Порядок последовательный: BR-19 и BR-04 пересекаются по `src/kamin-host/`,
BR-31 — по доставке в webview. После каждого merge выполняется новый `fetch` и
проверка следующей ветки на overlap с обновлённым `main`.

**Отложено с причинами (остаётся в backlog, не в этой пачке):**

- **BR-18A**, **BR-20** — acceptance требует isolated authenticated Linux
  server/browser, у мейнтейнера такого контура нет; шаг помечен как
  owner-only post-merge observation.
- **BR-02**, **BR-17** — research/operational PR вне поручения этой пачки.
- **INC-2026-0011, -0020, -0025, -0043, -0054** (P1 без PR решения) и 42
  карточки P2 — остаются принятым backlog. Каждая требует собственного
  research или evidence, не покрытого automated-гейтами этой пачки; включение
  их в пачку без этого дало бы speculative fix, что запрещено разделом
  «Snapshot runtime-пачки».

**Implementation and release:** BR-19 — PR #146, BR-31 — PR #147, BR-04 —
PR #148. Единственный release исходного snapshot опубликован через PR #150
(KaminIDE 1.0.60 / server 6.3.137). Поздний ограниченный остаток BR-04
(повторный посев открытых документов) слит отдельным PR #152 и вошёл в
release PR #163 (1.0.61); его включение в новый coordination snapshot не
зафиксировано. Это не меняет задним числом состав исходной пачки.

**Открытая приёмка:** для BR-19 нужен живой Windows-разрыв и восстановление
shell-соединения без ложного `Extension crashed`; для BR-31 — R6 из
INC-2026-0002 (reveal Agents без pointer → строки и invoke reply ≤ 1 с);
для BR-04 — принудительный respawn ребёнка на Windows с возвратом Chat,
Console, соседней вью и состояния документов. CI этих сценариев не выполнял.
Реализация в `main` не означает `done`; до task-specific evidence эти ID
остаются `verify`. Непокрытые active editor, selection и LSP state BR-04
требуют отдельного bounded child после уточнения контрактов. Новый release
не следует из этой незакрытой verification-пачки автоматически.

## История `RV-2026-09-06` — приёмка уже смерженного кода

**Status:** проход окончен; остатки перенесены в backlog. Windows UI и
authenticated live gates по BR-05/09/10/29 не выполнены и завершёнными не
объявляются: они ждут доступного контура у владельца.
**Snapshot:** `origin/main` `686cc92b6c935e8ffb2416cf7b3b6f22c6f19ba2`, после
merge и повторной проверки BR-30/#47. **Запуск:** владелец поручил закрыть
остатки уже реализованных задач без новых functional fixes. Аудит и команды:
[`runtime-closeout-2026-09-06.md`](../../docs/runtime-closeout-2026-09-06.md).

1. **BR-30** — done: Windows filesystem tests 12/12 на PR candidate и на `main`;
   PR #47 merged, close-out основан на post-merge run 34030247540.
2. **BR-05** — verify: send после recovery, close tab при reconnect,
   max-sessions/session-not-found на server из проверяемого `main`.
3. **BR-09** — verify: 3/3 reports без recovery и отдельный один bounded
   recovery, при tab switch/reconnect.
4. **BR-10** — verify: Windows focus/keyboard containment и соседние controls.
5. **BR-29** — verify: фактический focus/typing и keyboard/hitbox rename paths.

Владелец шагов 2–5 — maintainer с доступным Windows UI test environment и,
где требуется, isolated authenticated Linux server. Эта macOS-ревизия не
запускала и не запрашивала Computer Use на машине владельца. Доступные CLI/CI
проверки не выдаются за native UI acceptance.

Каждый остаток получает отдельный verification PR с exact SHA, versions,
scenario/outcome и sanitized evidence по исходной карточке. Исходный код уже
merged; до получения evidence состояния `done` для BR-05/09/10/29 сняты.
Если gate обнаруживает дефект, сохранить task открытой, связать существующий
BR или завести bounded child. В этой пачке разрешены tests и описания;
новые functional fixes и release не входят в поручение. Недоступный gate
фиксируется в карточке и не останавливает независимые проверки.

## История `RB-2026-09-A` — проход окончен 2026-09-03

**Status:** execution pass ended; full acceptance не завершена: BR-25 completion
перенесён, BR-29 verification восстановлена аудитом 2026-09-06.
**Base:** `origin/main` с PR #27; snapshot maintainer agent
зафиксирован на `8a1e6f9`. **Запуск:** владелец написал `Выполни текущую
runtime-пачку по правилам репозитория`.

Результат по шагам:

1. **BR-30** — done: Change/Fix PR #31 (record-aware incident-log writer).
2. **BR-25, baseline** — done: Diagnostic PR #33, private INC-2026-0002.
   Потеря локализована в shell delivery pump (новый child **BR-31**, `ready`),
   не в host cache/fan-out/parser.
3. **BR-27** — done: Change/Fix PR #34 (единая partition + parser fix для
   строковых `<teammate-message>`/`<task-notification>`), Windows gate пройден.
4. **BR-26** — done: Change/Fix PR #36 (generation-scoped replay staging),
   Windows gate пройден.
5. **BR-25, completion** — waiting: gate без pointer невозможен до BR-31;
   повторяется первым шагом следующей пачки вместе с BR-31.
6. **BR-28** — done: Diagnostic PR #35, private INC-2026-0003 — sibling reflow
   не воспроизводится в проверенной матрице; diagnostic task закрыта.
7. **BR-29** — implementation merged: Change/Fix PR #37 (atomic hover→rename, id-scoped якорь pill),
   Windows GPUI probe gate выполнен; focus/keyboard остаток теперь в `RV-2026-09-06`.
   Merge стал возможен после PR #40, который сделал
   job «Rust checks on Windows» зелёным на `main`.

**Release.** Release PR #38 `chore(release): KaminIDE 1.0.55 / server 6.3.132`
влит (`main` d9321a1) после зелёного CI. Installer
`KaminIDE_1.0.55_x64-setup.exe` собран из HEAD c599cae, проверен тихой
установкой поверх 1.0.54 и опубликован в GitHub Release `kaminide-latest`;
образ `dykamino/open-claude-bridge:6.3.132` (+ `latest`) опубликован вручную
через podman: действовавший в тот момент `docker.yml` падал на login без
секретов Docker Hub. CI Rust-gate починен отдельным PR #40 (fmt, clippy,
корпуса тестов, pin toolchain 1.96.0), после чего BR-29 (#37) влит. Новый
fail-closed release flow находится в `main` после PR #43–#44; его первый
production-run требует repository secrets и отдельный следующий Release PR.

Планируемая пачка реализации не начиналась в том запуске и не входит в
текущую verification-пачку. Первыми кандидатами
для неё остаются BR-31 (delivery pump wake) + BR-25 completion, затем BR-15
(зависит от BR-09 live acceptance и BR-25/27/26). Наблюдавшиеся во время пачки общие CI/release blockers
устранены PR #40 и PR #43–#44 и не считаются открытыми runtime-задачами.

## Реестр задач

| ID | State | Result | Track | Строгий prerequisite | Следующий artifact |
| --- | --- | --- | --- | --- | --- |
| [BR-01](RUNTIME_RELIABILITY.md#br-01--durable-incident-diagnostics) | done | observation | diagnostics | none | Только следующий реальный incident bundle |
| [BR-02](RUNTIME_RELIABILITY.md#br-02--calibrate-the-long-session-memory-envelope) | ready | research | long session | BR-01 и BR-30 выполнены | Повторяемый Windows envelope artifact |
| [BR-03](RUNTIME_RELIABILITY.md#br-03--long-session-mitigation) | deferred | change | long session | решение BR-02 | Минимальная доказанная mitigation |
| [BR-04](RUNTIME_RELIABILITY.md#br-04--recover-webviews-after-extension-host-respawn) | ready | verify | exthost recovery | PR #148 и #152 merged | Windows respawn gate для вью/документов; active editor/selection/LSP — отдельный child после уточнения |
| [BR-05](RUNTIME_RELIABILITY.md#br-05--rehydrate-authoritative-connection-state) | ready | verify | connection | код PR #14 merged | Остаток live gate: send/close-tab/server errors на exact-main server |
| [BR-06](RUNTIME_RELIABILITY.md#br-06--webview-update-stalls-until-pointer-activity) | ready | verify | rendering/connection | BR-31 implementation PR #147 merged; его R6 ещё открыт | Повторить оба исходных Chat no-pointer сценария и Plugins; сверить duplicate/остаток |
| [BR-07](RUNTIME_RELIABILITY.md#br-07--surface-native-claude-attention-in-chat) | waiting | change | native attention | BR-11 | Минимальный tab-scoped Console banner |
| [BR-08](RUNTIME_RELIABILITY.md#br-08--explain-unexpected-automatic-reload-skills) | ready | change | skills sync | none | Prepared bounded owner-scoped revision/reason telemetry; PR review, then deployment-owner observation; no reload behavior change |
| [BR-09](RUNTIME_RELIABILITY.md#br-09--make-agent-teams-report-delivery-explicit) | ready | verify | Agent Teams | код PR #15 merged | 3/3 reports и один bounded recovery на authenticated Windows/Linux gate |
| [BR-10](RUNTIME_RELIABILITY.md#br-10--show-the-effective-hook-in-approval-ui) | ready | verify | hooks | код PR #13 merged | Настоящие focus/Tab/Shift+Tab/approve/reject на Windows |
| [BR-11](RUNTIME_RELIABILITY.md#br-11--inventory-native-cli-only-blocking-states) | ready | research | native attention | none | Versioned blocker inventory |
| [BR-12](RUNTIME_RELIABILITY.md#br-12--establish-the-server-and-builtin-skills-baseline) | blocked | verify | deployment skills | owner inventory действующего deployment | Versioned roots/mounts/ownership без contents |
| [BR-13](RUNTIME_RELIABILITY.md#br-13--retire-the-legacy-bridge-sync-mount-safely) | blocked | observation | deployment | owner confirms every deployment migration | Migration proof до cleanup PR |
| [BR-14](RUNTIME_RELIABILITY.md#br-14--make-release-artifact-provenance-verifiable) | done | observation | release | none | PR #43–#44 в `main`; первый автоматический release после добавления secrets |
| [BR-15](RUNTIME_RELIABILITY.md#br-15--evaluate-automatic-agent-teams-selection) | waiting | research | Agent Teams | BR-09 live gate + BR-25 completion; BR-27/26 done | Versioned orchestration eval |
| [BR-16](RUNTIME_RELIABILITY.md#br-16--stabilize-upward-history-scroll-anchoring) | waiting | change | chat history | classification BR-22 | Keyed-anchor PR с CEF displacement gate |
| [BR-17](RUNTIME_RELIABILITY.md#br-17--persist-privacy-safe-bridge-server-logs) | ready | change | server operations | none | Persistent bounded logs в disposable compose |
| [BR-18](RUNTIME_RELIABILITY.md#br-18--keep-sessionend-relay-available-and-secret-safe-during-teardown) | done | observation | teardown | BR-17 для остаточной причины | PR #20 в `main`; trigger классифицируется по новым logs |
| [BR-18A](RUNTIME_RELIABILITY.md#br-18a--classify-local-sessionend-execution-on-explicit-end) | ready | verify | local hooks | BR-18 relay fix merged | Bounded local SessionEnd outcome/contract artifact; functional fix отдельно |
| [BR-19](RUNTIME_RELIABILITY.md#br-19--treat-shell-disconnect-as-lifecycle-cancellation-not-extension-crash) | ready | verify | lifecycle RPC | PR #146 merged | Живой Windows disconnect/reconnect без ложного crash; task-specific evidence |
| [BR-20](RUNTIME_RELIABILITY.md#br-20--restore-complete-claude-plan-usage-windows) | ready | change | account usage | none | Dynamic usage-window compatibility PR |
| [BR-21](RUNTIME_RELIABILITY.md#br-21--define-and-reconcile-dashboard-analytics-semantics) | ready | research | analytics | none | Сначала metric-contract decision PR; child PR ниже |
| [BR-22](RUNTIME_RELIABILITY.md#br-22--keep-live-chat-render-window-populated-by-drawable-rows) | blocked | verify | chat history | paired private diagnostic dumps | Classification одного расходящегося drawable path |
| [BR-23](RUNTIME_RELIABILITY.md#br-23--make-session-complete-notifications-transient-and-turn-scoped) | ready | change | notifications | PR #161 и regression fix #164 merged | Отдельный explicit transient host contract; текущий toast всё ещё закрывается вручную |
| [BR-24](RUNTIME_RELIABILITY.md#br-24--bound-and-reconcile-lost-webview-invoke-replies) | ready | change | invoke transport | BR-31 implementation PR #147 merged; R6 открыт | Bounded invoke/generation diagnostics; затем проверить residual guarantees |
| [BR-25](RUNTIME_RELIABILITY.md#br-25--verify-agents-view-delivery-and-rehydration-after-reveal) | waiting | verify | Agent Teams UI | BR-31 R6 ещё открыт; baseline + BR-27 + BR-26 done | Completion gate без repaint workaround после BR-31 R6 |
| [BR-26](RUNTIME_RELIABILITY.md#br-26--publish-agent-replay-state-atomically) | done | none | Agent Teams UI | none | Generation-scoped replay staging в `main`; BR-25 completion ждёт BR-31 |
| [BR-27](RUNTIME_RELIABILITY.md#br-27--derive-active-and-completed-from-one-lifecycle-partition) | done | none | Agent Teams UI | none | Partition + parser fix в `main`; BR-26 также done |
| [BR-28](RUNTIME_RELIABILITY.md#br-28--measure-sidebar-geometry-during-session-hover) | done | none | native sidebar | none | Paired artifact INC-2026-0003: sibling reflow не воспроизводится |
| [BR-29](RUNTIME_RELIABILITY.md#br-29--make-hover-to-rename-transition-atomic) | ready | verify | native sidebar | код PR #37 merged | Windows focus/typing/keyboard/hitbox acceptance |
| [BR-30](RUNTIME_RELIABILITY.md#br-30--keep-incident-log-records-atomic-across-rotation) | done | none | diagnostics | none | PR #31 + #47; Windows 12/12 на main 686cc92; BR-02 разблокирован |
| [BR-31](RUNTIME_RELIABILITY.md#br-31--wake-the-webview-delivery-pump-on-host-posts) | ready | verify | CEF/webview delivery | PR #147 merged | Windows R6 без pointer/forced repaint; rows и invoke reply ≤ 1 с |

## Зарегистрированные incidents вне BR-реестра

Очередь определяется файлами `runtime-issues/INC-*.md` в `origin/main` с
незакрытым статусом (`reported`, `confirmed`, `investigation` или `blocked`).
Автор сам проверяет и мержит регистрацию своей открытой карточки. Создание или
уточнение карточки не требует правки этого файла: один Diagnostic PR меняет
один уникальный incident path; merge регистраций выполняется последовательно.
Неизвестная причина остаётся задачей на research; готовое решение получает
отдельный связанный PR и проходит maintainer acceptance.

Этот файл меняется отдельным coordination PR только при продвижении выбранных
ID в текущую или планируемую пачку. Maintainer фиксирует snapshot ID в начале
запуска; входящие карточки после snapshot остаются в backlog до следующего
запуска. Собственный новый child внутри порученного scope можно включить
отдельным coordination PR по
[maintainer flow](../../docs/MAINTAINER_PR_FLOW.md#задача-обнаруженная-мейнтейнером).
Ограничения текущей verification-пачки при этом сохраняются.
`INC-2026-0001` уже продвинут в планируемую пачку C ниже.

## Декомпозиция BR-21

BR-21 не является одним Change/Fix PR. Первый decision PR утверждает значения
метрик, identity, time range и freshness. Только после его merge создаются
последовательно проверяемые child tasks:

1. `BR-21A` — canonical assistant-turn relation, parent attribution и token UUID;
2. `BR-21B` — range/timezone filters и compact payload consumption;
3. `BR-21C` — freshness/invalidation и live top-card counters;
4. `BR-21D` — model aliases и versioned price provenance;
5. `BR-21E` — labels/tooltips для quota, context и throughput semantics.

Каждый child получает отдельный PR и fixtures из acceptance BR-21. Нельзя
начинать их до decision PR или объединять BR-20 quota compatibility с локальной
JSONL analytics.

## Следующие планируемые пачки

Они не входят в verification snapshot `RV-2026-09-06` и не запускаются автоматически:

- **B — Agent Teams completion + independent fixes:** BR-31 → BR-25 completion
  → BR-15 (после live gate BR-09); затем BR-19 → BR-17 → BR-20 → BR-23 → BR-04;
- **C — contracts and diagnostics:** INC-2026-0001, BR-21 decision, BR-24
  diagnostics, BR-18A, BR-11, BR-08, BR-06 (после BR-31) и BR-02;
- **conditional:** BR-21A–E, BR-07, BR-16, BR-03 и любой child из
  BR-22/BR-24/BR-28 только после их prerequisites;
- **owner-blocked:** BR-12 deployment inventory, BR-13 migration proof и
  BR-22 evidence collection.

Перед продвижением следующей пачки maintainer повторно сверяет её с актуальным
`origin/main`, открытыми PR и новыми incidents. Продвижение оформляется
docs/process PR, чтобы новый snapshot был принят до выполнения его deliverables.
