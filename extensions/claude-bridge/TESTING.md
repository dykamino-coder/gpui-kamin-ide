# Claude Bridge testing

Этот документ описывает постоянный контур проверки Claude Bridge. Он не
привязан к конкретной ветке или уже завершённому PR.

## Automated validation

Из корня репозитория:

```bash
extensions/claude-bridge/verify-pr.sh --install
```

На повторном запуске с уже установленными зависимостями `--install` можно
опустить. Скрипт запускает применимые root, server, extension и webview
проверки, пересобирает committed runtime artifacts и проверяет, что build не
оставил незакоммиченный generated diff.

Если общий скрипт остановился на известной baseline-ошибке вне diff, PR всё
равно обязан запустить целевые проверки затронутого пакета и точно записать
ограничение. Красный результат нельзя описывать как успешный.

## Acceptance classes

Классы определены в корневом `CONTRIBUTING.md` и выбираются в шаблоне PR.

- Изменения чистой логики должны иметь automated merge gate.
- Изменения CEF/webview lifecycle, host respawn, focus/input, native TUI и
  session attach/reconnect требуют Windows runtime merge gate, если целевой
  пользовательский эффект заявлен исправленным.
- Диагностический PR может использовать post-merge production observation,
  если он только собирает данные и не заявляет устранение редкого сбоя.

## Базовая ручная матрица

Для пользовательского изменения выбираются целевой сценарий и соседние
инварианты из `plan/80-bridge-ux.md`. Минимально проверяются:

1. старт новой session и resume существующей;
2. переключение между session во время idle и активного assistant turn;
3. disconnect/reconnect без движения курсора и без смены focus;
4. Chat, Console и sidebar показывают согласованное состояние;
5. Send/Stop, queue, keyboard/focus и scroll сохраняют поведение;
6. закрытие и повторное открытие KaminIDE при продолжающейся server session;
7. соседние webview не удерживают или не обрабатывают неиспользуемый heavy
   stream.

## Plugin harness

Архитектурный контракт находится в `PLUGIN_HARNESS.md`. При изменении harness
дополнительно проверяются:

1. install/update/uninstall и rollback зависимостей;
2. независимые namespace двух plugins с совпадающими именами компонентов;
3. approval и token/session isolation hook relay;
4. MCP tools, resources, templates, prompts и pagination;
5. sensitive options без plaintext в settings, sync metadata и logs;
6. monitor/LSP lifecycle и cleanup;
7. новый или явно перезапущенный Claude CLI получает актуальные plugin roots.

Plugin-specific business logic и содержимое стороннего plugin repository не
являются частью Bridge PR.

### Corporate GitLab и marketplaces

Release/maintainer agent находится вне корпоративного контура: у него нет
сетевого и credential-доступа к internal GitLab, private marketplace repository
и plugin repositories, на которые ссылается этот marketplace. Он не вводит
реальный PAT, не импортирует Windows Credential Manager записи и не пытается
подключать корпоративный marketplace при merge/release.

Для изменения marketplace/plugin transport до merge остаются обязательными все
доступные проверки: unit tests, redacted fixtures `known_marketplaces.json` и
`marketplace.json`, disposable local Git/auth fixture, error/redaction paths и
Windows UI/runtime сценарии, которым не нужен корпоративный repository. Реальные
clone/pull/sync/install из corporate GitLab выполняет владелец доступа только
после обычной выкладки как non-blocking production observation.

Описание PR должно разделять эти два контура и перечислять: что maintainer уже
проверил, что физически недоступно, кто выполнит corporate observation, какой
результат ожидается и какое bounded/redacted evidence будет приложено. Нельзя
передавать в PR или агенту PAT, credential export, repository contents либо
необходимость подключения к корпоративной сети.

## Skills sync

Контракт и целевая матрица находятся в `SKILLS_SYNC.md`. Изменение sync или PTY
submission не принимается только по UI-скриншоту: обязательны tests на ordering,
coalescing, detach/reattach и exact snapshot semantics.

## Agent Teams

Изменение system prompt или team orchestration проверяется в двух слоях:

1. automated tests фиксируют injected reporting contract, порядок sections и
   отсутствие подмены user-provided instructions;
2. authenticated Windows live gate запускает не менее трёх named teammates с
   уникальными markers и проверяет доставку каждого bounded self-contained
   report через `SendMessage` до итогового ответа lead.

Уведомление `finished`/`idle` само по себе не считается report. Нельзя описывать
успешную повторную доставку после recovery request как успешную доставку с
первой попытки. Live gate отдельно фиксирует 3/3 first-attempt delivery и один
recovery request для искусственно потерянного report; во время работы выполняет
tab switch/reconnect. Автоматический выбор team для произвольно «большой» задачи
проверяется отдельным versioned model eval и не является бинарным UI smoke test.

## Hook approval UI

При изменении approval modal проверяются не только approve/reject actions, но и
качество informed consent: полный effective command с arguments, matcher,
danger indication, redaction secrets, отсутствие auto-selection неизвестного
hook, bounded overflow, copy/full-text access, hover, focus и keyboard. Тестовый
hook hash должен отличаться от ранее approved hash, иначе ожидаемый повторный
review не появится.

## Evidence

Для Windows runtime merge gate в PR прикладываются:

- версия KaminIDE, Bridge server и Claude Code;
- точные начальные условия и действия;
- ожидаемый и фактический результат;
- screenshot или короткая запись для UI;
- релевантные bounded logs без tokens, prompts и секретов.

Для post-merge production observation достаточно заранее определить, какой
сигнал будет собран, где он появится и кто подтвердит результат после выпуска.
Если observation зависит от corporate GitLab/marketplace, владельцем является
пользователь внутри корпоративного контура, а maintainer agent явно пропускает
этот шаг как недоступный, не помечая его passed или failed.

Release/CI PR дополнительно прикладывает source commit, image digest, revision
labels/attestation, installer asset version/digest и результат dry-run. Зелёный
job, который только пропустил build из-за существующего tag, не является
доказательством provenance.
