# Защита default branch: одноразовая настройка владельцем

[Готовый ruleset](../.github/main-ruleset.json) требует PR, разрешённые review
threads и два CI checks на актуальной базе; запрещает удаление и force push.
Обязательных сторонних approvals — 0: это сохраняет разрешённый в репозитории
author-registration/self-merge flow. Содержательное review и task-specific
acceptance по `CONTRIBUTING.md` остаются обязанностью мейнтейнера.

Файл — конфигурация для применения, **не доказательство включённой защиты**.
Он не содержит repository-specific IDs, credentials или сведений о deployment.
Обычный push файла и правка workflow не меняют GitHub Settings. У исполнявшего
сверку 2026-10-19 агента есть push, но нет admin/maintain; настройка не применена.

Владелец с repository administration permission выполняет из checkout:

```bash
gh repo view --json nameWithOwner,viewerPermission
gh api repos/{owner}/{repo}/rulesets
gh api repos/{owner}/{repo}/branches/main/protection
```

Ответ 404 на последнюю команду может означать отсутствие classic protection
или недостаточные права: его одного недостаточно для вывода. Проверить также
Settings → Rules → Rulesets. Если эквивалентная защита уже действует, не создавать
дубликат и не ослаблять её. Для нового ruleset:

```bash
gh api --method POST repos/{owner}/{repo}/rulesets \
  --input .github/main-ruleset.json
gh api repos/{owner}/{repo}/rules/branches/main
```

`{owner}/{repo}` GitHub CLI подставляет из текущего repository. После применения
проверить effective rules: `pull_request`, оба checks со strict policy,
`deletion`, `non_fast_forward`. Проверить существующий PR: GitHub должен требовать
оба checks, актуальную базу и закрытые review threads. Для уже существующего
ruleset сначала сохранить его JSON, затем сравнить и обновить именно его ID
через Settings; этот runbook не предлагает удалять или перезаписывать чужие rules.

API, поля и необходимые права описаны в
[GitHub REST: repository rulesets](https://docs.github.com/en/rest/repos/rules#create-a-repository-ruleset).
Production secrets и дополнительные полномочия CI для этого не нужны.
