# Исполняемый реестр Claude Bridge runtime

Этот файл отвечает только за маршрутизацию работ из
[`RUNTIME_RELIABILITY.md`](RUNTIME_RELIABILITY.md) и отдельных карточек
[`runtime-issues/INC-*.md`](runtime-issues/). Подробные факты, причины,
ограничения и acceptance остаются в исходных карточках и здесь не дублируются.

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

## Текущая пачка `RV-2026-09-06` — приёмка уже смерженного кода

**Status:** waiting — остаются Windows UI/authenticated live gates.
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
| [BR-04](RUNTIME_RELIABILITY.md#br-04--recover-webviews-after-extension-host-respawn) | ready | change | exthost recovery | none | Recovery PR с automated и Windows gates |
| [BR-05](RUNTIME_RELIABILITY.md#br-05--rehydrate-authoritative-connection-state) | ready | verify | connection | код PR #14 merged | Остаток live gate: send/close-tab/server errors на exact-main server |
| [BR-06](RUNTIME_RELIABILITY.md#br-06--webview-update-stalls-until-pointer-activity) | waiting | verify | rendering/connection | BR-31 | Повторить оба исходных Chat no-pointer сценария; сверить duplicate/остаток |
| [BR-07](RUNTIME_RELIABILITY.md#br-07--surface-native-claude-attention-in-chat) | waiting | change | native attention | BR-11 | Минимальный tab-scoped Console banner |
| [BR-08](RUNTIME_RELIABILITY.md#br-08--explain-unexpected-automatic-reload-skills) | ready | change | skills sync | none | Bounded revision/reason telemetry PR + observation |
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
| [BR-19](RUNTIME_RELIABILITY.md#br-19--treat-shell-disconnect-as-lifecycle-cancellation-not-extension-crash) | ready | change | lifecycle RPC | none | Typed generation-scoped cancellation PR |
| [BR-20](RUNTIME_RELIABILITY.md#br-20--restore-complete-claude-plan-usage-windows) | ready | change | account usage | none | Dynamic usage-window compatibility PR |
| [BR-21](RUNTIME_RELIABILITY.md#br-21--define-and-reconcile-dashboard-analytics-semantics) | ready | research | analytics | none | Сначала metric-contract decision PR; child PR ниже |
| [BR-22](RUNTIME_RELIABILITY.md#br-22--keep-live-chat-render-window-populated-by-drawable-rows) | blocked | verify | chat history | paired private diagnostic dumps | Classification одного расходящегося drawable path |
| [BR-23](RUNTIME_RELIABILITY.md#br-23--make-session-complete-notifications-transient-and-turn-scoped) | ready | change | notifications | PR #14 уже merged | Turn-scoped transient completion PR |
| [BR-24](RUNTIME_RELIABILITY.md#br-24--bound-and-reconcile-lost-webview-invoke-replies) | ready | change | invoke transport | BR-31 перед финальным runtime gate | Bounded invoke/pump diagnostics; затем проверить residual guarantees |
| [BR-25](RUNTIME_RELIABILITY.md#br-25--verify-agents-view-delivery-and-rehydration-after-reveal) | waiting | verify | Agent Teams UI | BR-31; baseline + BR-27 + BR-26 done | Пачка B: completion gate без repaint workaround |
| [BR-26](RUNTIME_RELIABILITY.md#br-26--publish-agent-replay-state-atomically) | done | none | Agent Teams UI | none | Generation-scoped replay staging в `main`; BR-25 completion ждёт BR-31 |
| [BR-27](RUNTIME_RELIABILITY.md#br-27--derive-active-and-completed-from-one-lifecycle-partition) | done | none | Agent Teams UI | none | Partition + parser fix в `main`; BR-26 также done |
| [BR-28](RUNTIME_RELIABILITY.md#br-28--measure-sidebar-geometry-during-session-hover) | done | none | native sidebar | none | Paired artifact INC-2026-0003: sibling reflow не воспроизводится |
| [BR-29](RUNTIME_RELIABILITY.md#br-29--make-hover-to-rename-transition-atomic) | ready | verify | native sidebar | код PR #37 merged | Windows focus/typing/keyboard/hitbox acceptance |
| [BR-30](RUNTIME_RELIABILITY.md#br-30--keep-incident-log-records-atomic-across-rotation) | done | none | diagnostics | none | PR #31 + #47; Windows 12/12 на main 686cc92; BR-02 разблокирован |
| [BR-31](RUNTIME_RELIABILITY.md#br-31--wake-the-webview-delivery-pump-on-host-posts) | ready | change | CEF/webview delivery | none | Следующая пачка: event-driven pull wake (INC-2026-0002) |

## Очередь отдельных incidents

Входящая очередь определяется файлами `runtime-issues/INC-*.md` с незакрытым
статусом (`reported`, `confirmed`, `investigation` или `blocked`). Создание или
уточнение одной карточки не требует правки этого файла: один Diagnostic PR
меняет один уникальный incident path и может выполняться параллельно с другими.

Этот файл меняется отдельным coordination PR только при продвижении выбранных
ID в текущую или планируемую пачку. Maintainer фиксирует snapshot ID в начале
запуска; более новые карточки остаются во входящей очереди до следующего
запуска. `INC-2026-0001` уже продвинут в планируемую пачку C ниже.

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
docs/process PR, чтобы новый snapshot был видим до запуска агента.
