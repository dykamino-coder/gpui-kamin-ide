# Исполняемый реестр Claude Bridge runtime

Этот файл отвечает только за маршрутизацию работ из
[`RUNTIME_RELIABILITY.md`](RUNTIME_RELIABILITY.md) и отдельных карточек
[`runtime-issues/INC-*.md`](runtime-issues/). Подробные факты, причины,
ограничения и acceptance остаются в исходных карточках и здесь не дублируются.

Сверка доставки на 2026-10-19 и handoff каждого остатка:
[RUNTIME_BACKLOG.md](RUNTIME_BACKLOG.md). Сводка не начинает новую пачку,
не принимает runtime outcomes и не меняет историю прежних запусков. Новые
incidents после её source snapshot остаются входными данными следующей сверки.

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

## Текущая пачка `RC-2026-11-13-KAMIN-BRIDGE`

**Status:** in progress. **Snapshot:** public `origin/main`
`5624a330a3cf1976d24431b2caf649feae74d5c9` (2026-11-13), последний immutable
release `kaminide-v1.0.94` / server 6.3.171; private evidence: открыт только
#141 (engine geometry, вне пачки).

**Поручение владельца 2026-11-13:** «надо все добить по камину и бриджу задачи» —
общий запуск по [maintainer flow](../../docs/MAINTAINER_PR_FLOW.md#general-backlog-run)
только для KaminIDE/Bridge. Разрешены commit/push/PR/merge и один batch release.
Работа движка (`crates/html`, бенчи) и hardening/security не входят. Роль автора
этой записи — maintainer `kmaint`, единственный writer общего реестра.

**Completion target:** каждый ID пачки получает принятый outcome в `main`
(`done` с evidence, `verify` с конкретным непроведённым gate или `blocked` с
owner/условием); один проверенный release 1.0.95 после последнего
release-relevant PR; close-out реестра, обзора и INC-карточек.

### Исполнители и scope

| Исполнитель | ID / PR | Result | Gate |
| --- | --- | --- | --- |
| kfix | [INC-0108](runtime-issues/INC-2026-0108.md) | change | automated stdio transport test |
| kfix | [INC-0107](runtime-issues/INC-2026-0107.md) | research → change | automated + live Windows gate |
| kfix | [INC-0109](runtime-issues/INC-2026-0109.md) | research | repeated-frame harness / tolerance decision |
| kfix | [#697](https://github.com/dykamino-coder/gpui-kamin-ide/pull/697) (`783c7d02`) | change, accept | его CI и browser lifecycle proofs |
| kresearch | BR-11 → (BR-07 позже) | research | versioned CLI blocker inventory |
| kresearch | BR-21 | research | metric-contract decision; BR-21A–E только после merge |
| kresearch | BR-22 → BR-16 | research → change | classification; BR-16 с CEF displacement gate |
| kresearch | BR-23 | change | fake-clock + Windows native toast gate |
| kresearch | BR-24 | change | ждёт BR-31 R6 ниже |
| kresearch | BR-02 | research | Windows envelope artifact; BR-03 deferred до решения |
| kdeps | Codex CLI 0.160.1 → 0.167.2, Claude Code CLI pin (2.1.322), npm/base image/CEF/toolchain | change | CI + применимые smoke |
| duckspeed | ускорение страницы статистики пользователей админки (DuckDB ~5 ГБ; на production timeout у большинства из 34 карточек) | change | CI + замер на копии данных; **обязательная часть 1.0.95** |
| kmaint | BR-31 R6 (+ [INC-0002](runtime-issues/INC-2026-0002.md), BR-25 completion) | verify | Windows CEF reveal без pointer, rows и invoke ≤ 1 с |
| kmaint | BR-06, BR-19, BR-04, BR-29, BR-10 | verify | живой Windows gate каждой карточки |
| kmaint | BR-05, BR-09, BR-18A | verify | Windows + isolated authenticated Linux server (Podman) |
| kmaint | BR-17, BR-08 | observation | локальный стенд как дополнение; production — deployment owner |

### Порядок и зависимости

1. Этот coordination PR (snapshot) — до merge любых результатов пачки.
2. kmaint verify track последовательно: BR-31 R6 → BR-25 completion / INC-0002 →
   BR-06 → BR-19 → BR-04 → BR-29 → BR-10 → BR-05 → BR-09 → BR-18A → BR-17/08.
   Каждый ID — отдельный Diagnostic PR с exact candidate и evidence.
3. BR-24 change ждёт принятого BR-31 R6; BR-16 ждёт BR-22; BR-07 ждёт BR-11;
   BR-15 ждёт BR-09 live gate и BR-25 completion; BR-03 ждёт BR-02.
4. Fix PR kfix/kresearch/kdeps принимаются по одному через общую merge-очередь,
   с fetch, base/head SHA и зелёными required checks; живой gate — по карточке.
5. До release лид подтверждает merge или исключение #797, #804 и PR учёта Codex
   в админке (codexstats); kdeps PR — тоже до release либо явное исключение.
   Обязательны и без них release 1.0.95 не выпускается: PR duckspeed и второй
   PR codexstats (таблица активных PTY-сессий: живые счётчики User/Asst/Context/Total,
   группировка сессий по пользователю).
6. Reconciliation PR → один release 1.0.95 → проверка GitHub Release, Docker
   image и ancestry → close-out.

### BR-12/13: что нужно от владельца deployment

BR-12 и BR-13 остаются `blocked`; у maintainer нет доступа к production deployment.

- BR-12: снять inventory новой сессии на действующем deployment — имена skills и источник каждого (deployment, CLI builtin, user, project, plugin), без содержимого.
- BR-12: сообщить image tag/digest и версию Claude Code этого deployment.
- BR-12: перечислить skills roots и mounts с владельцем (uid/gid), только пути и ownership.
- BR-13: для каждого deployment сделать backup legacy volume `bridge-sync`.
- BR-13: подтвердить migration marker и count/hash в `/app/data/bridge-sync`.
- BR-13: выполнить два restart после миграции и один rollback probe, сообщить результат.
- BR-13: подтвердить, что legacy mount больше не читается, и разрешить cleanup PR.

### Дополнение snapshot 2026-11-13: все открытые INC

Владелец уточнил «добить всё» по KaminIDE/Bridge, поэтому открытые INC из
исходного snapshot включены в пачку. Это дополнение того же snapshot
`5624a330`; входящие после него карточки остаются следующему запуску. Триаж —
по fresh `main` и GitHub-состоянию указанных PR. Класс не меняет статус
карточки: `done`/`resolved` ставится только после принятого результата.

| Класс | INC | Owner | Следующий artifact |
| --- | --- | --- | --- |
| fix merged, Windows live verify | 0002, 0023, 0025, 0051, 0053, 0055 | kmaint | Diagnostic PR с exact candidate и сценарием карточки |
| fix merged, Podman/Linux verify | 0015, 0019, 0062 | kmaint | isolated server/dashboard run; 0019 также Windows save/progress |
| fix merged, сверка по существующему evidence | 0016, 0078, 0079, 0084, 0088, 0089, 0090 | kmaint | reconciliation PR; повторять только непокрытые случаи |
| fix merged, только deployment observation | 0001, 0041, 0044, 0054, 0063, 0069, 0070, 0071, 0072, 0073, 0074, 0080, 0081, 0091, 0092, 0093 | владелец/лид (0001 — @ToToshka45) | наблюдение после выкладки 1.0.95; maintainer не выполняет |
| нужен fix: hooks/MCP, marketplace, Agent Teams | 0042 (change), 0077 (change), 0037, 0039, 0040, 0048 (research) | khooks | research → bounded Fix PR |
| нужен fix: webview/чат | 0031 + 0101 (общий механизм очереди), 0033, 0046, 0100 (research) | kchat | research → bounded Fix PR |
| нужен fix: sessions/resume | 0028, 0043, 0047, 0099, 0102 (research) | kses | research → bounded Fix PR |
| нужен fix: native sidebar, CEF/render, browser workspace | 0029, 0032, 0034, 0038, 0020, 0050, 0087, 0094 (research/change); 0064 только design | knative | research → bounded Fix PR; реализация 0064 после WPT по решению владельца |
| blocked: решение владельца | 0049 (receiving consumer), 0098 (render-only или CLI display mode) | лид спрашивает владельца | продолжение после решения |
| вне пачки: движок | 0085 (глубина стека float) | engine | не входит в Kamin/Bridge scope |

Дубликатов или неактуальных карточек триаж не нашёл. Карточки 0079, 0081, 0084,
0087–0092 и строка INC-0042 в обзоре отстают от уже смерженных PR; их приводит в
соответствие reconciliation PR kmaint без нового fix.

### Смена исполнителей 2026-11-13

Таблицы выше сохраняют исходное распределение. Для снижения нагрузки на машину
лид сократил параллельных исполнителей. Состав пачки и gates не меняются.

- Активны:
  - Claude: kmaint (verify, реестр, merge, release) и duckspeed;
  - Codex через драйвер: cxstats (учёт Codex в админке и таблица PTY-сессий,
    вместо codexstats), cxinc (INC-0108, INC-0107, INC-0042) и cxdeps
    ([#814](https://github.com/dykamino-coder/gpui-kamin-ide/pull/814):
    Codex CLI 0.167.2 и npm minor);
  - relayout10 работает на движке, вне пачки.
- Остановлены: kfix, kresearch, khooks, kchat, kses, knative, kdeps, toolspeed.
  Их незакоммиченные черновики остаются в своих seats.
- Неназначенные ID ждут свободного Codex-слота, очередь ведёт kmaint. Это
  остатки kfix/kresearch/khooks/kchat/kses/knative из таблиц выше.
  - Уже приняты: BR-21 decision
    ([#810](https://github.com/dykamino-coder/gpui-kamin-ide/pull/810)) и
    INC-0064 design
    ([#812](https://github.com/dykamino-coder/gpui-kamin-ide/pull/812)).
  - INC-0094 research — в
    [#813](https://github.com/dykamino-coder/gpui-kamin-ide/pull/813).
  - #797 и #804 ведёт лид.

### Вне пачки (backlog следующего запуска)

- INC-0085 — owner engine (см. выше).
- Engine PR (#712, #773, #791, #794, #798–#800, #807) и private #141 — работа движка.
- #797 и #804 ведёт лид; в пачку входят только как release prerequisite (шаг 5).

## Выборочная сверка после 1.0.80 — 2026-10-25

Scope: только INC-2026-0075/0076; source `origin/main`
`03b72b295248ccc78bcf27c1a16c5838e4e4df0b`. Это docs coordination после
merge/release, не новая runtime-пачка; прежние snapshot и gates сохраняются.

| ID / merged PR | State / artifact | Приёмка и следующий результат |
| --- | --- | --- |
| [INC-0075](runtime-issues/INC-2026-0075.md) / #412 | done / none | Merge `d2847fe7`; Windows candidate `409aa306`: update/uninstall только выбранной установки, foreign copy survives, relaunch. Доставлен `kaminide-v1.0.80`; обязательных gates и field observation по карточке не осталось |
| [INC-0076](runtime-issues/INC-2026-0076.md) / #413 | done / none | Merge `3e7c53de`; Windows candidate `d6080462`: normal start, overlap 24/21, recovery card, auto/manual relaunch. Доставлен `kaminide-v1.0.80`; обязательных gates и field observation по карточке не осталось |

Принятые implementation gates перечислены в
[#425 Verification](https://github.com/dykamino-coder/gpui-kamin-ide/pull/425#verification-and-limitations);
[Runtime acceptance](https://github.com/dykamino-coder/gpui-kamin-ide/pull/425#issuecomment-6388631189)
фиксирует smoke финального candidate `d4ca86b2` с деревом release commit выше.
[Immutable release](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.80)
проверен вместе с ancestry обоих merge SHA. Остатки общей installer-матрицы
и in-app published-update delivery из #425 не объявлены пройденными; они не
заменяют и не отменяют принятую task-specific приёмку. Следующий owner при
новой регрессии — maintainer с evidence на версии, содержащей fix.

## Завершённая пачка `RC-2026-11-03-MCP-RETENTION`

**Status:** complete. Публикация проверена, наблюдение после выкладки у владельца.

**Snapshot:** `origin/main` `f386ec9a952a87fc08b2abc0b9f59ae5ca34b33e`;
открытых PR на момент сверки нет. **Scope:** только server-only retention/overload
остаток INC-0080. Поручение владельца 2026-11-03 разрешает самому реализовать,
проверить, смержить и выпустить один coordinated release; deployment observation
выполнит владелец. Роль — maintainer. Соседние native/CEF, BR-02/03, HTML/WPT и
историческая атрибуция зависаний в snapshot не входят.

**Completion target:** решение о контракте → отдельный server fix с automated
acceptance → сверка → один проверенный release. Живой Windows smoke не заявляется;
обязательные CI и сборка installer сохраняются. История прежних пачек ниже неизменна.

| Deliverable | State / artifact | Gate и следующий шаг |
| --- | --- | --- |
| INC-0080 retention decision / #605 | done / none | [Контракт](server/MCP_TASK_RETENTION.md) принят в `b68458fa`: TTL, reservation, bounded ordinary fallback |
| INC-0080 server fix / #606 | done / none для retention source contract | Merge `d1206b79d2836d028090f16457315d3db14716c7`; candidate `84c97a095aef6ded7cef56ba20fa974f34779d87`: required CI, server/extension checks и Docker dry run passed. Local Docker: typecheck/lint и 651 tests passed / 6 existing skips; actual store/HTTP + delivery regressions. Клиент не меняется; native gate не требуется |
| Reconciliation / one release | done / none | [KaminIDE 1.0.85](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.85) / server 6.3.162, source `43d2bb31cda8c783dade905ee2d55e2cd9a4c42d`. Exact candidate, main quality и [publication](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/41629495504) прошли. Installer, provenance и Docker tags проверены |

**Остаток:** Task retention fix опубликован. Registry digest для `6.3.162` и
`latest` на момент проверки —
`sha256:626dca70d73c38bbffb8b5044bca6aa38dbe17a649fd8c84f33e055a31be33b8`.
Background retention остаётся конечным budget; при его заполнении
новая explicit background задача отклоняется до запуска, ordinary foreground
продолжает работать через bounded SSE path. Это не повтор старого глобального
admission outage. Windows UI и deployment не проверялись; владелец проверит
обычную работу после выкладки по checklist в INC-0080. Историческая причина
field hangs, native/CEF и BR-02/03 не закрыты этим результатом.

## История `RC-2026-10-30-INC82-83` — сверка завершена 2026-10-31

**Status:** complete — bounded source verification, public/private reconciliation
and handoff delivered; no new product fix or release required.
**Original snapshot:** public `origin/main`
`22b2b00b9f8b57ee441cbe8d6e2ba971dfac3c65`, coordination
[#553](https://github.com/dykamino-coder/gpui-kamin-ide/pull/553).
**Close-out source:** fresh public `origin/main`
`4867b416b43fc3e214eed075b2942ab7bb88ccb0`;
private `origin/main` `0975d4553b268353cbbd8c62f1562e21bbfc7dea`.

**Поручение 2026-10-30, продолжено 2026-10-31:** самому сверить INC-0082/0083
после параллельной работы, выполнить доступные серверные проверки, закрыть
только доказанный результат и оставить конкретный остаток/owner. Роль: maintainer
в этом scope. Windows и deployment не запускались. Отчёт о прежней native
приёмке #536 прочитан; author-local screenshot files независимо не проверены и
новым Windows pass не объявлены. Чужие ветки, PR и release не изменялись.

| Deliverable / merged PR | State / artifact | Принятый результат и оставшийся owner |
| --- | --- | --- |
| INC-0082 / #556; private #138 | done / none для terminal-count admission | #537/#539 доставлены в 1.0.82 / .159; 73 server tests, actual-limit 4,100 / 20 owners + 700 / owner, published .83 store repeat, restart/isolation/overload. Нет нового admission fix; INC-0080 ready decision по unread-result/TTL, deployment observation отдельно |
| INC-0083 / #564; private #139 | done / none для eager-history source contract | #536 опубликован в 1.0.82 / .159; paging/cursor/lazy/scanner tests, isolated 20-reader / 300-child workload. Exact Windows memory/response/queue research остаётся BR-02, BR-03 deferred; authenticated CLI compact observation у deployment owner |
| Concurrent delivery handoff / #555 | done / none | Read-only CI guard проверяет merged PR из явно заданной implementation branch против текущего Fix PR; 7 contract tests. Документация требует fresh-main reconciliation. Guard не определяет semantic ownership, не закрывает карточки и не подтверждает Windows |
| Batch close-out | done / coordination | Public/private PR merged; fetched main содержит проверенные карточки и пакеты, SHA-256/private backlinks сверены. Исходные evidence и старые snapshot/gates сохранены |

### Остаток: можно начинать без новых полевых логов

- **INC-0080 — ready / research-decision**, maintainer: определить peer-visible
  retention/overload contract по готовому pressure probe. При 257 terminal tasks
  одного owner непрочитанный результат может исчезнуть раньше advertised TTL.
  Это не прежняя блокировка admission. Первый artifact — server/CLI contract
  decision и матрица HTTP/store; fix только при обоснованном gap, без повторного
  исполнения side effects. В [карточке](runtime-issues/INC-2026-0080.md#task-result-retention-follow-up--2026-10-30)
  есть исходник, результат, варианты и acceptance; Windows для начала не нужен.
- **BR-02 — ready / research**, maintainer с Windows стендом: portable synthetic
  generator, compacted prefixes, large view + small control tab, 1/50/150/300
  children, pointer idle, switch/scroll/Console/reconnect; measured native/CEF/
  host memory, queue bytes/age и latency. Методика/thresholds до прогона;
  рабочий диапазон или лучший predictor вместо произвольного лимита MB.
  [Точный handoff](runtime-issues/INC-2026-0083.md#exact-remaining-artifact-not-another-field-log-request).
  **BR-03 deferred** до этих измерений, остальные native gates не сняты.
- **Deployment observations — not run**, deployment owner: обычная работа выше
  прежнего count threshold на .159+ и natural authenticated CLI compact с
  переходом index/current tail. Только versioned count/byte/stage metadata;
  без forced restart, удаления task files или обрезки canonical history.

Ни один остаток не требует снова реализовать #536/#537/#539 или ждать нового
исторического зависания для source/synthetic work. Исходная причина field crashes
и production capacity не установлены. Полный rewrite системы и шесть исходных
proposed slices не приняты как шесть обязательных багов. Related-task routing
не закрывает соседние incidents и не расширяет будущий snapshot автоматически.

### Доставка и свежесть

Первый опубликованный исправленный pair: **1.0.82 / server 6.3.159**;
последний проверенный immutable release на 2026-10-31: **1.0.84 / 6.3.161**.
Все восемь audited server modules совпадают по hash с tested source после новых
HTML/GPU/CI merge. Это source continuity, не Windows или production pass.
Пять собственных Docker test containers завершились exit 0 без OOM и удалены
по точным именам, без force/prune; owner resources не затронуты.
Diagnostic/process/CI changes этой пачки не требуют нового app/server release.

## История `RC-2026-10-20-COMPACT` — полный цикл двух source fixes

**Status:** complete — both bounded source fixes accepted, one publication verified; field observation is not run and belongs to the deployment owner. **Snapshot:** public `origin/main` `9c965ea8c7403c8a1d526ef53884c0812c8f1fd6`.
**Поручение 2026-10-20:** владелец поручил самому завершить fixes со скриншотов,
прояснить future completion/acceptance flow, выпустить один release и отдельно
проверить поведение после выкладки. Роль текущего агента — мейнтейнер; разрешены
commit/push/review/merge и один coordinated release. Доступа к deployment нет.

**Completion target:** оба ограниченных source-контракта приняты и находятся в
`main`; одна публикация проверена; карточки/обзор согласованы; field observation
имеет конкретный checklist, владельца и критерий переоткрытия. Публикация не
означает, что неизвестная причина исходного field события установлена.

| ID / PR snapshot | State / artifact | Gate и следующий результат |
| --- | --- | --- |
| INC-0067 / #377 | done / none | Source acceptance: candidate `a6bf138d`, merge `8f8d3bfa`; required CI, 175 host + 288 webview tests/builds. Deployment observation отдельно ниже |
| INC-0068 / #378 | done / none | Source acceptance: candidate `fbf7b270`, merge `84259a44`; required CI/dry run, 362 server tests + 5 skips, combined probe; same-inode regrowth limit сохраняется |
| Completion/acceptance documentation / #381 | done / none | Merge `3b7f27cf`: цель/полномочия/класс приёмки; исходные unperformed gates сохранены, CI не ослаблен |
| Один release / #383 | done / none | `kaminide-v1.0.77` / server 6.3.154; source `32d0badf`; exact candidate принят, main quality и publication прошли, installer/provenance/image/latest проверены |
| Field UX observation | ready / observation | Deployment owner после обычной выкладки обоих компонентов по [checklist](../../docs/COMPACT_RELEASE_OBSERVATION.md); `not run`, не новая очередь реализации |

Выполнен порядок: decision/coordination #381 → #377 → повторный fetch → #378 →
source close-out #382 → один release #383. Входящие чужие PR не включаются
автоматически. #380 (INC-0069,
retired subagent watcher) принят другим мейнтейнером в `9c965ea8` до этой
coordination: это уже исходный accepted source, а не новая задача этой пачки.
Пересечение с #378 по server watcher интегрировано без `ours`/`theirs`;
применимые tests повторены, фактическая доставка отмечена в release notes.
#302 и HTML/WPT не входят в поручение. Старый RV snapshot ниже
сохраняет исходные состав, ограничения и незакрытые Windows gates.

Доставка проверена: [quality run](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/39338087219),
[publication run](https://github.com/dykamino-coder/gpui-kamin-ide/actions/runs/39341597788),
[immutable release](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.77).
Source: `32d0badf496d5507809387fc2c0b66200fcfb5f0`; Docker 6.3.154/latest:
`sha256:31f1d2cb2fc61e15464bebc5df33baeb51f5bb4e2e311aa9a20cf3a77cfcc69e`. #380 доставлен как ранее
принятый source. При обновлении release branch сохранена параллельная регистрация
#384 / INC-0070/0071: это открытые исследования, не fixes этой пачки.
INC-0037, native BR-25/31 и исторические непройденные gates не закрыты.

## История `RV-2026-09-29` — остаток прежнего snapshot, приёмка открыта

На 2026-10-12 это последний **зафиксированный здесь** execution snapshot,
а не состав всей текущей очереди и не утверждение, что более поздних PR не было.
Октябрьские merged результаты и незакрытые INC учитываются в обзоре выше;
общий новый запуск оформляет новую coordination-пачку, сохраняя эту в истории.

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
- **На исходном snapshot:** INC-2026-0011, -0020, -0025, -0043, -0054 (P1 без PR решения) и 42
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
| [BR-04](RUNTIME_RELIABILITY.md#br-04--recover-webviews-after-extension-host-respawn) | ready | verify | exthost recovery | PR #148, #152 и #832 merged; Windows gate #832 (Chat/Agents) passed 2026-11-13; опубликовано в 1.0.95 | Respawn на установленном 1.0.95; документы/active editor/selection/LSP — отдельный child после уточнения |
| [BR-05](RUNTIME_RELIABILITY.md#br-05--rehydrate-authoritative-connection-state) | ready | verify | connection | код PR #14 merged | Остаток live gate: send/close-tab/server errors на exact-main server |
| [BR-06](RUNTIME_RELIABILITY.md#br-06--webview-update-stalls-until-pointer-activity) | ready | verify | rendering/connection | BR-31 implementation PR #147 merged; его R6 ещё открыт | Повторить оба исходных Chat no-pointer сценария и Plugins; сверить duplicate/остаток |
| [BR-07](RUNTIME_RELIABILITY.md#br-07--surface-native-claude-attention-in-chat) | waiting | change | native attention | BR-11 | Минимальный tab-scoped Console banner |
| [BR-08](RUNTIME_RELIABILITY.md#br-08--explain-unexpected-automatic-reload-skills) | waiting | observation | skills sync | telemetry PR #200 merged | Post-merge production observation of the diagnostic window (deployment owner) |
| [BR-09](RUNTIME_RELIABILITY.md#br-09--make-agent-teams-report-delivery-explicit) | done | none | Agent Teams | код PR #15 merged; 2026-11-13 оба случая passed на authenticated стенде (CLI 2.1.322, сервер 584b5616) | Закрыто решением lead 2026-11-13: серверный prompt contract не менялся с 584b5616 до `main`, клиент проверен на main-сборке |
| [BR-10](RUNTIME_RELIABILITY.md#br-10--show-the-effective-hook-in-approval-ui) | ready | verify | hooks | Fix PR #842 merged (`84f9f198`); Windows gate 2026-11-13 на `0d2863d6` passed (delivery в Chat, focus при показе, restore после очереди); опубликовано в 1.0.95 | Проверка на установленном 1.0.95, затем close |
| [BR-11](RUNTIME_RELIABILITY.md#br-11--inventory-native-cli-only-blocking-states) | ready | research | native attention | none | Versioned blocker inventory |
| [BR-12](RUNTIME_RELIABILITY.md#br-12--establish-the-server-and-builtin-skills-baseline) | blocked | verify | deployment skills | owner inventory действующего deployment | Versioned roots/mounts/ownership без contents |
| [BR-13](RUNTIME_RELIABILITY.md#br-13--retire-the-legacy-bridge-sync-mount-safely) | blocked | observation | deployment | owner confirms every deployment migration | Migration proof до cleanup PR |
| [BR-14](RUNTIME_RELIABILITY.md#br-14--make-release-artifact-provenance-verifiable) | done | observation | release | none | PR #43–#44 в `main`; первый автоматический release после добавления secrets |
| [BR-15](RUNTIME_RELIABILITY.md#br-15--evaluate-automatic-agent-teams-selection) | waiting | research | Agent Teams | BR-09 live gate + BR-25 completion; BR-27/26 done | Versioned orchestration eval |
| [BR-16](RUNTIME_RELIABILITY.md#br-16--stabilize-upward-history-scroll-anchoring) | waiting | change | chat history | classification BR-22 | Keyed-anchor PR с CEF displacement gate |
| [BR-17](RUNTIME_RELIABILITY.md#br-17--persist-privacy-safe-bridge-server-logs) | verify | observation | server operations | PR #203 merged | Production rollout of the lifecycle volume + read-only confirmation (deployment owner) |
| [BR-18](RUNTIME_RELIABILITY.md#br-18--keep-sessionend-relay-available-and-secret-safe-during-teardown) | done | observation | teardown | BR-17 для остаточной причины | PR #20 в `main`; trigger классифицируется по новым logs |
| [BR-18A](RUNTIME_RELIABILITY.md#br-18a--classify-local-sessionend-execution-on-explicit-end) | ready | verify | local hooks | BR-18 relay fix merged | Bounded local SessionEnd outcome/contract artifact; functional fix отдельно |
| [BR-19](RUNTIME_RELIABILITY.md#br-19--treat-shell-disconnect-as-lifecycle-cancellation-not-extension-crash) | ready | verify | lifecycle RPC | PR #146 merged; probe `hostDisconnect` PR #830 merged; reconnect/crash containment passed 2026-11-13 | Остаток: pending dialog/picker в момент разрыва и реальная ошибка запроса после reconnect |
| [BR-20](RUNTIME_RELIABILITY.md#br-20--restore-complete-claude-plan-usage-windows) | done | none | account usage | none | PR #202 in `main`; authenticated Podman CLI/dashboard/Account gate passed on exact head; follow-up #307 in `main` reuses BR-20 `parseStatusline` for the chat input limits row (no change to BR-20 scope or status) |
| [BR-21](RUNTIME_RELIABILITY.md#br-21--define-and-reconcile-dashboard-analytics-semantics) | ready | verify | analytics | Metric contract #810 merged; Codex accounting and live PTY counters/grouping #818 (includes #822) merged `a839d5a5`; container check 2026-11-13: signed-out Codex card and stats endpoints OK; опубликовано в 1.0.95 / server 6.3.172 | Live check on the 6.3.172 stand: Codex usage from a real GPT reply, PTY grouping by user; остальные BR-21 child по контракту |
| [BR-22](RUNTIME_RELIABILITY.md#br-22--keep-live-chat-render-window-populated-by-drawable-rows) | ready | verify | chat history | none для source/synthetic research; paired dumps для historical attribution | Drawable-row window fix merged в [#388](https://github.com/dykamino-coder/gpui-kamin-ide/pull/388) (INC-2026-0071, Windows gate passed); после релиза — field observation; BR-16 ждёт завершения классификации |
| [BR-23](RUNTIME_RELIABILITY.md#br-23--make-session-complete-notifications-transient-and-turn-scoped) | ready | verify | notifications | Change PR #834 merged (`5d95419f`); Windows gate 2026-11-13 на `94ebd71c` passed: countdown 8 s, автозакрытие, hover pause/resume, Open; реальное завершение в фоне тоже показывает transient notice (INC-2026-0113 закрыт как ошибка измерения); опубликовано в 1.0.95 | Live Agent Teams one-per-turn на установленном 1.0.95 |
| [BR-24](RUNTIME_RELIABILITY.md#br-24--bound-and-reconcile-lost-webview-invoke-replies) | waiting | change | invoke transport | diagnostics PR #201 merged; production observation + BR-31 R6 | Behavioral bounded invoke/reconciliation fix with Windows CEF gate |
| [BR-25](RUNTIME_RELIABILITY.md#br-25--verify-agents-view-delivery-and-rehydration-after-reveal) | waiting | verify | Agent Teams UI | BR-31 R6 ещё открыт; baseline + BR-27 + BR-26 done | Completion gate без repaint workaround после BR-31 R6 |
| [BR-26](RUNTIME_RELIABILITY.md#br-26--publish-agent-replay-state-atomically) | done | none | Agent Teams UI | none | Generation-scoped replay staging в `main`; BR-25 completion ждёт BR-31 |
| [BR-27](RUNTIME_RELIABILITY.md#br-27--derive-active-and-completed-from-one-lifecycle-partition) | done | none | Agent Teams UI | none | Partition + parser fix в `main`; BR-26 также done |
| [BR-28](RUNTIME_RELIABILITY.md#br-28--measure-sidebar-geometry-during-session-hover) | done | none | native sidebar | none | Paired artifact INC-2026-0003: sibling reflow не воспроизводится |
| [BR-29](RUNTIME_RELIABILITY.md#br-29--make-hover-to-rename-transition-atomic) | ready | verify | native sidebar | Fix PR #839 merged (`a60ef903`); Windows gate 2026-11-13 на `ff6cd87a` passed: Enter commit, Escape, Tab/Shift+Tab, возврат фокуса к строке; опубликовано в 1.0.95 | Scroll/смена session во время transition и bounds BR-28; затем проверка на установленном 1.0.95 |
| [BR-29A](RUNTIME_RELIABILITY.md#br-29a--keep-the-keyboard-focus-ring-visible-inside-clipped-containers) | done | none | native sidebar | none | Fix PR #853 merged (`93dddd3b`), Windows gate на `95ebe50f` passed; опубликовано в 1.0.96; 2026-11-13 проверка на установленном 1.0.96 passed (все chips и первая/четвёртая/последняя строка) |
| [BR-29B](RUNTIME_RELIABILITY.md#br-29b--scroll-the-session-list-to-the-keyboard-focused-row) | ready | change | native sidebar | none | Зарегистрировано 2026-11-13 (kmaint2): Tab-фокус на строке вне видимой части списка сессий не прокручивает список | Bounded shell fix с тестом, падающим на старом коде; исполнитель kmaint2 |
| [BR-30](RUNTIME_RELIABILITY.md#br-30--keep-incident-log-records-atomic-across-rotation) | done | none | diagnostics | none | PR #31 + #47; Windows 12/12 на main 686cc92; BR-02 разблокирован |
| [BR-31](RUNTIME_RELIABILITY.md#br-31--wake-the-webview-delivery-pump-on-host-posts) | ready | verify | CEF/webview delivery | PR #147 merged; R6 на 1.0.96: 74/75 accelerated reveals ≤ 1.1 с, в одном 4 сообщения ждали в outbox без pull | Per-view диагностика outbox/pull в `[views]`, повтор R6 до поимки, fix названного слоя (kmaint2) |

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

Ниже сохранены исторические направления выбора, **не актуальный список
неисправленного кода**. #146/#147/#148/#152/#200/#201/#202/#203 уже merged;
их текущий outcome — в реестре и обзоре. При общем запуске выбирать по ним,
а не повторять старые implementation/diagnostic PR из направлений ниже.

Они не входят в verification snapshot `RV-2026-09-06` и не запускаются автоматически:

- **B — Agent Teams completion + independent fixes:** BR-31 → BR-25 completion
  → BR-15 (после live gate BR-09); затем BR-19 → BR-17 → BR-20 → BR-23 → BR-04;
- **C — contracts and diagnostics:** INC-2026-0001, BR-21 decision, BR-24
  diagnostics, BR-18A, BR-11, BR-08, BR-06 (после BR-31) и BR-02;
- **conditional:** BR-21A–E, BR-07, BR-16, BR-03 и любой child из
  BR-22/BR-24/BR-28 только после их prerequisites;
- **owner-blocked:** BR-12 deployment inventory, BR-13 migration proof;
  historical BR-22 evidence collection не блокирует independent source research.

Перед продвижением следующей пачки maintainer повторно сверяет её с актуальным
`origin/main`, открытыми PR и новыми incidents. Продвижение оформляется
docs/process PR, чтобы новый snapshot был принят до выполнения его deliverables.
