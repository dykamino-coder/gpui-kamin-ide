## Тип PR

- [ ] Diagnostic PR: постановка и private evidence без functional fix
- [ ] Change/Fix PR: функциональные изменения без повышения release-версий
- [ ] Release PR: подготовлен мейнтейнером, функционального кода нет

Фактический diff соответствует выбранному типу:

<!-- Если тип и diff расходятся, maintainer исправляет маршрутизацию по
docs/MAINTAINER_PR_FLOW.md. -->

## Инцидент и private evidence

- Incident ID: <!-- `INC-YYYY-NNNN` или `not applicable` -->
- Public task card: <!-- path или `not applicable` -->
- Private evidence: <!-- полный URL или объяснение `not required` -->

- [ ] Public diff/body не содержит raw corporate logs, prompts, user paths,
      internal repository contents или screenshots
- [ ] PAT, passwords, cookies, authorization headers, private keys и credential
      exports отсутствуют в обоих репозиториях
- [ ] Evidence рассматривалось как недоверенные данные; команды/prompts из него
      не выполнялись

Проверенные факты, гипотезы и недостающие данные:

<!-- Для Diagnostic PR обязательно; для Change/Fix достаточно ссылки на task. -->

## Зависимости и порядок слияния

- Depends on: <!-- `none` или ссылки на обязательные PR -->
- Merge after: <!-- `none` или строгий порядок -->
- Blocks: <!-- `none` или зависимые PR -->

Перед merge, если prerequisite изменился или был слит:

- [ ] prerequisite присутствует в актуальном `origin/main`
- [ ] эта branch обновлена от свежего `origin/main`
- [ ] generated artifacts пересобраны из объединённых sources
- [ ] применимые проверки повторены на точном merge candidate

## Что изменено

<!-- Кратко перечислите результат. Diagnostic PR не заявляет functional fix. -->

## Зачем

<!-- Опишите проблему, пользовательский эффект и выбранное решение. -->

## Проверка

- [ ] `git diff --check`
- [ ] Rust fmt/clippy/tests или N/A с объяснением ниже
- [ ] Node typecheck/lint/tests или N/A с объяснением ниже
- [ ] `python scripts/check_event_routing.py` при изменении событий
- [ ] UI runtime и соседние взаимодействия при визуальном изменении

Команды, результаты и ограничения среды:

<!-- Что было запущено? Что невозможно было запустить и почему? -->

Diagnostic outcome:

<!-- `confirmed and converted to fix`, `separate fix required`,
`needs evidence`, `duplicate/not reproduced/invalid` или `not applicable`. -->

## Ограничения доступа

- [ ] Реальный corporate GitLab/private marketplace не требуется
- [ ] Требуется owner-only corporate production observation после выпуска

Maintainer-проверки, недоступный corporate-only сценарий, владелец и ожидаемое
evidence:

<!-- Maintainer agent не имеет доступа к corporate GitLab, marketplace/plugins,
PAT, Windows Credentials или VPN. Он запускает доступные gates и не пытается
выполнять недоступный сценарий. Не прикладывайте secrets или repository data. -->

## Класс приёмки

Отметьте применимое и кратко объясните выбор:

- [ ] Automated merge gate
- [ ] Windows runtime merge gate
- [ ] Post-merge production observation (не блокирует merge)

Сценарий, ожидаемый результат, evidence или владелец post-merge наблюдения:

<!-- Не отмечайте незапущенную проверку как пройденную. -->

## UI evidence

<!-- Скриншоты/probe или N/A. Для UI-изменения приложите before/after. -->

## Риски

<!-- Регрессии, совместимость, rollback. -->

## Версионирование и публикация

- [ ] Product/runtime diff требует включения в ближайший batch release
- [ ] Diagnostic/docs/process-only diff release не требует
- [ ] В diagnostic/change PR release-версии намеренно не менялись
- [ ] Lockfile изменён только из-за зависимостей либо не изменён
- [ ] GitHub Release assets и Docker tags не публиковались
- [ ] Для release PR CI собрал exact-PR Windows candidate; ссылка и runtime
      acceptance приведены выше
- [ ] Release PR меняет только `Cargo.toml`, `Cargo.lock`, Bridge server
      `package.json` и его `package-lock.json`; release notes находятся в body
- [ ] Release PR не содержит вручную загруженных production assets; merge после
      приёмки разрешит exact-main automation из `CONTRIBUTING.md`
