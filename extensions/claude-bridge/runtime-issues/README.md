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

Все карточки `INC-*` со статусом `reported`, `confirmed`, `investigation` или
`blocked` автоматически образуют входящую очередь. Diagnostic PR создаёт или
уточняет только свою карточку и не редактирует общий
`../RUNTIME_EXECUTION.md`; статусы `resolved` и `rejected` закрывают incident.

Выбранные ID продвигаются в текущую или планируемую runtime-пачку отдельным
coordination PR. Maintainer agent фиксирует snapshot при запуске, поэтому новая
карточка или PR не расширяет уже выполняемую пачку и относится к следующему
запуску.
