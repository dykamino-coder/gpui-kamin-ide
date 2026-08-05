# Проверка безопасного reload skills

Этот change PR исправляет гонку между пользовательским вводом в PTY и
автоматическим `/reload-skills`. Изменения не зависят от plugin harness и могут
проверяться локально одним checkout ветки PR.

## Что было не так

До исправления webview отправлял `Ctrl+U` и `session:submitText` двумя разными
WS-сообщениями, а sync route независимо вызывал `submitTextToSession` для
`/reload-skills`. Между этими операциями не было общей сериализации.

В результате reload мог:

- дописаться к незавершённой строке в bridge console;
- занять prompt между `Ctrl+U` и пользовательским submit;
- выполниться, пока session detached, а после reattach показать смешанный input;
- несколько раз подряд попасть в PTY после user/project sync burst.

Отдельно skills sync был добавочным: удалённый skill мог остаться в snapshot.
User skills symlink также позволял project overlay записать project-файл в
общий user snapshot.

## Новые инварианты

1. Server единолично выполняет `Ctrl+U → bracketed paste → Enter`.
2. В одной session одновременно активна только одна submit-транзакция.
3. Raw PTY input не вклинивается в активную транзакцию; `Ctrl+C` имеет приоритет
   и отменяет отложенный `Enter` и очередь submit.
4. Maintenance-команда coalesce-ится по ключу и запускается только когда session
   running, attached, prompt-ready и текущая raw-строка пуста.
5. Pending reload переживает detach/reattach и новую sync revision.
6. User/project skills образуют точный session-local snapshot: project имеет
   приоритет, удалённые пути исчезают, file/directory collision разрешается.
7. Skills upload и копирование skills при создании session сериализованы одним
   token-level lock, поэтому session не читает skills между remove и rewrite.
8. Отсутствующее поле `skills` сохраняет старый snapshot для совместимости со
   старыми partial-sync clients; присутствующее `{}` означает удалить все skills.

## Автоматическая проверка

Из корня репозитория:

```bash
npm ci
npm run check

npm --prefix extensions/claude-bridge/server ci
npm --prefix extensions/claude-bridge/server run typecheck
npm --prefix extensions/claude-bridge/server run lint
npm --prefix extensions/claude-bridge/server test
npm --prefix extensions/claude-bridge/server run format:check

npm --prefix extensions/claude-bridge/extension ci
npm --prefix extensions/claude-bridge/extension run typecheck
npm --prefix extensions/claude-bridge/extension run build

npm --prefix extensions/claude-bridge/webview ci
npm --prefix extensions/claude-bridge/webview run typecheck
npm --prefix extensions/claude-bridge/webview exec vitest run
npm --prefix extensions/claude-bridge/webview run build

python3 scripts/check_event_routing.py
git diff --check
```

Целевые тесты отдельно:

```bash
npm --prefix extensions/claude-bridge/server exec vitest run -- \
  src/core/pty/submit-text.test.ts \
  src/core/pty/input-coordinator.test.ts \
  src/core/pty/skills-snapshot.test.ts \
  src/core/sync/lock.test.ts

npm --prefix extensions/claude-bridge/webview exec vitest run -- \
  src/lib/send-message.test.ts \
  src/components/input-bar/useInputDraft.test.ts
```

Они покрывают transaction ordering, delayed echo, hard timeout, double submit,
raw-input buffering, `Ctrl+C`, detach/reattach, old-client split frames,
maintenance revision coalescing, exact overlay, token lock, semantic webview
submit и восстановление несохранённого chat draft без PTY side effect.

## Ручная матрица

### 1. Незавершённая строка bridge console

1. Запустить session и дождаться prompt.
2. В bridge console набрать `console-draft`, не нажимая Enter.
3. Уйти из интерфейса так, чтобы session стала detached.
4. Изменить skill и дождаться sync upload.
5. Вернуться в ту же session.

Ожидание: в строке остаётся только `console-draft`; `/reload-skills` не добавлен
и не выполнился. После отправки `console-draft` и следующего `Stop` pending reload
выполняется ровно как отдельная команда.

### 2. Draft в Cloud Bridge chat input

1. Ввести `chat-draft` в composer, не отправлять.
2. Переключить session или скрыть интерфейс.
3. Выполнить skills sync и вернуться.

Ожидание: `chat-draft` восстановлен только в composer, не появился в PTY и не
был отправлен. Кнопка Send отправляет ровно `chat-draft` одним semantic submit.

### 3. User/project sync burst

1. Изменить одновременно user и project skill.
2. Дождаться обоих uploads на idle session.

Ожидание: effective snapshot содержит обе версии с project override на
совпадающем пути. В PTY нет `/reload-skills/reload-skills`; выполняется одна
актуальная reload revision либо следующая revision остаётся pending, если первая
уже начала submit.

### 4. Удаление и collision

1. Синхронизировать user skill, убедиться, что он виден session.
2. Отправить `skills: {}` и дождаться безопасного reload.
3. Проверить collision: user содержит файл `collision`, project содержит
   `collision/SKILL.md`.

Ожидание: удалённого skill нет; `collision` в session является директорией с
project-версией. User snapshot не содержит project-only файлов.

### 5. Соседние старые механизмы

- Быстро отправить два сообщения: оба уходят отдельно и в исходном порядке.
- Нажать Stop во время задержанного paste echo: после `Ctrl+C` нет позднего Enter.
- Запустить `/compact` из header: команда выполняется один раз.
- Запустить auto-rename из native и webview sidebar: `/rename` выполняется один раз.
- Переключить session во время обычного assistant turn: queued user message и
  visual queue сохраняют прежнее поведение.

## Ограничения

Изменений визуального UI нет. Проверка настоящего CLI TUI требует локальной
авторизованной Claude Code session; unit tests используют fake PTY и fake timers.
Release-версии намеренно не меняются.

## Известные baseline-ограничения

На `origin/main` общие quality scripts сейчас не полностью зелёные независимо
от этого diff:

- root `npm run check` проходит `typecheck`, затем root `lint` останавливается
  на шести ошибках в неизменённых `src/exthost/api/webview-post-queue.ts` и
  `src/kamin-host/services/index.ts`; root `npm test` нужно запускать отдельно;
- server `npm run lint` сообщает, что glob `.` полностью ignored текущей
  ESLint-конфигурацией;
- server `npm run format:check` перечисляет 197 уже существующих файлов без
  Prettier-formatting.

Полный webview build также переписывает неизменённые `tools.html` и
`customize.html` из-за существующего generated-artifact drift. Они не входят в
этот PR. Целевой `chat.html` после удаления trailing end-of-line whitespace
воспроизводится с SHA-256
`691588c9f16fc965bf47c343eed5ca37f037d89bf06176a8fe86946c8b25ca9e`.
