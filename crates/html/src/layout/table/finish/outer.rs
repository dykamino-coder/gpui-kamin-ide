//! Внешняя коробка таблицы: клон-обёртка для рамки сросшейся модели и итоговая обёртка.

use super::wrap_with_captions;
use crate::dom::Element;
use crate::render::styled_div_with;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(super) fn table_outer_box(
    e: &Element,
    bw: [f32; 4],
    outer_win: [f32; 4],
    collapse: bool,
    pad_px: [f32; 4],
    min_h: Option<Len>,
    min_w: Option<Len>,
    table_border_box: bool,
    fixed_floor: Option<f32>,
    inherited: &Computed,
    needs_clone: bool,
) -> gpui::Div {
    let host_style;

    if needs_clone {
        let mut c = inherited.clone();
        if collapse {
            c.border_width = Default::default();
            c.border_visible = [None; 4];
            // §17.6.2: внутрь таблицы уходит ПОЛОВИНА её кромки. Паддингом,
            // а не рамкой: проба кромок — абсолютный ребёнок по паддинг-боксу,
            // и рамка утащила бы линию сетки внутрь на свою величину.
            c.padding = crate::style::computed::Sides {
                top: Some(Len::Px(outer_win[0] / 2.0)),
                right: Some(Len::Px(outer_win[1] / 2.0)),
                bottom: Some(Len::Px(outer_win[2] / 2.0)),
                left: Some(Len::Px(outer_win[3] / 2.0)),
            };
        }
        // Пол фиксированной раскладки (см. `fixed_floor`) — сама ширина
        // коробки: сетка внутри ровно такой ширины.
        if let Some(f) = fixed_floor {
            c.width = Some(Len::Px(f));
        }
        c.min_height = min_h;
        c.min_width = min_w;
        if table_border_box {
            c.border_box = Some(true);
            // Вертикальное письмо: `height` — это ИНЛАЙН-размер стола
            // (Blink: `ComputeTableInlineSize` читает `style.LogicalWidth()`,
            // а при `vertical-*` это физическое `height`), и наш конвейер УЖЕ
            // потратил её как КОНТЕНТНУЮ величину: предел ортогонального
            // потока сеется из `e.style.height` без вычета краёв (блок
            // `merged.ortho_limit` выше), и повёрнутый абзац рвёт строку
            // ровно по нему. Значит второй раз, коробкой, та же величина
            // обязана лечь по content-box, иначе краи вычитаются дважды и
            // стол выходит короче содержимого на свои рамки
            // (`row-progression-vrl-002`: 140.0 вместо 180.0 при неизменной
            // туши). Флаг `border_box` один на обе оси, поэтому его НЕ
            // снимаем — иначе content-box получила бы и `width`, то есть
            // БЛОЧНАЯ ось, где border-box верен; вместо этого краи инлайн-оси
            // добавляются к самой величине. Гейт тот же, что у
            // транспонирования решётки и у `spacing_phys`.
            if e.style.vertical == Some(true)
                && let Some(Len::Px(h)) = c.height
            {
                // Сросшийся стол несёт свои краи не рамкой, а паддингом в
                // половину победившей кромки (см. ветку `collapse` выше) —
                // берём ровно то, что легло в коробку.
                let inline_edges = if collapse {
                    (outer_win[0] + outer_win[2]) / 2.0
                } else {
                    bw[0] + bw[2] + pad_px[0] + pad_px[2]
                };
                c.height = Some(Len::Px(h + inline_edges));
            }
        }
        host_style = c;
        styled_div_with(e, &host_style).flex().flex_col()
    } else {
        styled_div_with(e, inherited).flex().flex_col()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_table_outer(
    e: &Element,
    caps_top: Vec<AnyElement>,
    caps_bot: Vec<AnyElement>,
    split_wrapper: bool,
    inherited: &Computed,
    root_table: bool,
    outer: gpui::Div,
    shrink_wrap: bool,
) -> AnyElement {
    let mut outer = outer;
    // Сжатие по содержимому — `min(max-content, доступное)` (CSS 2.1
    // §17.5.2.2: «the used width is the greater of W and MIN» при W =
    // ширине контейнера, если таблица шире): в ряду-обёртке стол обязан
    // ужиматься. Блоку потока сжатие выключено (`flex_shrink = 0` в
    // `collapsed`), и стол с длинным текстом вылезал из узкого родителя
    // на всю max-content ширину. Пол GRIDMIN держит `item_is_table`.
    if shrink_wrap && caps_top.is_empty() && caps_bot.is_empty() && !split_wrapper {
        outer.style().flex_shrink = Some(1.0);
    }
    let outer = outer;
    // Обёртка «заголовок + коробка»: заголовок вне рамки и обрезки.
    let outer = wrap_with_captions(e, caps_top, caps_bot, inherited, outer);
    // Вторая половина §17.4: сама обёртка. Гибкий ряд возвращает сетке сжатие
    // по содержимому — тот же приём, что у корневого стола ниже.
    if split_wrapper {
        let mut wrap = Computed::default();
        wrap.position = e.style.position;
        wrap.inset = e.style.inset;
        wrap.z_index = e.style.z_index;
        let mut wrap = crate::style::apply::apply(div(), &wrap).flex().flex_row();
        wrap.style().no_inline_block_baseline = Some(true);
        return wrap.child(outer).into_any_element();
    }
    // Стол с `width: auto` СЖИМАЕТСЯ по содержимому (§17.5.2): у нас это
    // делает гибкий ряд-обёртка. Приём `align_self: FlexStart` выше работает
    // только когда родитель — гибкая колонка нашей сборки; под `body` со
    // сброшенными полями путь другой, и стол растягивался во всю ширину
    // (`html-display-table`, `root-box-002`). Обёртка снимает зависимость от
    // родителя. Элемент гибкого контейнера, сетки и ячейки не заворачивается:
    // там стол — сам элемент раскладки, и обёртка забрала бы его свойства.
    if shrink_wrap {
        let mut wrap = div().flex().flex_row();
        if root_table {
            wrap = wrap.w_full();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        return wrap.child(outer).into_any_element();
    }
    outer.into_any_element()
}
