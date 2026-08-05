# Инструкции ИИ-агентам

`CONTRIBUTING.md` — обязательный источник истины для разработки, PR и релизов.
Перед изменениями также прочитать `ARCHITECTURE.md` и более вложенный
`AGENTS.md`, если он появится в затрагиваемой папке.

## Изоляция работы

- Одна задача — одна ветка и один worktree от актуального `origin/main`.
- Перед работой проверить `git status`, текущую ветку и расхождение с
  `origin/main`.
- Не переключать, не переписывать и не очищать `main` или чужой worktree.
- Не включать в diff чужие или не относящиеся к задаче изменения.
- Не применять destructive Git-команды и не делать force-push без явного
  поручения пользователя.

## Change PR по умолчанию

- Любая обычная задача считается change PR.
- Не менять Cargo workspace version, release entries в `Cargo.lock`, Bridge
  server version, GitHub Release assets и Docker tags.
- Lockfile обновляется только вследствие изменения зависимостей.
- Использовать Conventional Commit-заголовок на английском.
- Коммит, push и создание PR выполнять только когда это входит в поручение.

## Качество

- Соблюдать границы `ui/`, `state/`, `host/` из `ARCHITECTURE.md`.
- После изменения маршрутизации событий запускать
  `python scripts/check_event_routing.py`.
- Запускать применимые fmt, lint, clippy, test и UI-проверки из
  `CONTRIBUTING.md`.
- Не заявлять незапущенную проверку как успешную; назвать ограничение среды.
- Для UI проверить не только целевой сценарий, но и hover/click/focus/keyboard
  соседних элементов и визуальный результат.
- Сгенерированные builtin extension/webview artifacts должны соответствовать
  исходникам и входить в тот же PR.

## Release-операции

Повышение версий, Windows installer, GitHub Release, merge и Docker publication
разрешены только при явном поручении подготовить релиз. Тогда:

- использовать отдельную release branch от свежего `origin/main`;
- не добавлять функциональный код в release PR;
- синхронно обновить все version sources из `CONTRIBUTING.md`;
- собирать installer из неизменного одобренного release HEAD;
- не загружать asset и не публиковать ничего без явной авторизации;
- при любом изменении release HEAD повторить проверки и сборку.
