//! Общий рукав `element()`: блочная коробка без особого тега.
mod columns;
pub(super) use columns::ColumnPlan;
pub(super) use columns::column_plan;

mod children;
pub(super) use children::generic_children;

// owner: A

mod multicol;
use multicol::multicol_box;

use crate::dom::Element;
use crate::layout::positioned::absolute_overflow;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, Styled, px};

pub(super) fn generic_box(
    e: &Element,
    mut merged: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
) -> AnyElement {
    let mut d = styled_div_with(e, &merged);
    let overflow_plan = absolute_overflow::Plan::new(&merged, inherited);
    if let Some(plan) = &overflow_plan {
        plan.prepare(d.style());
    }
    // ЗАМЕРЕНО И ЗАКРЕПЛЕНО: минимума высоты в видимую область на
    // коробке корня БОЛЬШЕ НЕТ. §10.6.3 — высота корня `auto`, ростом
    // с окно обязан быть начальный содержащий блок, а не коробка:
    // рамка и фон корня уходили полосами до низа окна
    // (`background-root-011`, `normal-flow/root-box-001`). Абсолютные
    // потомки всплывают в слой ICB (`icb_open`/`icb_close`), и
    // содержащим блоком им служит корневой `div` стенда.
    //
    // Замер по семьям backgrounds/*, *root*, positioning/*,
    // containing-block*, normal-flow/*, *margin*: приобретено 11,
    // потеряна одна (`background-root-024`: слой донора-тела лежит в
    // детях корня и отсчитывался от его padding-box).
    // Многоколоночный поток. Своей многоколоночной раскладки нет, но
    // сетка даёт то же расположение: число рядов считаем по числу
    // детей, а заполнение идёт по колонкам — тогда порядок совпадает
    // с браузерным (сверху вниз, затем в следующую колонку).
    // Число колонок бывает задано и КОСВЕННО — их шириной: сколько
    // целых колонок этой ширины влезает в коробку, столько их и будет
    // (css-multicol-1 §7.3). Ширина коробки нужна заданная: без неё
    // считать не от чего, и остаётся прежняя дорожечная раскладка.
    // Умолчание `column-gap: normal` — один кегль (css-align §8.3).
    // Вычисленные значения, а не заданные: `resolve_em` живёт в
    // `inline::inherit`, поэтому точки лежат в `merged`
    // (`render.rs:13235`), а в `e.style` остаётся `Len::Em`. Кегль
    // для `column-gap: normal` — СОБСТВЕННЫЙ кегль элемента
    // (css-align §8.3; Blink `length_utils.cc:1356`
    // `style.GetFontDescription().ComputedPixelSize()`), и он тоже
    // разрешён только в `merged`.
    let ColumnPlan {
        used_gap,
        column_width,
        col_rl,
        col_axis,
        col_vert,
        col_inline_size,
        used_count,
        width_driven,
        height_driven,
        lone_span,
    } = column_plan(e, &merged, opts);
    if let Some(cols) = used_count
        .filter(|n| *n > 1)
        .or(width_driven.then_some(0))
        .or(height_driven.then_some(1))
        .or(lone_span.then_some(1))
    {
        let result = multicol_box(
            d,
            e,
            merged,
            inherited,
            opts,
            column_width,
            cols,
            used_gap,
            col_axis,
            col_vert,
            col_rl,
            col_inline_size,
            outer_row,
            lone_span,
        );
        (d, merged) = match result {
            Ok(state) => state,
            Err(el) => return el,
        };
    } else if let Some(Len::Px(w)) = column_width {
        // Ширина колонки без их числа — это «сколько влезет»: ровно
        // то, что умеет короткая форма дорожек в GPUI.
        d = d.grid().grid_cols_min(px(w));
    // Ось блочного потока не зависит от `display` (css-writing-modes-4
    // §3.1): inline-block, ячейка, list-item с вертикальным письмом
    // раскладывают детей той же горизонтальной осью, что и голый блок
    // (`block-flow-direction-*`, `line-box-direction-*`).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: явный `Some(Block)` — он же стоит у
    // блокифицированных (абсолют в сетке), и
    // `grid-positioned-children-writing-modes-001` 0.39 -> 1.32 даже с
    // гейтом «родитель не сетка».
    // `list-item` сюда НЕ пускать (★ ЗАМЕРЕНО: `li` с одним текстом
    // становился рядом — `line-box-direction-vrl-019/vlr-020`
    // 6.58 -> 12.55).
    } else if merged.vertical == Some(true)
        // `display: table-caption` — это `Display::Block` С МЕТКОЙ
        // (`computed.rs:3125`), и голый `Some(Block)` сюда пускать
        // нельзя (замеренный откат выше). Метку же ставит ТОЛЬКО
        // само объявление `display: table-caption`, тег `<caption>`
        // её не несёт, а подпись ВНУТРИ стола сюда не приходит вовсе
        // — её строит `table()`. Письмо применяется к подписи
        // (css-writing-modes-4 §3.1, «Applies to: all elements
        // except table row groups, column groups, rows, columns»),
        // значит ось её блочного потока задаёт письмо, а не `display`.
        // Blink: `table_layout_algorithm.cc:67`
        // `ConstraintSpaceBuilder(space, caption.Style().GetWritingDirection(), true)`.
        && (matches!(
            e.style.display,
            None | Some(Display::InlineBlock) | Some(Display::TableCell)
        ) || e.style.is_caption == Some(true)
            // Объявленный `display: flow-root` — тот же блок со своим
            // контекстом (`computed.rs` ставит ему `Some(Block)` с
            // меткой `flow_root`); блокифицированный абсолют метки не
            // несёт, откат выше его не касается. Без этого
            // `flow-root` в `vertical-rl` раскладывал детей
            // горизонтальным блоком: хост полос мерился по
            // min-content, флоаты-колонки эталона
            // `css-break/background-image-001` вставали поперёк строки
            // и пропадали.
            || (e.style.flow_root == Some(true)
                && e.style.display == Some(Display::Block)))
    {
        // Вертикальное письмо: ось блочного потока — горизонтальная.
        // Дети идут слева направо (`vertical-lr`) или справа налево
        // (`vertical-rl`).
        d = d.flex();
        d = if merged.vertical_rl == Some(true) {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        };
        // `sideways-lr`: строка идёт снизу вверх — начало строчной
        // оси у НИЖНЕГО края (css-writing-modes-4 §block-flow).
        // Прижим коробки к низу — ЗАПЛАТКА того времени, когда абзац
        // вертелся по часовой и его содержимое росло от верха.
        // С поворотом против часовой (`VerticalText::ccw`) строка сама
        // начинается у нижнего края, и второй прижим снова уводит
        // рисунок. Снимать ВМЕСТЕ с патчем поворота и мерить
        // `wm-propagation-body-047`, `abs-pos-border-offset-002` —
        // ровно те две пары, ради которых заплатка ставилась.
        if e.style.width.is_none() {
            d = d.flex_shrink_0();
        }
    } else if e.style.display.is_none() {
        // Блок без явного `display` — блочная раскладка taffy, а не
        // гибкая колонка. Колонка навязывала детям сжатие: ребёнок
        // выше родителя ужимался, тогда как браузер даёт ему вылезти.
        // Схлопывание вертикальных отступов при этом делает сама
        // раскладка — включая протекание через пустой блок.
        d = d.flex().flex_col();
        // rtl-прижим переполняющих блоков — ТОЧЕЧНЫЙ align_self End
        // детям с заданной шириной (в blocks): items_end на контейнере
        // снимал stretch у всех, и абзац в rtl ужимался до текста —
        // text-align внутри пустел (text-align-end-001: bw=124.8
        // вместо 300). ЗАМЕРЕНО: +18 css-text при −2..3 wm и −4 mix
        // (spot-механика abs-pos-border-offset полагалась на
        // items_end — новый след) — нетто +10.
    }
    // Ряд по умолчанию — но не тогда, когда письмо справа налево:
    // там ряд обязан идти в обратную сторону, и общая ветка его
    // разворот отменяла. Письмо — СЛИТОЕ: `direction` наследуется
    // (css-writing-modes-4 §2.1), и ряд под `body { direction: rtl }`
    // без своего `direction` шёл слева направо — поля
    // `margin-inline-start` эталонов вставали не между элементами
    // (`gap-001-rtl-ref`, `gap-003-rtl-ref`).
    if e.style.display == Some(Display::Flex)
        && e.style.flex_dir.is_none()
        && merged.rtl != Some(true)
    {
        // При вертикальном письме умолчание `row` — это ось строки, а
        // она идёт сверху вниз.
        d = if merged.vertical == Some(true) {
            d.flex_col()
        } else {
            d.flex_row()
        };
    }
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: отдавать субсетке обычной сетки
    // РАЗРЕШЁННЫЙ кусок родительских дорожек — тем же кодом, что и
    // проход лунок (`if item.style.subgrid` в `lanes`), с вычетом
    // своих краёв и правкой зазора. Кусок брался из ЯВНЫХ линий
    // ребёнка (в обычной сетке размещение делает вендор, и другого
    // источника `at`/`span` до раскладки нет). Срез css-grid (1133
    // пары, 646 зелёных): 645, приобретено НОЛЬ, и
    // `row-auto-placed-subgrid-nested-subgrid-inherited-tracks-001`
    // ушла 0.00 → HUNG (вложенная субсетка зацикливает раскладку).
    // Возвращать только вместе с размещением субсетки НА НАШЕЙ
    // стороне: пока `at`/`span` берутся из линий, вложенный случай
    // получает кусок от куска и сходится не всегда.
    // Имена областей разворачиваются здесь: контейнер и его дети
    // видны одновременно только на этом уровне.
    generic_children(e, d, merged, inherited, opts, overflow_plan)
}
