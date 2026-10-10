//! Div ячейки: слитый стиль коробки, слои фона ряда/группы, относительный сдвиг, ортогональная ячейка.

use crate::dom::Element;
use crate::layout::positioned::relative::relative_shift;
use crate::layout::table::anon::html_cell;
use crate::render::styled_div_with;
use crate::style::computed::{Align, Computed};
use crate::style::values::value::Len;
use gpui::{ParentElement, Styled, px};

pub(super) fn cell_box_div(
    paint_layers: bool,
    cell_bgs: &std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    row: &Element,
    carry: (
        f32,
        f32,
        Option<crate::style::values::value::Color>,
        Option<&Element>,
    ),
    shift: (f32, f32),
    cm: &Computed,
    cell: &Element,
) -> gpui::Div {
    // Коробке ячейки нужен СЛИТЫЙ стиль: у сырого `cell.style`
    // шрифтовые единицы не разрешены, и `apply` считает их от жёстких
    // 16 точек — `padding: 1em` при кегле 20 давало 16
    // (`table-height-algorithm-008a/b/c`). Берутся только те слоты,
    // где это безопасно: ширину и её минимум решает дорожка, и их
    // подмена уже мерилась отдельно.
    let box_style = {
        let mut c = cell.style.clone();
        let fixup = |own: Option<Len>, merged: Option<Len>| match own {
            Some(Len::Px(_)) | None => own,
            _ => merged,
        };
        c.padding = crate::style::computed::Sides {
            top: fixup(c.padding.top, cm.padding.top),
            right: fixup(c.padding.right, cm.padding.right),
            bottom: fixup(c.padding.bottom, cm.padding.bottom),
            left: fixup(c.padding.left, cm.padding.left),
        };
        c.height = fixup(c.height, cm.height);
        // Толщина рамки — тем же правилом: `border: 1em solid` у
        // ячейки доезжало сюда неразрешённым `Em`, а раскладка кладёт
        // только `Px` (`apply.rs`: прочие длины молча отбрасываются),
        // и рамка пропадала целиком (`table-height-algorithm-008b/c`
        // против зелёной `-008a`, где то же самое написано отступом).
        c.border_width = crate::style::computed::Sides {
            top: fixup(c.border_width.top, cm.border_width.top),
            right: fixup(c.border_width.right, cm.border_width.right),
            bottom: fixup(c.border_width.bottom, cm.border_width.bottom),
            left: fixup(c.border_width.left, cm.border_width.left),
        };
        c
    };
    // Сплошной цвет ячейки сросшейся модели уходит в слой под
    // кромками (`cell_bgs`, см. объявление): коробка остаётся без
    // заливки, цвет пишет проба. Картинка, градиент, `background-clip`,
    // спрятанная или преобразованная ячейка красятся по-прежнему на
    // месте — для них слой не строится.
    let bg_layered = paint_layers
        && box_style.bg_clip.is_none()
        && box_style.gradient.is_none()
        && box_style.bg_image.is_none()
        && box_style.hidden != Some(true)
        && box_style.opacity.is_none_or(|o| o >= 1.0)
        && box_style.transform.is_none()
        && box_style.translate.is_none()
        && box_style.filter.is_none();
    let mut box_style = box_style;
    let own_bg = box_style.background;
    if bg_layered {
        box_style.background = None;
    }
    let mut d = styled_div_with(cell, &box_style);
    // Заливка строки И ГРУППЫ строк: своей коробки у них в общей сетке
    // не остаётся, поэтому фон рисуют ячейки. Раньше бралась только
    // строка, и `<tbody style="background">` пропадал молча
    // (`position-relative-table-tbody-left`).
    let mut layer_bg = bg_layered.then_some(own_bg).flatten();
    if let Some(bg) = carry.2 {
        // Ряд с КАРТИНКОЙ красит и цвет САМ (см. CellsClipped) —
        // ячейка его не дублирует, иначе цвет ложится поверх
        // картинки. Ряду только с тенью цвет оставляют ячейки.
        let picture = row.style.bg_image.is_some() || row.style.gradient_raw.is_some();
        if !picture {
            if bg_layered {
                layer_bg = Some(bg);
            } else {
                d = d.bg(bg.to_hsla());
                // A row/group background is replicated here only for
                // painting (CSS 2.1 section 17.5.1); it must not snap
                // each cell's text into an independent fill frame.
                d.style().css_synthetic_background = Some(true);
            }
        }
    }
    if let Some(bg) = layer_bg {
        d = d.child(crate::layout::table::paint::cell_bg_probe(
            cell_bgs.clone(),
            bg.to_hsla(),
        ));
    }
    // Сдвиг строки или её группы: собственного элемента у них нет,
    // поэтому край, заданный на `<tr>`/`<tbody>`, двигает ячейки.
    // A relatively positioned cell's own percentage insets resolve
    // against the row's specified height (`position-relative-013`),
    // not the table grid the cell is laid out in; the row offset adds.
    let own_pct = cell.style.position == Some(crate::style::computed::Position::Relative)
        && [
            cell.style.inset.left,
            cell.style.inset.right,
            cell.style.inset.top,
            cell.style.inset.bottom,
        ]
        .iter()
        .any(|l| matches!(l, Some(Len::Pct(_))));
    if own_pct {
        let own = relative_shift(cell, Some(row));
        d = d
            .relative()
            .left(px(shift.0 + own.0))
            .top(px(shift.1 + own.1));
        let s = d.style();
        s.inset.right = None;
        s.inset.bottom = None;
    } else if shift != (0.0, 0.0) {
        d = d.relative().left(px(shift.0)).top(px(shift.1));
    }
    d
}

pub(super) fn ortho_cell_box(
    e: &Element,
    row: &Element,
    cm: &Computed,
    cell: &Element,
    d: gpui::Div,
) -> gpui::Div {
    // Умолчание браузера для ячейки — `vertical-align: middle`: без
    // него полоса высотой 10px в строке 22px стояла на 6 точек выше.
    // Ортогональная ячейка заводит СВОЙ контекст форматирования и
    // раскладывает содержимое в СВОЁМ письме (css-writing-modes-4
    // §3.1: `writing-mode` применяется к `table-cell`; §7.1: правила
    // горизонтальной оси переходят на вертикальную). Значит ось
    // блочного потока внутри неё ГОРИЗОНТАЛЬНА: дети идут справа
    // налево у `vertical-rl`/`sideways-rl` и слева направо у
    // `vertical-lr`/`sideways-lr`, а не столбиком. Тот же приём, что
    // у голого блока в `element()` (:14320). Blink строит место
    // ячейки её собственным письмом —
    // `table_layout_utils.cc:218`:
    // `ConstraintSpaceBuilder(table_writing_direction.GetWritingMode(),
    //                         cell_writing_direction, /* is_new_fc */ true)`.
    let ortho_cell = cm.vertical == Some(true) && e.style.vertical != Some(true);
    let mut d = if ortho_cell {
        let d = d.flex();
        if cm.vertical_rl == Some(true) {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        }
    } else {
        d.flex().flex_col()
    };
    if ortho_cell {
        // ОРТОГОНАЛЬНАЯ ячейка (вертикальный контент в горизонтальной
        // таблице): строчная ось вертикальна — `text-align` правит
        // ВЕРТИКАЛЬНОЕ положение строки (line-left = верх), а
        // `vertical-align` уходит на поперечную ось
        // (table-cell-align-005/006).
        use crate::style::computed::TextAlign;
        // `start`/`end` — края СТРОКИ: вертикальная строка идёт
        // сверху вниз, `dir=rtl` разворачивает её снизу вверх.
        let rtl = cm.rtl == Some(true);
        d = match cm.text_align {
            Some(TextAlign::Right) => d.items_end(),
            Some(TextAlign::Center) => d.items_center(),
            Some(TextAlign::End) if !rtl => d.items_end(),
            Some(TextAlign::Start) if rtl => d.items_end(),
            _ => d.items_start(),
        };
        // Начальное значение `vertical-align` — `baseline` (§17.5.3),
        // и у одиночного ряда это ВЕРХ ячейки, а не середина. Пока
        // умолчанием стояла середина, содержимое опускалось на
        // полразницы высот (`direction-applies-to-005`: квадрат на
        // 30 точек ниже эталона).
        d = match cm.vertical_align {
            Some(Align::End) => d.justify_end(),
            Some(Align::Center) => d.justify_center(),
            _ => d.justify_start(),
        };
    } else {
        d = match cm.vertical_align {
            Some(Align::End) => d.justify_end(),
            Some(Align::Center) => d.justify_center(),
            _ => d.justify_start(),
        };
        // `vertical-align: baseline` (CSS 2.1 §17.5.3): первые
        // базовые ячеек ряда совпадают, ряд растёт на сдвиг, а
        // коробка ячейки по-прежнему заполняет ряд — сдвигается
        // только содержимое (taffy `Style::table_cell_baseline`;
        // Blink `table_layout_utils.cc` `ComputeRowBaseline`).
        // Значение — СОБСТВЕННОЕ ячейки: `vertical-align` не
        // наследуется, а слитый `cm` тянет его от любого предка.
        // Только `td`/`th` берут значение ряда (UA-правило
        // `td, th { vertical-align: inherit }`).
        let own_va = cell.style.vertical_align.or(if html_cell(cell) {
            row.style.vertical_align
        } else {
            None
        });
        if own_va == Some(Align::Baseline) && e.style.vertical != Some(true) {
            d.style().table_cell_baseline = Some(true);
        }
    }
    d
}
