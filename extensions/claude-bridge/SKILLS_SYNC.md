# Skills sync and automatic reload

## Runtime contract

User/project skills и plugin roots имеют разный lifecycle.

- Отсутствующее поле `skills` в sync request сохраняет прежний snapshot.
- Присутствующее поле, включая `{}`, полностью заменяет соответствующий
  user/project snapshot.
- Project snapshot перекрывает user snapshot только внутри session-local
  overlay; исходный user snapshot не мутируется.
- Plugin snapshot materialized отдельно и передаётся Claude CLI через
  `--plugin-dir`. Его изменение требует новой или явно перезапущенной CLI
  session.

Server вызывает автоматический `/reload-skills` только после фактически
изменившегося user/project skills snapshot. Одинаковый повторный upload не
создаёт reload. Простое переключение tab/session само по себе reload не
планирует.

Pending reload может стать видимым после attach/reconnect: coordinator ждёт
running, attached, prompt-ready session с пустой raw-строкой. Поэтому появление
команды сразу после возврата в tab ещё не доказывает, что tab switch был её
источником; причиной мог быть ранее отложенный changed sync.

## PTY invariants

1. Server выполняет `Ctrl+U → bracketed paste → Enter` одной транзакцией.
2. В session одновременно активна только одна semantic submit-транзакция.
3. Raw input не вклинивается в активную транзакцию; `Ctrl+C` имеет приоритет.
4. Maintenance-команда coalesce-ится по ключу и revision.
5. Pending reload переживает detach/reattach и выполняется только на безопасной
   границе prompt.
6. Skills upload и построение session overlay сериализованы token-level lock.

Эти инварианты запрещено ослаблять ради скрытия `/reload-skills` в UI. Server
MCP tools не являются источником этой maintenance-команды.

## Unresolved observation

Полевое наблюдение: пользователь иногда видит `/reload-skills` после открытия
старой session или reconnect, не инициируя команду вручную. Текущий код
объясняет выполнение ранее pending reload, но имеющихся logs недостаточно,
чтобы отличить его от неожиданного changed upload клиента.

До отдельного исследования нельзя:

- удалять автоматический reload;
- считать каждый видимый reload лишним;
- утверждать, что tab switch непосредственно отправляет команду.

Нужна bounded telemetry без содержимого skills: token/session pseudonymous id,
source (`user`/`project`), snapshot revision, `changed`, время постановки в
очередь и причина фактического submit. После этого сценарий проверяется на
Windows при tab switch, reconnect и длительном idle.

## Automated checks

```bash
npm --prefix extensions/claude-bridge/server exec vitest run -- \
  src/core/pty/input-coordinator.test.ts \
  src/core/pty/session-settings-skills.test.ts \
  src/core/pty/session-plugin-args.test.ts \
  src/core/sync/skills-contract.test.ts \
  src/core/sync/lock.test.ts

npm --prefix extensions/claude-bridge/webview exec vitest run -- \
  src/lib/send-message.test.ts \
  src/lib/session-actions.test.ts \
  src/components/input-bar/useInputDraft.test.ts
```

## Manual checks

1. Набрать незавершённую строку в Console, выполнить changed skills sync,
   detach/reattach и убедиться, что draft не смешался с reload.
2. Оставить draft в Chat, переключить session и убедиться, что он не попал в
   PTY.
3. Отправить user/project sync burst и убедиться, что effective overlay верен,
   а maintenance revisions coalesce-ятся.
4. Отправить идентичный snapshot повторно: нового reload быть не должно.
5. Изменить plugin snapshot: текущая CLI session не должна молча менять plugin
   roots; новая или явно перезапущенная session получает новую версию.
