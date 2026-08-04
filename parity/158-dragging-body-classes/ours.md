# 158 dragging-body-classes — наша реализация
Файлы: crates/shell/src/root.rs:33-58 (DragKind/DragState), 2918-2940+ (begin_drag/drag_move), 4946-4990 (window-level mouse-move/up: drag сплиттеров, chip/tab/tool drag)

## Структура/содержание
Прямого аналога НЕТ — и по большей части он не нужен архитектурно:
- «iframe глотает mousemove» невозможен: вебвью — нативные composition-поверхности вне gpui-дерева, а drag сплиттера обрабатывается window-level `on_mouse_move` в root, не элементами.
- «elementFromPoint проваливается к zone-контейнерам» заменён явным hit-test по probe_registry bounds (root.rs:4963-4966).
Чего реально нет: (1) глобального курсора `grabbing` на время tool-drag (курсор остаётся обычным, ClosedHand не ставится); (2) подавления hover-эффектов и тултипов на время любого drag — hover-стили элементов продолжают срабатывать под ghost'ом.

## Метрики
Порог старта drag: 4px по любой оси (tab/chip/tool — root.rs:4931,4940,4950). Спец-обработка вебвью при drag: SetBounds-доводчик через 150мс после mouse-up (root.rs:4979-4990) — наша замена «замирающему resize».

## Отличия от original.md той же папки
- `body.kamin-dragging iframe { pointer-events:none }` — не требуется (нет iframe'ов; вопрос перехвата мыши вебвью решён в слое wv_visual, отдельная зона).
- `body.kamin-dragging :where(button…) { pointer-events:none }` — НЕ ВОСПРОИЗВЕДЕНО: hover/tooltip во время drag сплиттера и тула не гасятся (возможные визуальные артефакты: подсветка кнопок под курсором при перетаскивании).
- `cursor: grabbing !important` при tool-drag — НЕ РЕАЛИЗОВАНО (gpui CursorStyle::ClosedHand доступен, не используется).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы
- цвета: N/A: цвета
