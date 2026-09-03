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
- `done` — functional часть уже находится в `main`; отдельное production
  observation не возвращает задачу в очередь реализации.

Тип результата:

- `change` — один bounded Change/Fix PR с кодом и tests;
- `verify` — Diagnostic PR с воспроизводимым runtime artifact; произвольный fix
  до результата запрещён;
- `research` — Diagnostic/decision PR, который фиксирует контракт или данные;
- `observation` — только заранее определённая полевая проверка;
- `none` — новых PR по задаче сейчас не требуется.

## Текущая пачка `RB-2026-09-A`

**Status:** ready. **Base:** первый task начинается от `origin/main`, содержащего
PR #27. **Запуск:** владелец пишет: `Выполни текущую runtime-пачку по правилам
репозитория`.

Maintainer agent фиксирует этот список в начале запуска и не добавляет в него
другие BR-задачи:

1. **BR-30** — исправить атомарность structured records и смержить Change/Fix
   PR после filesystem tests.
2. **BR-25, baseline** — выполнить authenticated Windows Agent Teams gate на
   коде после PR #21 и сохранить sanitized Diagnostic outcome.
3. **BR-27** — исправить единую partition `Active`/`Completed`; использовать
   результат baseline, но не смешивать с transport defect, если он обнаружен.
4. **BR-26** — публиковать replay snapshot атомарно, уже поверх BR-27.
5. **BR-25, completion** — повторить gate после BR-27/BR-26. Не покрытый ими
   delivery defect получает отдельный новый BR child; polling/timer не
   добавляется как догадка.
6. **BR-28** — снять paired Windows geometry artifact без layout-правок.
7. **BR-29** — исправить hover-to-rename lifecycle поверх результата BR-28.
   Отдельный sibling-layout fix создаётся только если BR-28 доказал reflow.

Все шаги выполняются последовательно, каждый в новой branch/worktree от
свежего `origin/main`. Blocked результат одного track не задерживает независимый
track. После последнего release-relevant merge пачки выполняется один release;
Diagnostic-only результаты сами по себе release не вызывают.

Пачка считается завершённой, когда её Change/Fix PR смержены либо получили
проверяемый blocked outcome, BR-25 и BR-28 имеют сохранённый результат, а этот
раздел обновлён отдельным docs/process PR. Следующая планируемая пачка не
начинается в том же запуске.

## Реестр задач

| ID | State | Result | Track | Строгий prerequisite | Следующий artifact |
| --- | --- | --- | --- | --- | --- |
| [BR-01](RUNTIME_RELIABILITY.md#br-01--durable-incident-diagnostics) | done | observation | diagnostics | none | Только следующий реальный incident bundle |
| [BR-02](RUNTIME_RELIABILITY.md#br-02--calibrate-the-long-session-memory-envelope) | waiting | research | long session | BR-30 | Повторяемый Windows envelope artifact |
| [BR-03](RUNTIME_RELIABILITY.md#br-03--long-session-mitigation) | deferred | change | long session | решение BR-02 | Минимальная доказанная mitigation |
| [BR-04](RUNTIME_RELIABILITY.md#br-04--recover-webviews-after-extension-host-respawn) | ready | change | exthost recovery | none | Recovery PR с automated и Windows gates |
| [BR-05](RUNTIME_RELIABILITY.md#br-05--rehydrate-authoritative-connection-state) | done | none | connection | none | PR #14 в `main` |
| [BR-06](RUNTIME_RELIABILITY.md#br-06--webview-update-stalls-until-pointer-activity) | ready | verify | rendering/connection | none | Paired no-pointer Windows trace; fix только при повторе |
| [BR-07](RUNTIME_RELIABILITY.md#br-07--surface-native-claude-attention-in-chat) | waiting | change | native attention | BR-11 | Минимальный tab-scoped Console banner |
| [BR-08](RUNTIME_RELIABILITY.md#br-08--explain-unexpected-automatic-reload-skills) | ready | change | skills sync | none | Bounded revision/reason telemetry PR + observation |
| [BR-09](RUNTIME_RELIABILITY.md#br-09--make-agent-teams-report-delivery-explicit) | done | none | Agent Teams | none | PR #15 в `main` |
| [BR-10](RUNTIME_RELIABILITY.md#br-10--show-the-effective-hook-in-approval-ui) | done | none | hooks | none | PR #13 в `main` |
| [BR-11](RUNTIME_RELIABILITY.md#br-11--inventory-native-cli-only-blocking-states) | ready | research | native attention | none | Versioned blocker inventory |
| [BR-12](RUNTIME_RELIABILITY.md#br-12--establish-the-server-and-builtin-skills-baseline) | ready | verify | deployment skills | none | Versioned roots/mounts inventory без contents |
| [BR-13](RUNTIME_RELIABILITY.md#br-13--retire-the-legacy-bridge-sync-mount-safely) | blocked | observation | deployment | owner confirms every deployment migration | Migration proof до cleanup PR |
| [BR-14](RUNTIME_RELIABILITY.md#br-14--make-release-artifact-provenance-verifiable) | ready | change | release | none | Fail-closed provenance PR + dry run |
| [BR-15](RUNTIME_RELIABILITY.md#br-15--evaluate-automatic-agent-teams-selection) | waiting | research | Agent Teams | BR-25 + BR-27 + BR-26 | Versioned orchestration eval |
| [BR-16](RUNTIME_RELIABILITY.md#br-16--stabilize-upward-history-scroll-anchoring) | waiting | change | chat history | classification BR-22 | Keyed-anchor PR с CEF displacement gate |
| [BR-17](RUNTIME_RELIABILITY.md#br-17--persist-privacy-safe-bridge-server-logs) | ready | change | server operations | none | Persistent bounded logs в disposable compose |
| [BR-18](RUNTIME_RELIABILITY.md#br-18--keep-sessionend-relay-available-and-secret-safe-during-teardown) | done | observation | teardown | BR-17 для остаточной причины | PR #20 в `main`; trigger классифицируется по новым logs |
| [BR-19](RUNTIME_RELIABILITY.md#br-19--treat-shell-disconnect-as-lifecycle-cancellation-not-extension-crash) | ready | change | lifecycle RPC | none | Typed generation-scoped cancellation PR |
| [BR-20](RUNTIME_RELIABILITY.md#br-20--restore-complete-claude-plan-usage-windows) | ready | change | account usage | none | Dynamic usage-window compatibility PR |
| [BR-21](RUNTIME_RELIABILITY.md#br-21--define-and-reconcile-dashboard-analytics-semantics) | ready | research | analytics | none | Сначала metric-contract decision PR; child PR ниже |
| [BR-22](RUNTIME_RELIABILITY.md#br-22--keep-live-chat-render-window-populated-by-drawable-rows) | blocked | verify | chat history | paired private diagnostic dumps | Classification одного расходящегося drawable path |
| [BR-23](RUNTIME_RELIABILITY.md#br-23--make-session-complete-notifications-transient-and-turn-scoped) | ready | change | notifications | PR #14 уже merged | Turn-scoped transient completion PR |
| [BR-24](RUNTIME_RELIABILITY.md#br-24--bound-and-reconcile-lost-webview-invoke-replies) | ready | change | invoke transport | none | Сначала bounded correlation/lifecycle diagnostics PR |
| [BR-25](RUNTIME_RELIABILITY.md#br-25--verify-agents-view-delivery-and-rehydration-after-reveal) | waiting | verify | Agent Teams UI | baseline done (INC-2026-0002); completion после BR-31 + BR-27 + BR-26 | Текущая пачка: completion gate |
| [BR-26](RUNTIME_RELIABILITY.md#br-26--publish-agent-replay-state-atomically) | ready | change | Agent Teams UI | none | Текущая пачка: generation-scoped staging PR |
| [BR-27](RUNTIME_RELIABILITY.md#br-27--derive-active-and-completed-from-one-lifecycle-partition) | done | none | Agent Teams UI | none | Partition + parser fix в `main`; BR-26 разблокирован |
| [BR-28](RUNTIME_RELIABILITY.md#br-28--measure-sidebar-geometry-during-session-hover) | ready | verify | native sidebar | none | Текущая пачка: paired geometry artifact |
| [BR-29](RUNTIME_RELIABILITY.md#br-29--make-hover-to-rename-transition-atomic) | waiting | change | native sidebar | BR-28 | Текущая пачка: atomic rename transition |
| [BR-30](RUNTIME_RELIABILITY.md#br-30--keep-incident-log-records-atomic-across-rotation) | done | none | diagnostics | none | Record-aware writer PR в `main`; BR-02 evidence run разблокирован |
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

Они не являются частью текущего snapshot и не запускаются автоматически:

- **B — independent fixes:** BR-19 → BR-17 → BR-20 → BR-23 → BR-04 → BR-14;
- **C — contracts and diagnostics:** INC-2026-0001, BR-21 decision, BR-24
  diagnostics, BR-11, BR-12, BR-08, BR-06 и BR-02;
- **conditional:** BR-21A–E, BR-07, BR-15, BR-16, BR-03 и любой child из
  BR-22/BR-24/BR-28 только после их prerequisites;
- **owner-blocked:** BR-13 и BR-22 evidence collection.

Перед продвижением следующей пачки maintainer повторно сверяет её с актуальным
`origin/main`, открытыми PR и новыми incidents. Продвижение оформляется
docs/process PR, чтобы новый snapshot был видим до запуска агента.
