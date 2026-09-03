# Карточки runtime-инцидентов Claude Bridge

Новые Diagnostic PR добавляют один файл `INC-YYYY-NNNN.md` на одну проблему.
Сырые материалы находятся в private evidence repository; здесь остаются только
sanitized symptom, проверенные факты, status, acceptance и private URL.

Допустимые статусы:

- `reported` — evidence приложен, maintainer ещё не проверил;
- `confirmed` — дефект или нарушенный контракт подтверждён;
- `investigation` — симптом подтверждён, но данных для fix недостаточно;
- `blocked` — явно перечислены недостающие данные или prerequisite;
- `resolved` — связанным Change/Fix PR исправлено и проверено;
- `rejected` — duplicate, invalid или проверяемо не воспроизводится.

Существующие BR-карточки пока остаются в `RUNTIME_RELIABILITY.md`; их не нужно
механически переписывать для начала нового flow.

Каждая незакрытая карточка `INC-*` со статусом `reported`, `confirmed` или
`investigation` обязана иметь строку в очереди incidents
`../RUNTIME_EXECUTION.md` либо входить в его текущую пачку. Иначе evidence
сохранено, но maintainer backlog-run не обязан начать исследование.
