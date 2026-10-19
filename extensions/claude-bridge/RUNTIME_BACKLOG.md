# Runtime backlog: что осталось и как продолжать

**Сверка доставки 2026-10-19:** public `origin/main`
`5790ee05ab3fd2b348db0f9fc7f160ca77f45b4b`; последний опубликованный immutable
release — KaminIDE **1.0.76 / server 6.3.153** (2026-10-19),
[#373](https://github.com/dykamino-coder/gpui-kamin-ide/pull/373).
HTML/WPT/cleanup commits сами по себе не означают миграцию production CEF.
Private evidence здесь не переаудировано целиком: metadata/link audit от
2026-10-12 и его базы сохранены ниже; для INC-0065 сверены merged #347,
immutable acceptance link и merged private #109. INC-0066 отдельно принят по
source/native-CLI контракту #369 и private #110; его доставка в 1.0.76 проверена
по immutable Release, installer provenance и Docker revision/digest.

На этом snapshot: **24 незавершённых BR из 32**, **34 незакрытых INC из 63**.
Приняты **8 BR**, **28 INC resolved** и **1 INC rejected** в своей доказанной области.
58 открытых записей — не 58 независимых багов: связи и prerequisites ниже.
INC-0065 приведён к уже принятому outcome #347; новых UI-прогонов эта сверка не заявляет.
BR-21A–E — планируемые children после decision, а не пять уже зарегистрированных задач.
Private storage IDs INC-0030/0035/0036 не создают дополнительные public задачи.

Это обзор маршрутов, не третий источник статусов и не новая execution-пачка.
Факты/acceptance — в [BR-карточках](RUNTIME_RELIABILITY.md) и
[INC-карточках](runtime-issues/); состояния BR, зависимости и snapshot выполнения —
в [исполняемом реестре](RUNTIME_EXECUTION.md). При следующем coordination/close-out
обновлять затронутые строки этого обзора вместе с canonical карточкой; дата/база
показывают предел актуальности. При расхождении сначала проверить merged PR и evidence.

## Выборочная сверка compact — 2026-10-20

Source до решения: `9c965ea8` (включает accepted fix INC-0069/#380); остальные строки
и численность выше относятся к датированному снимку 2026-10-19, а не к новому
полному аудиту. Пачка `RC-2026-10-20-COMPACT` выбрана владельцем только для
INC-0067/0068 и completion/acceptance flow. #380 / INC-0069 принят отдельно
другим мейнтейнером; исходный accepted source сохраняется при интеграции #378.

| INC | Текущее состояние | Следующий owner / artifact |
| --- | --- | --- |
| [INC-0067](runtime-issues/INC-2026-0067.md) | resolved bounded source contract; #377 merged `8f8d3bfa`, accepted CI/tests/builds; field не проверен | Maintainer: один release и Published fix; deployment owner: linked field checklist после выпуска |
| [INC-0068](runtime-issues/INC-2026-0068.md) | resolved bounded source contract; #378 merged `84259a44`, accepted CI/dry run/tests; field не проверен | Maintainer: один release и Published fix; deployment owner: linked field checklist; same-identity regrowth remains a limit |

Решение о классе сохраняет историческое требование и непроверенный field
outcome; native BR-25/31 и hook INC-0037 остаются открыты по своим карточкам.
Source outcome проверен в `origin/main` `84259a44`. После публикации добавить
проверенные delivery facts в эти строки и canonical cards отдельным docs PR.

## Как начать следующий проход

Поручение владельца «делай runtime-задачи по RUNTIME_BACKLOG.md и maintainer flow,
без работ над kamin-html/WPT» выбирает конечный свежий snapshot по
[общему запуску](../../docs/MAINTAINER_PR_FLOW.md#general-backlog-run).
Этот документ сам по себе задачи не запускает. `RV-2026-09-29` — прежняя
verification-пачка, не список всей октябрьской работы.

1. Fetch public/private, сверить все открытые BR/INC и PR; зафиксировать новую
   coordination-пачку с ID, current source/CLI versions, зависимостями и gates.
2. У `research` достаточно кода + существующего evidence для первого bounded
   source/synthetic deliverable. Проверить нынешний контракт; не требовать весь
   старый transcript/доступ на production. `verify` начинает с уже merged fix,
   его tests/acceptance и точного текущего кандидата, а не с повторной реализации.
3. Полный historical capture не обещан. Разделять: **можно исследовать сейчас**,
   **можно принять controlled результат**, **можно объяснить старый field incident**.
   Synthetic non-reproduction не закрывает историческую причину. Unknown остаётся
   явно unknown; future capture — только при естественном повторении и по private intake.
4. Если gate недоступен, записать конкретный сценарий/данные, владельца и условие
   продолжения. Это блокирует этот outcome, а не весь backlog. Research других
   независимых ID продолжается. Не заменять required Windows runtime gate CI.
5. После каждого результата/fetch пересмотреть зависимости внутри snapshot.
   В конце оставить по каждому остаточному ID actionable handoff, а не общий
   ответ «не хватает логов». Docs-only reconciliation release не вызывает.

Аудит 2026-10-12 (public `d54de40bfcd5621f155b870ca88b9af7ac74c8c2`,
private `25c6af31ebfc84e7f674330d4a9cdec78ffcddac`) проверил наличие private packages/manifest metadata и доступность 82
ссылок из карточек (80 по локальным Git objects, 2 через GitHub API). Это не
повторная проверка checksum каждого raw файла и не повторная runtime acceptance.
Выбранный evidence packet мейнтейнер проверяет по его provenance/SHA256SUMS.
Для старых INC-0046–0050, INC-0058 и INC-0062 новый private packet не является
предусловием исследования/принятия по их карточкам.

<a id="engine-boundary"></a>

## Runtime и HTML/WPT

На baseline выше shell использует **CEF 154 + GPUI**:
[shell dependencies](../../crates/shell/Cargo.toml),
[CEF lifecycle/delivery](../../crates/shell/src/web/mod.rs).
`kamin-html` объявлен в workspace и используется собственными examples/WPT runner;
production shell пока не зависит от него. [HTML crate](../../crates/html) и WPT
задачи ведутся отдельно. Улучшение CSS/WPT coverage не подтверждает исправление
Chat replay, scroll anchoring, invoke, IPC, hooks или session persistence.

[#216](https://github.com/dykamino-coder/gpui-kamin-ide/pull/216) уже обновил
GPUI/gpui-component и DirectX recovery. Native layout/editor/GPU сценарии сначала
перепроверять на свежей базе; #235 оставил явные Windows recovery gates.
Не переносить старую гипотезу на новый backend без проверки.

Снять задачу как superseded допустимо только после фактического перехода
затронутого production path, ссылки на migration PR и acceptance исходного
пользовательского сценария на новом path. Пока переход только планируется,
задача остаётся открытой; speculative CEF workaround не писать. Если её path
уже изменился, отдельный research outcome объясняет, что сохранилось и что исчезло.

## Открытые INC: маршрут, материал, остаток

Все ссылки ведут на handoff canonical карточки. `research` означает доступный
первый шаг, не готовность неизвестного fix к merge. `observation` требует
указанного владельца deployment/field и не создаёт новое исправление.

| ID | Маршрут | Что уже есть | Что делать / чего не хватает для закрытия |
| --- | --- | --- | --- |
| [INC-0001](runtime-issues/INC-2026-0001.md#maintainer-handoff) | `observation` | Private CPU captures; #187 (1.0.65), #261 (1.0.72) and follow-ups in 1.0.74 published; controlled Windows gates recorded. | Сравнить простой и активный Claude на одной актуальной RDP/WARP сборке с опубликованными #187/#261 и follow-ups 1.0.74; #261 gate уже принят, partial redraw отдельно INC-0064. **Граница:** Полевой A/B с одинаковой нагрузкой и renderer attribution; историческая единственная причина не доказана. **Owner:** field owner @ToToshka45. |
| [INC-0002](runtime-issues/INC-2026-0002.md#maintainer-handoff) | `verify` | Private reveal baseline; BR-31/#147 merged. | R6: reveal Agents без pointer/forced repaint; строки и invoke reply ≤ 1 с, затем BR-25 completion. **Граница:** Windows CEF R6 artifact на актуальном кандидате. **Owner:** maintainer/test operator. |
| [INC-0015](runtime-issues/INC-2026-0015.md#maintainer-handoff) | `verify` | Private source finding; #170 merged, deferred-response tests. | Изолированный dashboard: смена диапазона, старый ответ после нового, error/loading и unmount. **Граница:** Runtime outcome текущего dashboard; не повторять request-ownership fix. **Owner:** maintainer. |
| [INC-0016](runtime-issues/INC-2026-0016.md#maintainer-handoff) | `verify` | Private API finding; #169 merged with isolated DuckDB/API tests. | Сверить #169 с каждым пунктом acceptance: GET/DELETE identity, pages/filters, errors, nonmatching events/metrics и auth. **Граница:** Документированный acceptance outcome; повторить только непокрытые isolated cases. BR-21 decision отдельно. **Owner:** maintainer. |
| [INC-0019](runtime-issues/INC-2026-0019.md#maintainer-handoff) | `verify` | Private source finding; #171 merged, slow-socket tests. | Disposable Linux WS: slow consumer, cancellation/reconnect и lossless export; Windows save/progress. **Граница:** Linux WS/client outcome и Windows save/progress artifact. **Owner:** maintainer/test operator. |
| [INC-0020](runtime-issues/INC-2026-0020.md#maintainer-handoff) | `research` | Private crash/diag/incident/state bundle; renderer termination established. | На текущем CEF/GPUI построить карту OOM/device-loss/recovery, измерить isolated long-session и sleep/RDP сценарии. **Граница:** Для причины старого crash нет WER/dump и exact binary provenance; новый controlled failure может обосновать только свой bounded fix. **Owner:** maintainer; field owner only for historic attribution. |
| [INC-0023](runtime-issues/INC-2026-0023.md#maintainer-handoff) | `verify` | #216 merged; private controlled device-removal/effects recovery passed on 22fb1ad. | Проверить sleep/RDP с populated blur/effects и освобождение старых live COM objects. **Граница:** Оставшиеся Windows effects recovery cases; старые логи не нужны для controlled gate. **Owner:** maintainer with Windows runtime. |
| [INC-0025](runtime-issues/INC-2026-0025.md#maintainer-handoff) | `verify` | #216 merged; private repeated recovery with same/changed dimensions passed. | Проверить allocation failure/failed recovery, sleep/RDP и DPI transitions. **Граница:** Windows failure-injection/recovery artifact; успешный RemoveDevice не покрывает эти ветки. **Owner:** maintainer with Windows runtime. |
| [INC-0028](runtime-issues/INC-2026-0028.md#maintainer-handoff) | `research` | Private reports, logs, screenshot and state; current session-error handlers. | Fault injection startup/auth/admission/not-found; локализовать retry и проверить независимую соседнюю вкладку. **Граница:** Классифицированный terminal error/contract и воспроизводимый regression case до fix. **Owner:** maintainer. |
| [INC-0029](runtime-issues/INC-2026-0029.md#maintainer-handoff) | `research` | Private screenshots; native Projects layout; BR-28 bounded non-reproduction. | Измерить inter-group gaps на synthetic Projects разных размеров/viewport и текущем GPUI. **Граница:** Geometry before/after и owner layout path; BR-28 hover outcome не закрывает этот симптом. **Owner:** maintainer. |
| [INC-0031](runtime-issues/INC-2026-0031.md#maintainer-handoff) | `research` | Private screenshot; current queue/submit/JSONL mapping. | Synthetic queued item: submission → CLI consumption → pending removal, включая replay/late ack. **Граница:** Trace одного item и first divergent boundary; старый consumed-message payload отсутствует. **Owner:** maintainer. |
| [INC-0032](runtime-issues/INC-2026-0032.md#maintainer-handoff) | `research` | Private screenshot; native Projects overlay/scroll code. | Воспроизвести action-overlay open/close, wheel/hover и сохранение scroll position на synthetic tree. **Граница:** Windows geometry/input artifact при подтверждённом дефекте; rename fix не является доказательством. **Owner:** maintainer. |
| [INC-0033](runtime-issues/INC-2026-0033.md#maintainer-handoff) | `research` | Private screenshot only; collection plan #204; Markdown/normalization source. | Проверить valid synthetic prose/fences/backticks/table при streaming/final и разных pane widths. **Граница:** Нет исходных Markdown bytes для старого screenshot: его причину не классифицировать. Synthetic result отдельно; capture только при новом естественном случае. **Owner:** maintainer; reporter for a future bounded capture. |
| [INC-0034](runtime-issues/INC-2026-0034.md#maintainer-handoff) | `research` | Private screenshots; rename/title propagation source. | Synthetic CLI rename → server event → native Projects label; проверить ack, reconnect и две сессии. **Граница:** Versioned authoritative title contract и trace до первого расхождения. **Owner:** maintainer. |
| [INC-0037](runtime-issues/INC-2026-0037.md#maintainer-handoff) | `research` | Private screenshot + recovered related hook attachments/full redacted stderr; collection plan #205; hook source/relay/executor code. | Составить provenance map shipped/synced/user hooks и проверить generated compact-hook contracts на synthetic settings. **Граница:** Нет effective declaration/script owner и exact linkage старого screenshot; related exceptions восстановлены. Не исправлять неизвестный сторонний hook; bounded capture только при новом случае. **Owner:** maintainer; hook owner for declaration/exception. |
| [INC-0038](runtime-issues/INC-2026-0038.md#maintainer-handoff) | `research` | Private screenshot; native Projects scroll/layout source. | Synthetic sparse/dense Projects: content bounds, viewport, collapse/expand и пустой scroll tail. **Граница:** Measured geometry и current reproduction; не объявлять duplicate BR-28 без совпадения механизма. **Owner:** maintainer. |
| [INC-0039](runtime-issues/INC-2026-0039.md#maintainer-handoff) | `research` | Private server report, screenshot and JSON state; AgentTeams/report code. | Разделить delivered report, idle и next-action; synthetic lifecycle плюс current real-CLI trace в isolated account. **Граница:** Versioned report/idle contract; #197 status fix не доказывает управление/доставку отчётов. **Owner:** maintainer/test operator. |
| [INC-0040](runtime-issues/INC-2026-0040.md#maintainer-handoff) | `research` | Two private server research documents; public marketplace/tool source audit. | Inventory destructive entry points; disposable fake marker checkout для pull/auth/add/remove/no-.git/concurrency. **Граница:** Нет deletion command/ancestry, old bundle provenance и pre-loss inventory. Исторический инициатор unknown; не выбирать backup/re-clone fix по догадке. **Owner:** maintainer; reporter only for a future operation trace. |
| [INC-0041](runtime-issues/INC-2026-0041.md#maintainer-handoff) | `observation` | #158 published in 1.0.61; Windows runtime acceptance recorded; private package exists. | Owner сообщает redacted outcome background/manual refresh в действующей remote integration после обычной доставки. **Граница:** Deployment-only observation; не повторять accepted result/error propagation fix и не запрашивать credentials. **Owner:** deployment owner. |
| [INC-0042](runtime-issues/INC-2026-0042.md#maintainer-handoff) | `verify` | #302 open (conflicts, Windows gate pending); #304 AGENTS.md split published in 1.0.68. | Проверить оставшийся MCP diff #302 от свежей базы, затем synthetic plugin add/update + supported CLI: catalog convergence и bounded recovery; не начинать параллельный fix. **Граница:** Source-to-runtime trace и catalog acceptance; не расширять reconnect budgets без evidence. **Owner:** maintainer/test operator. |
| [INC-0043](runtime-issues/INC-2026-0043.md#maintainer-handoff) | `research` | Private screenshot, host/incident logs and session/layout JSON from manual restart. | Synthetic active CLI while client restarts: mirror → replay → Chat projection; найти first divergence, сохранить server activity. **Граница:** Correlated current replay/projection artifact; старый серверный trace/provenance неполон, truncation fix не предрешён. **Owner:** maintainer. |
| [INC-0044](runtime-issues/INC-2026-0044.md#maintainer-handoff) | `observation` | #198 merged, private exact-head Windows pointer-free startup acceptance passed. | После доставки owner подтверждает startup cache/installed/UI convergence в действующей remote integration. **Граница:** Deployment-only outcome; historical attribution unknown. Manual Update/no-pointer остаётся BR-06/31. **Owner:** deployment owner. |
| [INC-0046](runtime-issues/INC-2026-0046.md#maintainer-handoff) | `research` | Historical sanitized note and submit/queue source; original packet absent. | Synthetic new-session/agent startup с immediate follow-up; trace ready/submit/queued/CLI accepted ordering. **Граница:** Текущая reproduction; поздний fix требует automated ordering + real CLI/Windows. Старый transcript не prerequisite. **Owner:** maintainer. |
| [INC-0047](runtime-issues/INC-2026-0047.md#maintainer-handoff) | `research` | Historical sanitized note; current rewind/tail/replay/projection source. | Установить current pinned CLI rewind/branch protocol, затем synthetic history transition/replay. **Граница:** Versioned branch contract + differential case; fix требует exact Windows Chat/Console gate. **Owner:** maintainer. |
| [INC-0048](runtime-issues/INC-2026-0048.md#maintainer-handoff) | `research` | Historical sanitized note; #158/#198 contracts and startup/marketplace source. | Fake Git provider: delay/auth/network/failure, session startup и независимые вкладки с auto-update on/off. **Граница:** Trace causal startup blocker или bounded non-reproduction; remote integration только owner observation. **Owner:** maintainer. |
| [INC-0049](runtime-issues/INC-2026-0049.md#maintainer-handoff) | `decision` | Historical sanitized note; identity source map and clarification plan #206. | Описать поддержанные consumers: conversation resume, live PTY attach, server directory; проверить reachable UI/API. **Граница:** Reporter выбирает intended receiving operation до UX fix. Не нужны реальные ID/папки/токены. **Owner:** maintainer for map; reporter for consumer decision. |
| [INC-0050](runtime-issues/INC-2026-0050.md#maintainer-handoff) | `research` | Historical sanitized note; CEF key/focus routing, DOM and native focus source. | Synthetic embedded controls/modal: Tab/Shift+Tab, host focus и DOM traversal; сверить already fixed transport. **Граница:** Точный affected element/current failure; заявленный fix требует Windows focus/keyboard gate. **Owner:** maintainer. |
| [INC-0051](runtime-issues/INC-2026-0051.md#maintainer-handoff) | `verify` | #172 merged, Markdown/link regression tests; private screenshot. | Valid synthetic emphasis/autolink/punctuation cases → DOM href → actual Windows link activation. **Граница:** Exact Windows GPUI/CEF activation artifact; no old private URL needed. **Owner:** maintainer with Windows runtime. |
| [INC-0053](runtime-issues/INC-2026-0053.md#maintainer-handoff) | `verify` | #160 published in 1.0.61; #303 live model/effort follow-up in 1.0.68 (isolated CLI 2.1.296, no Windows CEF acceptance); private screenshots and tests. | Current pinned CLI: accept/reject/pending, idle/stream, Console hidden, reconnect/respawn before next response и token isolation. **Граница:** Real CLI + Windows artifact. Pending prompt in Chat и isolation исследовать bounded, не переключать старые сессии насильно. **Owner:** maintainer/test operator with isolated test account. |
| [INC-0054](runtime-issues/INC-2026-0054.md#maintainer-handoff) | `observation` | #159 published in 1.0.61; private Windows resume/expiry acceptance recorded. | Read-only effective cleanupPeriodDays, mounts/storage и file metadata на deployment. **Граница:** Field deletion attribution/retention observation; accepted resume behavior отдельно, не считать DB indexing причиной. **Owner:** deployment owner. |
| [INC-0055](runtime-issues/INC-2026-0055.md#maintainer-handoff) | `verify` | #168 merged; private diagnostic ZIP and IPC/queue tests. | Controlled Windows IPC send failure во время stream/subagents: process lifetime, queue depth, outcomes/recovery/disconnection. **Граница:** Windows acceptance no false delivery/unresolved posts; old write UNKNOWN root cause remains unknown. Recovery policy отдельный bounded child. **Owner:** maintainer with Windows runtime. |
| [INC-0062](runtime-issues/INC-2026-0062.md#maintainer-handoff) | `verify` | #233 merged; PR records unit/CI and Podman TLS matrix, no Windows GUI gate. | Сверить accepted policy и #233 evidence с baseline 502, default success, verified reject/root success и CLI CA bundle. **Граница:** Оформить remaining acceptance/closure decision по существующему evidence, повторять только uncovered cases; deployment observation отдельно. **Owner:** maintainer; deployment owner for production observation. |
| [INC-0063](runtime-issues/INC-2026-0063.md#accepted-delivery--2026-10-19-reconciliation) | `observation` | #331 accepted and published in 1.0.73; render-model replay and rebuilt chat bundle. | Owner compares narration between tool groups in Chat/Console on a deployed build containing #331. No missing raw transcript blocks another implementation: the implementation is already accepted. **Owner:** @dykamino-coder. |
| [INC-0064](runtime-issues/INC-2026-0064.md) | `research` | Source-confirmed full-window painting; #261/1.0.74 cost reductions do not implement partial redraw. | Design damage tracking before implementation. Implementation waits for the owner's WPT prerequisite; research does not close the incident. **Gate:** Windows WARP/GPU painted-area and visual acceptance. **Owner:** maintainer, scheduling decision by owner. |

## Открытые BR: зависимости и следующий результат

Состояния берутся из исполняемого реестра. Gate владельца не является
причиной запускать новые fixes. Один R6 artifact может быть связан с BR-31,
INC-0002 и completion BR-25; outcome каждого acceptance записать отдельно.

| ID | Маршрут | Следующий результат | Условие закрытия/продолжения |
| --- | --- | --- | --- |
| [BR-02](RUNTIME_RELIABILITY.md#br-02--calibrate-the-long-session-memory-envelope) | `research` | Source + private long-session facts; repeated synthetic memory envelope. | Нет current Windows envelope/decision; BR-03 ждёт решение, не выдумывать лимит. |
| [BR-03](RUNTIME_RELIABILITY.md#br-03--long-session-mitigation) | `waiting` | Mitigation только после BR-02. | Envelope decision и bounded выбранный механизм. |
| [BR-04](RUNTIME_RELIABILITY.md#br-04--recover-webviews-after-extension-host-respawn) | `verify` | #148/#152 merged: Windows extension-host respawn, Chat/Console/neighbor view и documents. | Windows artifact; active editor/selection/LSP только отдельный child после contract decision. |
| [BR-05](RUNTIME_RELIABILITY.md#br-05--rehydrate-authoritative-connection-state) | `verify` | #14 merged: isolated exact-main server send/recovery, close-tab и server errors. | Authenticated Windows/Linux gate; production credentials не требуются. |
| [BR-06](RUNTIME_RELIABILITY.md#br-06--webview-update-stalls-until-pointer-activity) | `verify` | #147 delivery wake reused: оба Chat no-pointer сценария + Plugins manual Update. | Windows no-pointer outcome; сопоставить BR-31 R6, не добавлять forced repaint. |
| [BR-07](RUNTIME_RELIABILITY.md#br-07--surface-native-claude-attention-in-chat) | `waiting` | Tab-scoped Console banner после blocker inventory BR-11. | Versioned native attention contract. |
| [BR-08](RUNTIME_RELIABILITY.md#br-08--explain-unexpected-automatic-reload-skills) | `observation` | #200 telemetry accepted; owner reports diagnostic window after deployment. | Real reload event/source; maintainer не имеет deployment access. |
| [BR-09](RUNTIME_RELIABILITY.md#br-09--make-agent-teams-report-delivery-explicit) | `verify` | #15 merged: 3/3 reports без recovery и отдельный один bounded recovery. | Current real CLI + isolated authenticated Windows/Linux evidence; отличается от #197 status. |
| [BR-10](RUNTIME_RELIABILITY.md#br-10--show-the-effective-hook-in-approval-ui) | `verify` | #13 merged: modal focus, Tab/Shift+Tab, approve/reject и соседние controls. | Windows keyboard/focus artifact. |
| [BR-11](RUNTIME_RELIABILITY.md#br-11--inventory-native-cli-only-blocking-states) | `research` | Current pinned CLI-only blockers inventory (включая model confirmation). | Versioned trigger/output/actions; старые production transcripts не prerequisite. |
| [BR-12](RUNTIME_RELIABILITY.md#br-12--establish-the-server-and-builtin-skills-baseline) | `observation` | Owner inventory roots/mounts/ownership действующего deployment, без contents. | Deployment-owner report; нельзя подменить local synthetic inventory. |
| [BR-13](RUNTIME_RELIABILITY.md#br-13--retire-the-legacy-bridge-sync-mount-safely) | `observation` | Owner confirms every deployment migrated before legacy cleanup. | Migration proof; до него destructive cleanup запрещён. |
| [BR-15](RUNTIME_RELIABILITY.md#br-15--evaluate-automatic-agent-teams-selection) | `waiting` | Orchestration eval после BR-09 и BR-25 completion. | Live report gate + delivery completion; BR-26/27 уже done. |
| [BR-16](RUNTIME_RELIABILITY.md#br-16--stabilize-upward-history-scroll-anchoring) | `waiting` | Upward lazy-load anchor + streaming user-scroll follow policy после BR-22 classification. | Classified history path; keyed displacement Windows CEF gate, не WPT scrollbar result. |
| [BR-17](RUNTIME_RELIABILITY.md#br-17--persist-privacy-safe-bridge-server-logs) | `observation` | #203 durable lifecycle logging merged; owner rollout/read-only confirmation. | Effective log volume/lifecycle fields на deployment. |
| [BR-18A](RUNTIME_RELIABILITY.md#br-18a--classify-local-sessionend-execution-on-explicit-end) | `verify` | Explicit end SessionEnd contract в isolated authenticated CLI account. | Bounded Windows/Linux hook outcome; не end/kill чужую production session. |
| [BR-19](RUNTIME_RELIABILITY.md#br-19--treat-shell-disconnect-as-lifecycle-cancellation-not-extension-crash) | `verify` | #146 merged: Windows shell disconnect/reconnect без false Extension crashed. | Task-specific lifecycle artifact, а не только build CI. |
| [BR-21](RUNTIME_RELIABILITY.md#br-21--define-and-reconcile-dashboard-analytics-semantics) | `research` | Metric semantics decision из source/synthetic SQL; #169 API и #202 quota не заменяют её. | Identity/range/freshness/model/price decision; только потом BR-21A–E. |
| [BR-22](RUNTIME_RELIABILITY.md#br-22--keep-live-chat-render-window-populated-by-drawable-rows) | `research` | Private screenshots + current predicate/window/projection source; synthetic long history/question/stream matrix. | Нет paired historic drawable dumps. Сначала классифицировать current failing boundary; BR-16/change gate сохраняется. |
| [BR-23](RUNTIME_RELIABILITY.md#br-23--make-session-complete-notifications-transient-and-turn-scoped) | `change` | #161/#164 merged; bounded transient host toast contract и tests. | Current manual-dismiss toast недостаточен; fake-clock + exact Windows native toast acceptance. |
| [BR-24](RUNTIME_RELIABILITY.md#br-24--bound-and-reconcile-lost-webview-invoke-replies) | `waiting` | Behavioral fix ждёт classified production observation + BR-31 R6; #201 diagnostics merged. | Source/synthetic timeout/late-ack countercheck доступен сейчас отдельно; он не снимает prerequisites fix. |
| [BR-25](RUNTIME_RELIABILITY.md#br-25--verify-agents-view-delivery-and-rehydration-after-reveal) | `waiting` | Baseline done, #147 merged; completion без repaint workaround после BR-31 R6. | Windows Agents delivery/reveal completion artifact. |
| [BR-29](RUNTIME_RELIABILITY.md#br-29--make-hover-to-rename-transition-atomic) | `verify` | #37 merged: actual typing/F2/flyout/double-click/Enter/Esc/blur/hitbox. | Windows focus/keyboard gate; geometric probe сам по себе недостаточен. |
| [BR-31](RUNTIME_RELIABILITY.md#br-31--wake-the-webview-delivery-pump-on-host-posts) | `verify` | #147 merged; тот же R6, что INC-0002, затем BR-25/06. | Reveal no-pointer: rows + invoke ≤ 1 с; два separate fixes не нужны. |

## Принятые результаты: не реализовывать повторно

Закрытый outcome ограничен acceptance карточки. `Resolved` и `done` не
доказывают устранение всех соседних field симптомов и не заменяют release.
Новая воспроизводимая регрессия требует evidence на build, содержащем fix.

| INC | Принятый результат | Доставка на snapshot |
| --- | --- | --- |
| [INC-0003](runtime-issues/INC-2026-0003.md) | rejected: sibling reflow not reproduced; no fix | N/A (diagnostic) |
| [INC-0004](runtime-issues/INC-2026-0004.md) | resolved; [#184](https://github.com/dykamino-coder/gpui-kamin-ide/pull/184) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0005](runtime-issues/INC-2026-0005.md) | resolved; [#154](https://github.com/dykamino-coder/gpui-kamin-ide/pull/154) | published [1.0.61](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.61) |
| [INC-0006](runtime-issues/INC-2026-0006.md) | resolved; [#155](https://github.com/dykamino-coder/gpui-kamin-ide/pull/155) | published [1.0.61](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.61) |
| [INC-0007](runtime-issues/INC-2026-0007.md) | resolved; [#190](https://github.com/dykamino-coder/gpui-kamin-ide/pull/190) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0008](runtime-issues/INC-2026-0008.md) | resolved; [#191](https://github.com/dykamino-coder/gpui-kamin-ide/pull/191) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0009](runtime-issues/INC-2026-0009.md) | resolved; [#192](https://github.com/dykamino-coder/gpui-kamin-ide/pull/192) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0010](runtime-issues/INC-2026-0010.md) | resolved; [#193](https://github.com/dykamino-coder/gpui-kamin-ide/pull/193) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0011](runtime-issues/INC-2026-0011.md) | resolved; [#153](https://github.com/dykamino-coder/gpui-kamin-ide/pull/153) | published [1.0.61](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.61) |
| [INC-0012](runtime-issues/INC-2026-0012.md) | resolved; [#194](https://github.com/dykamino-coder/gpui-kamin-ide/pull/194) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0013](runtime-issues/INC-2026-0013.md) | resolved; [#195](https://github.com/dykamino-coder/gpui-kamin-ide/pull/195) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0014](runtime-issues/INC-2026-0014.md) | resolved; [#188](https://github.com/dykamino-coder/gpui-kamin-ide/pull/188) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0017](runtime-issues/INC-2026-0017.md) | resolved; [#183](https://github.com/dykamino-coder/gpui-kamin-ide/pull/183) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0018](runtime-issues/INC-2026-0018.md) | resolved; [#196](https://github.com/dykamino-coder/gpui-kamin-ide/pull/196) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0021](runtime-issues/INC-2026-0021.md) | resolved; [#185](https://github.com/dykamino-coder/gpui-kamin-ide/pull/185) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0022](runtime-issues/INC-2026-0022.md) | resolved; [#186](https://github.com/dykamino-coder/gpui-kamin-ide/pull/186) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0024](runtime-issues/INC-2026-0024.md) | resolved; [#182](https://github.com/dykamino-coder/gpui-kamin-ide/pull/182) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0026](runtime-issues/INC-2026-0026.md) | resolved; [#157](https://github.com/dykamino-coder/gpui-kamin-ide/pull/157) | published [1.0.61](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.61) |
| [INC-0027](runtime-issues/INC-2026-0027.md) | resolved; [#156](https://github.com/dykamino-coder/gpui-kamin-ide/pull/156) | published [1.0.61](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.61) |
| [INC-0045](runtime-issues/INC-2026-0045.md) | resolved; [#199](https://github.com/dykamino-coder/gpui-kamin-ide/pull/199) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0052](runtime-issues/INC-2026-0052.md) | resolved; [#117](https://github.com/dykamino-coder/gpui-kamin-ide/pull/117) | published [1.0.56](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.56) |
| [INC-0056](runtime-issues/INC-2026-0056.md) | resolved; [#197](https://github.com/dykamino-coder/gpui-kamin-ide/pull/197) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0057](runtime-issues/INC-2026-0057.md) | resolved; [#189](https://github.com/dykamino-coder/gpui-kamin-ide/pull/189) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0058](runtime-issues/INC-2026-0058.md) | resolved; [#181](https://github.com/dykamino-coder/gpui-kamin-ide/pull/181) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0059](runtime-issues/INC-2026-0059.md) | resolved; [#211](https://github.com/dykamino-coder/gpui-kamin-ide/pull/211) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0060](runtime-issues/INC-2026-0060.md) | resolved; [#212](https://github.com/dykamino-coder/gpui-kamin-ide/pull/212) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0061](runtime-issues/INC-2026-0061.md) | resolved; [#213](https://github.com/dykamino-coder/gpui-kamin-ide/pull/213) | published [1.0.65](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.65) |
| [INC-0065](runtime-issues/INC-2026-0065.md#accepted-outcome--2026-10-19-reconciliation) | resolved; [#347](https://github.com/dykamino-coder/gpui-kamin-ide/pull/347), exact-head 8/8 GPU + 8/8 WARP accepted | published [1.0.75](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.75) |
| [INC-0066](runtime-issues/INC-2026-0066.md#accepted-result--2026-10-19) | resolved; [#369](https://github.com/dykamino-coder/gpui-kamin-ide/pull/369), generated model pin and runtime instructions; automated + isolated native CLI acceptance | published [1.0.76](https://github.com/dykamino-coder/gpui-kamin-ide/releases/tag/kaminide-v1.0.76) |

BR `done`: [BR-01](RUNTIME_RELIABILITY.md#br-01--durable-incident-diagnostics), [BR-14](RUNTIME_RELIABILITY.md#br-14--make-release-artifact-provenance-verifiable), [BR-18](RUNTIME_RELIABILITY.md#br-18--keep-sessionend-relay-available-and-secret-safe-during-teardown), [BR-20](RUNTIME_RELIABILITY.md#br-20--restore-complete-claude-plan-usage-windows), [BR-26](RUNTIME_RELIABILITY.md#br-26--publish-agent-replay-state-atomically), [BR-27](RUNTIME_RELIABILITY.md#br-27--derive-active-and-completed-from-one-lifecycle-partition), [BR-28](RUNTIME_RELIABILITY.md#br-28--measure-sidebar-geometry-during-session-hover), [BR-30](RUNTIME_RELIABILITY.md#br-30--keep-incident-log-records-atomic-across-rotation). Их остаточные observations и соседние child tasks читаются по карточкам; первичный fix не повторять.

## Повторный разбор локальных материалов — 2026-10-12

Public source baseline для этого дополнения: `210af97f`; private база до
supplements: `25c6af31`. Private supplements приняты в
[#102](https://github.com/dykamino-coder/gpui-kamin-ide-priv-evidence/pull/102) и
[#103](https://github.com/dykamino-coder/gpui-kamin-ide-priv-evidence/pull/103);
immutable commits закреплены в карточках. Это уточнение открытых постановок, без новой пачки,
продуктовых fixes, runtime acceptance или release.

| Проверенная область | Что изменилось в handoff |
| --- | --- |
| [INC-0037](runtime-issues/INC-2026-0037.md#recovered-local-context--2026-10-12) | В двух retained transcripts восстановлены семь related compact-hook errors и полный redacted stderr: MODULE_NOT_FOUND для entry script. Не запрашивать этот stderr заново; остаётся declaration/install owner и привязка к старому screenshot. |
| [INC-0020](runtime-issues/INC-2026-0020.md#older-local-generations-recovered--2026-10-12) | Добавлены более ранние cumulative OOM callbacks; два log поколения имеют общий prefix, поэтому не считать их разными событиями. Они не доказывают memory cause или связь с sleep/RDP. |
| [INC-0033](runtime-issues/INC-2026-0033.md#retained-source-countercheck--2026-10-12) / [BR-22](RUNTIME_RELIABILITY.md#br-22--keep-live-chat-render-window-populated-by-drawable-rows) | В проверенных roots не найден original Markdown / paired drawable dump для старых screenshots. Другой transcript не заменяет их. Source/synthetic classification доступен сейчас; historical attribution остаётся ограниченной. |
| [INC-0040](runtime-issues/INC-2026-0040.md#retained-source-countercheck--2026-10-12) | Два original reports уже зарегистрированы; новых deletion/process traces не найдено. Доступен disposable marker matrix текущих destructive/non-destructive entry points. |
| [INC-0043](runtime-issues/INC-2026-0043.md#retained-source-countercheck--2026-10-12) | Re-read оригинального export подтверждает прежнюю границу: отсутствует поздний crash/reopen interval. Нужен synthetic same-time server/replay/mirror/store comparison, а не повторная отправка того же файла. |

Проверены fingerprint/provenance локальных копий и существующие private packets,
включая содержимое выбранных ZIP/gzip. Совпадение checksum означает тот же artifact,
а не новое доказательство причины. Полные transcripts/profile archives могут быть
намеренно исключены ради credentials/privacy; использовать safe structural extracts.
Диагностические folders — не отдельная очередь: их старые TASK-номера сопоставлены
с canonical BR/INC, их локальные статусы не отменяют merged acceptance.

Перед новым fix сверять уже принятый результат: INC-0056/#197 (persistent follow-up),
INC-0057/#189 (MCP result recovery), BR-18 (relay credential/teardown) не повторять
ради соседних неразобранных симптомов. INC-0055/0039, BR-09/15/25 сохраняют свои
отдельные gates. Большой общий transcript сам по себе не доказывает общий дефект.
Новые native panel PR и изменения GPUI проверять от свежего origin, не принимать
параллельную незавершённую работу как закрытие задачи.

Итог следующего research по каждой карточке: source boundary + reproducible
synthetic case либо bounded non-reproduction; затем конкретный остаточный gate,
owner и условие продолжения. Отсутствие старого field artifact не заменять
придуманной причиной, а доступную source работу — общим запросом «нужны логи».

## Существенные пробелы

- **Историческая атрибуция:** INC-0020 (dump/binary), INC-0033 (original Markdown),
  INC-0037 (effective declaration/script owner и exact screenshot linkage; related stderr теперь есть), INC-0040 (deleting operation), BR-22 (paired drawable dumps).
  Имеющееся evidence сохраняется; не просить заново весь архив, не обещать его восстановить.
  Независимые current-source/synthetic исследования перечислены выше.
- **Product input:** INC-0049 — receiving operation, не дополнительные логи.
- **Field/deployment owner:** INC-0001/0041/0044/0054 и BR-08/12/13/17.
  У maintainer нет deployment access; эти observations не выполнять от его имени.
- **Runtime acceptance:** остальные `verify` требуют только конкретного gate карточки.
  Если Windows/test account отсутствует, сохранить ID, exact candidate и непроведённый
  сценарий; продолжить независимый research, не объявлять всю очередь исчерпанной.

Сверкой **2026-10-12** ни одна карточка не закрыта и ни одна гипотеза не объявлена причиной.
Для следующего прохода достаточен стабильный общий prompt выше; новые задачи
регистрируются обычным flow и попадут в следующий finite snapshot без изменения команды.
