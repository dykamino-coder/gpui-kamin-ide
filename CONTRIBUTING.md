# Разработка и выпуск KaminIDE

Этот документ — источник истины для людей и ИИ-агентов. Архитектурные правила
описаны в `ARCHITECTURE.md`, общие инструкции ИИ-агентам — в `CLAUDE.md`, а
`AGENTS.md` служит коротким адаптером для Codex.

Главное разделение процесса:

- **change PR** содержит код, тесты и документацию, но не повышает release-версии;
- **release PR** принадлежит мейнтейнеру и централизованно повышает версии,
  собирает Windows installer и запускает публикацию.

Так параллельные задачи не конфликтуют в `Cargo.toml`, lockfile и server
manifest, а номер версии всегда соответствует одному проверенному артефакту.

## 1. Ветки и worktree

Каждая задача выполняется в отдельной ветке и, если одновременно работают
несколько людей или агентов, в отдельном worktree. Ветка начинается от свежего
`origin/main`:

```bash
git fetch origin --prune
git worktree add ../work-session-hover \
  -b fix/session-hover origin/main
```

Для Codex-агентов используется префикс `codex/`, например
`codex/fix-session-hover`. Перед работой и перед PR нужно проверить:

```bash
git status --short --branch
git rev-list --left-right --count HEAD...origin/main
```

Нельзя переключать, переписывать или очищать `main` и чужие worktree. Чужие
незакоммиченные изменения сохраняются и не включаются в текущую задачу.

## 2. Обычный change PR

Change PR включает только относящиеся к задаче изменения:

- исходный код;
- тесты и тестовые fixtures;
- документацию;
- lockfile, если действительно изменились зависимости;
- сгенерированные runtime-артефакты, которые намеренно хранятся в Git.

Change PR **не меняет**:

- `[workspace.package].version` в `Cargo.toml`;
- workspace-версии пакетов в `Cargo.lock`;
- версию Bridge server;
- GitHub Release assets и Docker tags.

Даже мейнтейнер делает свои функциональные изменения через change PR без
version bump. Номер выбирается только после определения полного состава
релиза.

Исключение для lockfile: изменение Rust/npm-зависимости должно обновлять
соответствующий lockfile, но текущая release-версия пакета остаётся прежней.

## 3. Архитектурные требования

Следовать `ARCHITECTURE.md`, в частности:

- `ui/` не зависит от `state/`;
- событие проходит через `ShellEvent` → `dispatch` → один domain handler;
- новые Rust-файлы начинаются с `//!`;
- Rust-файл не должен превышать 250 строк; существующее превышение не
  увеличивается без обоснованного рефакторинга;
- комментарий объясняет причину или проверенный инвариант, а не пересказывает
  код;
- UI сохраняет pixel fidelity и существующие keyboard/mouse/focus сценарии.

## 4. Проверки change PR

Запускается применимый набор проверок. Базовый Rust-контур:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python scripts/check_event_routing.py
git diff --check
```

Корневой Node-контур:

```bash
npm ci
npm run check
```

Если менялся Bridge server:

```bash
npm --prefix extensions/claude-bridge/server ci
npm --prefix extensions/claude-bridge/server run typecheck
npm --prefix extensions/claude-bridge/server run lint
npm --prefix extensions/claude-bridge/server test
npm --prefix extensions/claude-bridge/server run format:check
```

Если менялись extension или webview, пересобрать и закоммитить runtime-выход:

```bash
npm --prefix extensions/claude-bridge/extension ci
npm --prefix extensions/claude-bridge/extension run build
npm --prefix extensions/claude-bridge/webview ci
npm --prefix extensions/claude-bridge/webview run build
```

В Git намеренно входят `builtin-extensions/claude-bridge/extension.js` и
собранные `*.html`. Их diff должен соответствовать исходникам.

Для UI-изменения дополнительно проверяются Windows runtime, hover/click,
keyboard/focus, соседние элементы и визуальный результат. Незапущенная из-за
окружения проверка явно указывается в PR — её нельзя выдавать за пройденную.

### Класс приёмки

Каждый change PR выбирает один или несколько классов приёмки и объясняет выбор:

- **automated merge gate** — применимые проверки обязаны пройти до merge;
- **Windows runtime merge gate** — обязателен до merge, если корректность
  зависит от CEF/webview lifecycle, native TUI, focus/keyboard/mouse, host
  respawn или другого поведения, которое unit test достоверно не воспроизводит;
- **post-merge production observation** — неблокирующее наблюдение после
  выпуска. Оно допустимо для диагностики, telemetry и редких сбоев, которые PR
  не заявляет исправленными до получения полевых данных.

Production observation не заменяет Windows runtime gate для уже заявленного
UX-исправления. Если Windows-проверка является merge gate, в PR приводятся
сценарий, ожидаемый результат и evidence. Если проверка отложена до production,
PR явно называет владельца наблюдения и не утверждает, что полевой дефект уже
устранён.

## 5. Коммиты и PR

Заголовок PR и итогового squash-коммита следует Conventional Commits и пишется
по-английски:

```text
fix(sessions): keep hover action pills reachable
feat(installer): add repair mode
perf(webview): reduce resize churn
docs(contributing): define the release flow
```

В описании PR обязательны:

- что и зачем изменено;
- как проверено;
- UI evidence для визуального изменения;
- риски или ограничения;
- явная отметка, что release-версии намеренно не менялись.

Предпочтительный способ объединения change PR — squash merge с сохранением
Conventional Commit-заголовка. Прямой push и force-push в `main` запрещены.

## 6. Release PR

Release PR создаёт мейнтейнер из актуального `origin/main` после объединения
нужных change PR. На время финальной сборки состав релиза фиксируется: если
release branch перебазирована или в неё добавлен код, все проверки и installer
запускаются заново.

В текущей схеме приложение и Docker-образ server выпускаются вместе. Поэтому
release PR выбирает две новые уникальные patch-версии:

| Компонент | Источник истины | Производные файлы |
|---|---|---|
| KaminIDE | `Cargo.toml` → `[workspace.package].version` | `Cargo.lock` |
| Bridge server | `extensions/claude-bridge/server/package.json` | `package-lock.json`; runtime читает manifest напрямую |

Индивидуальные `crates/*/Cargo.toml` используют `version.workspace = true` и
вручную не повышаются. Корневой `package.json`, builtin extension, dashboard,
VS Code API и Claude Code versions — независимые версии и в обычном релизе
KaminIDE не меняются.

Server manifest и lockfile обновляются одной командой:

```bash
npm --prefix extensions/claude-bridge/server \
  version 6.3.118 --no-git-tag-version
```
После изменения Cargo version запускается `cargo check --workspace`, чтобы
перегенерировать workspace entries в `Cargo.lock`.

Release PR содержит только версии, release notes и необходимые release-файлы.
Функциональные исправления оформляются отдельным change PR. Заголовок:

```text
chore(release): KaminIDE 1.0.43 / server 6.3.118
```

Фактические номера выбираются после последнего `fetch`; номера из примера не
резервируются заранее.

## 7. Windows installer

Installer собирается на доверенной Windows x64 машине из точного HEAD
одобренного release PR. Сначала выполняются полные проверки, затем:

```bash
npm ci
npm run build:host
npm run build:payload
cargo build --release
node scripts/build_setup_rust.mjs
```

Единственная точка входа — `scripts/build_setup_rust.mjs`: он зовёт
`scripts/build_installer.mjs` за раскладкой `dist-installer/` и приклеивает её
к своему стабу. NSIS удалён из репозитория (`installer.nsi`,
`scripts/build_rust_installer.mjs`, NSIS-секция сборщика): его стаб ловился
эвристиками антивирусов как дроппер — Kaspersky ругался на 1.0.47.

Результат имеет имя `KaminIDE_<version>_x64-setup.exe`. Build guards проверяют,
что release binary новее `Cargo.toml`, а `runtime.tar.zst` соответствует
текущим host/builtin sources, и не позволяют упаковать старые компоненты под
новой версией.

Минимальный smoke test:

1. обновление поверх предыдущей опубликованной версии;
2. автоматический перезапуск приложения;
3. версия в UI и `%LOCALAPPDATA%\Programs\KaminIDE-GPUI\version.txt`;
4. `DisplayVersion` в
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\KaminIDE-GPUI`;
5. контекстное меню `Open with KaminIDE`;
6. проверка `%TEMP%\kaminide-install.log` и in-app update.

Для изменений самого installer выполняется полная матрица из
`plan/104-installer-test-matrix.md`.

## 8. Публикация

Порядок публикации обязателен:

1. release PR одобрен и больше не изменяется;
2. installer собран и проверен из его HEAD;
3. новый installer загружен в GitHub Release `kaminide-latest`;
4. в Release оставлен ровно один актуальный `KaminIDE_*_x64-setup.exe`;
5. только после этого release PR объединён с `main`;
6. Docker workflow читает новую server version, скачивает installer и
   публикует version tag и `latest`;
7. проверяются Docker tag, `/download` и `/updates/kaminide/...`.

Если asset загрузить после merge, workflow может успеть собрать image со старым
installer. Если release branch изменилась после сборки, asset считается
устаревшим и пересобирается.

Перезапись уже опубликованной версии не допускается. Исправление после релиза
получает новый patch и проходит тот же процесс как hotfix.

Успешная публикация обязана оставлять проверяемую связь между release HEAD и
artifact: Docker image содержит source/revision labels и provenance attestation,
а workflow до build проверяет version/digest installer asset и после push —
version tag, digest и labels. GitHub Actions pin-ятся на полные commit SHA.
Существующий tag с неизвестным или другим revision является ошибкой, а не
успешным skip. Ручная публикация image вне release pipeline не считается
завершённым релизом.
