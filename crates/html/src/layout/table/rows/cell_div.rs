//! Div ячейки: слитый стиль коробки, слои фона ряда/группы, относительный сдвиг, ортогональная
//! ячейка.

use crate::dom::Element;
use crate::layout::positioned::relative::relative_shift;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{ParentElement, Styled, px};

pub(super) fn cell_box_div(
    layers: &crate::layout::table::context::TableLayers<'_>,
    out: &mut crate::layout::table::context::RowsOut,
    row_state: &crate::layout::table::context::RowState<'_>,
    cm: &Computed,
    cell: &Element,
    cx: &crate::layout::cx::LayoutCx,
) -> gpui::Div {
    let row = row_state.row;
    let carry = row_state.carry;
    let shift = (carry.dx, carry.dy);
    let paint_layers = layers.paint_layers;
    let cell_bgs = &out.cell_bgs;
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
        c.used_mut().reset_box_layout.padding = crate::style::computed::Sides {
            top: fixup(
                c.used().reset_box_layout.padding.top,
                cm.reset_box_layout.padding.top,
            ),
            right: fixup(
                c.used().reset_box_layout.padding.right,
                cm.reset_box_layout.padding.right,
            ),
            bottom: fixup(
                c.used().reset_box_layout.padding.bottom,
                cm.reset_box_layout.padding.bottom,
            ),
            left: fixup(
                c.used().reset_box_layout.padding.left,
                cm.reset_box_layout.padding.left,
            ),
        };
        c.used_mut().reset_box_layout.height =
            fixup(c.used().reset_box_layout.height, cm.reset_box_layout.height);
        // Толщина рамки — тем же правилом: `border: 1em solid` у
        // ячейки доезжало сюда неразрешённым `Em`, а раскладка кладёт
        // только `Px` (`paint/apply/`: прочие длины молча отбрасываются),
        // и рамка пропадала целиком (`table-height-algorithm-008b/c`
        // против зелёной `-008a`, где то же самое написано отступом).
        c.used_mut().reset_border.border_width = crate::style::computed::Sides {
            top: fixup(
                c.used().reset_border.border_width.top,
                cm.reset_border.border_width.top,
            ),
            right: fixup(
                c.used().reset_border.border_width.right,
                cm.reset_border.border_width.right,
            ),
            bottom: fixup(
                c.used().reset_border.border_width.bottom,
                cm.reset_border.border_width.bottom,
            ),
            left: fixup(
                c.used().reset_border.border_width.left,
                cm.reset_border.border_width.left,
            ),
        };
        // Background size and position in font units — the same rule: the
        // painter sees no font size on the raw style and used 16px
        // (`table-cell-align-002`: `.5em` at 20px drew at 8px).
        c.used_mut().reset_background.bg_pos.x = fixup(
            c.used().reset_background.bg_pos.x,
            cm.reset_background.bg_pos.x,
        );
        c.used_mut().reset_background.bg_pos.y = fixup(
            c.used().reset_background.bg_pos.y,
            cm.reset_background.bg_pos.y,
        );
        if let (
            crate::style::computed::BgSize::Fixed(w, h),
            crate::style::computed::BgSize::Fixed(mw, mh),
        ) = (
            c.used().reset_background.bg_size,
            cm.reset_background.bg_size,
        ) {
            c.used_mut().reset_background.bg_size =
                crate::style::computed::BgSize::Fixed(fixup(w, mw), fixup(h, mh));
        }
        c
    };
    // Сплошной цвет ячейки сросшейся модели уходит в слой под
    // кромками (`cell_bgs`, см. объявление): коробка остаётся без
    // заливки, цвет пишет проба. Картинка, градиент, `background-clip`,
    // спрятанная или преобразованная ячейка красятся по-прежнему на
    // месте — для них слой не строится.
    let collapsed = crate::layout::table::anon::row_collapsed(row, carry.group);
    let bg_layered = paint_layers
        && !collapsed
        && box_style.used().reset_background.bg_clip.is_none()
        && box_style.used().reset_background.gradient.is_none()
        && box_style.used().reset_background.bg_image.is_none()
        && box_style.cascaded().inh.ui.hidden != Some(true)
        && box_style
            .used()
            .reset_effects
            .opacity
            .is_none_or(|o| o >= 1.0)
        && box_style.used().reset_transforms.transform.is_none()
        && box_style.used().reset_transforms.translate.is_none()
        && box_style.used().reset_effects.filter.is_none();
    let mut box_style = box_style;
    let own_bg = box_style.used().reset_background.background;
    if bg_layered {
        box_style.used_mut().reset_background.background = None;
    }
    let mut d = cx
        .contracts
        .build
        .styled_box(cell, &BoxStyle::shared(box_style.used()), cx);
    if collapsed {
        d.style().opacity = Some(0.0);
    }
    // Заливка строки И ГРУППЫ строк: своей коробки у них в общей сетке
    // не остаётся, поэтому фон рисуют ячейки. Раньше бралась только
    // строка, и `<tbody style="background">` пропадал молча
    // (`position-relative-table-tbody-left`).
    let mut layer_bg = bg_layered.then_some(own_bg).flatten();
    if let Some(bg) = carry.background {
        // Ряд с КАРТИНКОЙ красит и цвет САМ (см. CellsClipped) —
        // ячейка его не дублирует, иначе цвет ложится поверх
        // картинки. Ряду только с тенью цвет оставляют ячейки.
        let picture = row.style.used().reset_background.bg_image.is_some()
            || row.style.used().reset_background.gradient_raw.is_some()
            || (row.style.used().reset_background.background.is_some()
                && row.style.used().reset_background.bg_clip.is_some());
        if !picture {
            if bg_layered {
                layer_bg = Some(bg);
            } else {
                d = d.bg(bg.to_hsla());
                // A row/group background is replicated here only for
                // painting (CSS 2.1 section 17.5.1); it must not snap
                // each cell's text into an independent fill frame.
                d = d.with_paint_metadata(crate::text::raster::ReplicatedBackground);
            }
        }
    }
    if let Some(bg) = layer_bg {
        d = d.child(crate::layout::table::probes::cell_bg_probe(
            cell_bgs.clone(),
            bg.to_hsla(),
        ));
    }
    // Сдвиг строки или её группы: собственного элемента у них нет,
    // поэтому край, заданный на `<tr>`/`<tbody>`, двигает ячейки.
    // A relatively positioned cell's own percentage insets resolve
    // against the row's specified height (`position-relative-013`),
    // not the table grid the cell is laid out in; the row offset adds.
    let own_pct = cell.style.used().reset_positioned.position
        == Some(crate::style::computed::Position::Relative)
        && [
            cell.style.used().reset_positioned.inset.left,
            cell.style.used().reset_positioned.inset.right,
            cell.style.used().reset_positioned.inset.top,
            cell.style.used().reset_positioned.inset.bottom,
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

mod orthogonal;
pub(super) use orthogonal::ortho_cell_box;
