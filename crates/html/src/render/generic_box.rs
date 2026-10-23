//! Общий рукав `element()`: блочная коробка без особого тега.
use crate::layout::options::RenderOpts;
use crate::paint::box_tree::styled::styled_div_with;
mod clamp_host;
mod columns;
pub(super) use columns::ColumnPlan;
pub(super) use columns::column_plan;

mod child_borrow;
mod children;
pub(crate) use children::child_probes;
pub(super) use children::generic_children;

mod multicol;
use multicol::multicol_box;

use crate::dom::Element;
use crate::layout::positioned::absolute_overflow;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, Styled, px};

pub(super) fn generic_box(
    e: &Element,
    mut merged: BoxStyle,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
) -> AnyElement {
    // css-writing-modes-4 §4.3 / css-align-3 §9.1: an orthogonal flow exports no
    // baseline to a horizontal block container (Blink skips such children).
    if merged.wm().vertical()
        && !inherited.wm().vertical()
        && !e.inline
        && crate::layout::box_kind::in_flow(&e.style.computed())
        && matches!(
            inherited.display,
            None | Some(Display::Block | Display::InlineBlock)
        )
    {
        merged.baseline_orthogonal = true;
    }
    let host_opts;
    let opts = if clamp_host::active(e, &merged)
        && opts.cx.clamp.frame.is_none_or(|(key, _)| key != e.node_id)
    {
        host_opts = opts.inner(|cx| {
            cx.clamp.build = cx.clamp.build.container(e.node_id);
            cx.clamp.input = Some(crate::text::clamp::ClampInput::Measure(Default::default()));
        });
        &host_opts
    } else {
        opts
    };
    let mut d = styled_div_with(e, &merged, &opts.cx);
    let overflow_plan = absolute_overflow::Plan::new(&merged, inherited, &opts.cx.position.state);
    if let Some(plan) = &overflow_plan {
        plan.prepare(d.style());
    }
    // Минимума высоты в видимую область на коробке корня НЕТ: по §10.6.3
    // высота корня `auto`, ростом с окно обязан быть начальный содержащий
    // блок, а не коробка — иначе рамка и фон корня уходят полосами до низа
    // окна (`background-root-011`, `normal-flow/root-box-001`). Абсолютные
    // потомки всплывают в слой ICB (`icb_open`/`icb_close`), и содержащим
    // блоком им служит корневой `div` стенда. Известный пробел:
    // `background-root-024` — слой донора-тела лежит в детях корня и
    // отсчитывается от его padding-box.
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
    // `style::cascade::inherit`, поэтому точки лежат в `merged`,
    // а в `e.style` остаётся `Len::Em`. Кегль
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
    // Явный `Some(Block)` сюда не пускать: он же стоит у блокифицированных
    // (абсолют в сетке), и гейт «родитель не сетка» не спасает
    // (`grid-positioned-children-writing-modes-001`). `list-item` тоже не
    // пускать: `li` с одним текстом становится рядом
    // (`line-box-direction-vrl-019/vlr-020`).
    } else if merged.wm().vertical()
        // `display: table-caption` — это `Display::Block` С МЕТКОЙ
        // `is_caption`, и голый `Some(Block)` сюда пускать
        // нельзя (оговорка выше). Метку же ставит ТОЛЬКО
        // само объявление `display: table-caption`, тег `<caption>`
        // её не несёт, а подпись ВНУТРИ стола сюда не приходит вовсе
        // — её строит `table()`. Письмо применяется к подписи
        // (css-writing-modes-4 §3.1, «Applies to: all elements
        // except table row groups, column groups, rows, columns»),
        // значит ось её блочного потока задаёт письмо, а не `display`.
        // Blink: `table_layout_algorithm.cc:67`
        // `ConstraintSpaceBuilder(space, caption.Style().GetWritingDirection(), true)`.
        && (matches!(
            e.style.computed().display,
            None | Some(Display::InlineBlock) | Some(Display::TableCell)
        ) || e.style.computed().is_caption == Some(true)
            // Объявленный `display: flow-root` — тот же блок со своим
            // контекстом (`style::computed` ставит ему `Some(Block)` с
            // меткой `flow_root`); блокифицированный абсолют метки не
            // несёт, оговорка выше его не касается. Без этого
            // `flow-root` в `vertical-rl` раскладывал детей
            // горизонтальным блоком: хост полос мерился по
            // min-content, флоаты-колонки эталона
            // `css-break/background-image-001` вставали поперёк строки
            // и пропадали.
            || (e.style.computed().flow_root == Some(true)
                && e.style.computed().display == Some(Display::Block)))
    {
        // Вертикальное письмо: ось блочного потока — горизонтальная.
        // Дети идут слева направо (`vertical-lr`) или справа налево
        // (`vertical-rl`).
        d = d.flex();
        d = if merged.wm().block_rl() {
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
        if e.style.computed().width.is_none() {
            d = d.flex_shrink_0();
        }
    } else if e.style.computed().display.is_none() {
        // Блок без явного `display` — блочная раскладка taffy, а не
        // гибкая колонка. Колонка навязывала детям сжатие: ребёнок
        // выше родителя ужимался, тогда как браузер даёт ему вылезти.
        // Схлопывание вертикальных отступов при этом делает сама
        // раскладка — включая протекание через пустой блок.
        d = d.flex().flex_col();
        // The column stands for a block container: its baselines are those of
        // the first/last child that has one (CSS 2.1 §10.8.1), not a baseline
        // synthesized from the first child's edge (subgrid/baseline-001-ref:
        // an empty sized block before a text block).
        d.style().block_baselines = Some(true);
        // rtl-прижим переполняющих блоков — ТОЧЕЧНЫЙ align_self End
        // детям с заданной шириной (в blocks): items_end на контейнере
        // снимал stretch у всех, и абзац в rtl ужимался до текста —
        // text-align внутри пустел (text-align-end-001: bw=124.8
        // вместо 300). Известный след: spot-механика
        // `abs-pos-border-offset-*` полагалась на items_end.
    }
    // Ряд по умолчанию — но не тогда, когда письмо справа налево:
    // там ряд обязан идти в обратную сторону, и общая ветка его
    // разворот отменяла. Письмо — СЛИТОЕ: `direction` наследуется
    // (css-writing-modes-4 §2.1), и ряд под `body { direction: rtl }`
    // без своего `direction` шёл слева направо — поля
    // `margin-inline-start` эталонов вставали не между элементами
    // (`gap-001-rtl-ref`, `gap-003-rtl-ref`).
    if e.style.computed().display == Some(Display::Flex)
        && e.style.computed().flex_dir.is_none()
        && !merged.wm().rtl()
    {
        // При вертикальном письме умолчание `row` — это ось строки, а
        // она идёт сверху вниз.
        d = if merged.wm().vertical() {
            d.flex_col()
        } else {
            d.flex_row()
        };
    }
    // Не отдавать субсетке обычной сетки РАЗРЕШЁННЫЙ кусок родительских
    // дорожек тем же кодом, что и проход лунок (`if item.style.subgrid` в
    // `lanes`): кусок берётся из ЯВНЫХ линий ребёнка (размещение в обычной
    // сетке делает вендор, другого источника `at`/`span` до раскладки нет),
    // и вложенная субсетка получает кусок от куска и зацикливает раскладку
    // (`row-auto-placed-subgrid-nested-subgrid-inherited-tracks-001`).
    // Вернуть можно только вместе с размещением субсетки НА НАШЕЙ стороне.
    // Имена областей разворачиваются здесь: контейнер и его дети
    // видны одновременно только на этом уровне.
    if clamp_host::active(e, &merged) {
        if let Some((key, width)) = opts.cx.clamp.frame.filter(|(key, _)| *key == e.node_id) {
            let _ = key;
            clamp_host::frame(&mut d, width);
        } else {
            return clamp_host::build(e, merged, inherited, opts, outer_row, d);
        }
    }
    generic_children(e, d, merged, inherited, opts, overflow_plan)
}
