//! Размеры замещаемых с источником-изображением: доли высоты и пределов, сведённые к точкам
//! от содержащего блока до раскладки; используемый размер рисунка по стилю и природным
//! размерам источника (`PictureSizing`, `Picture`), под обособлением размера и с
//! `object-view-box`; автоматический минимум гибкого элемента по пропорции и перенос осей и
//! выравнивания с держателя на измеряемый элемент. Сборку самой картинки (`Img`, растр или
//! вектор источника) по этим размерам ведёт `paint::replaced::image`.

use crate::dom::Element;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};
pub(crate) mod automatic_minimum;
pub(crate) mod contained_size;
mod used_size;
pub(crate) mod view_box;
pub(crate) use used_size::{Picture, PictureSizing};
mod auto_sized;
mod sized;

/// Доля ВЫСОТЫ замещаемого, сведённая к точкам по содержащему блоку.
///
/// Держатель картинки стоит внутри анонимного ряда строки
/// (`inline::as_wrapped_row`): у ряда высота `auto`, элемент прижат по базовой
/// линии и не растягивается, поэтому доля высоты бралась ОТ РЯДА и разрешалась
/// в ноль — картинка не рисовалась ни одной точкой
/// (`background-image-cover-002-ref`). Содержащий блок здесь известен точно —
/// это `inherited`, и доля сводится к точкам ещё на сборке.
///
/// Только обычный блочный контейнер: у гибкого, сеточного и лунок высота
/// приходит от раскладки, и подстановка ломает `row-auto-repeat-auto-023`.
pub(crate) fn pct_height_to_px(e: &Element, inherited: &Computed) -> Element {
    let mut resolved = pct_limits_to_px(e, inherited);
    // Replaced boxes resolve viewport units from their raw style, too.
    resolved.style.cascaded_mut().viewport_unit_insets = resolved
        .style
        .cascaded()
        .viewport_unit_insets
        .or(inherited.viewport_unit_insets);
    let e = &resolved;
    let (Some(Len::Pct(k)), Some(Len::Px(h))) = (e.style.cascaded().height, inherited.height)
    else {
        return e.clone();
    };
    // `inline-block` сюда пускается: картинка СТРОКИ внутри `inline-block` с
    // высотой в точках иначе рисуется природным размером — доля высоты до
    // раскладки не доезжает (`intrinsic-percent-replaced-021`: `<button>`
    // 100px, картинка 200×200 вместо 100×100; тот же `div` блоком — верно).
    if !matches!(
        inherited.display,
        None | Some(Display::Block) | Some(Display::InlineBlock)
    ) {
        return e.clone();
    }
    // `height` в разборе — высота СОДЕРЖИМОГО (§10.6.2, content-box): отступы
    // и рамку прибавляет уже раскладка. Вычитать их отсюда нельзя — картинка
    // выходила на 6 точек короче (0.91 вместо 0.00 на `cover-002`).
    if h <= 0.0 {
        return e.clone();
    }
    // …но при `box-sizing: border-box` у РОДИТЕЛЯ заданная высота включает
    // его отступы и рамку, а опора доли — содержимое (CSS 2.1 §10.5,
    // css-sizing-3 §box-sizing): `.inner {box-sizing: border-box; height: 50px;
    // padding-top: 25px}` даёт холсту 25, а не 50
    // (`intrinsic-percent-replaced-012/013`).
    let h = if inherited.border_box == Some(true) {
        let px_of = px_lossy_zero;
        let b = inherited.borders();
        (h - px_of(inherited.padding.top)
            - px_of(inherited.padding.bottom)
            - px_of(b.top)
            - px_of(b.bottom))
        .max(0.0)
    } else {
        h
    };
    let mut copy = e.clone();
    copy.style.cascaded_mut().height = Some(Len::Px(k * h));
    copy
}

/// Пределы замещаемого в ДОЛЯХ — в точки от содержащего блока.
///
/// Размер рисунка считается на сборке (`image_with`), и доля там уже не с чем
/// сравнивать: `max-width: 100%` у картинки отбрасывался целиком, и она
/// вылезала за родителя (css-sizing-3 §5: пределы решаются от содержащего
/// блока, как и сам размер). Ширина берётся от ширины содержащего блока,
/// высота — от его высоты и только у обычного блочного контейнера: у гибкого
/// и сеточного высота приходит от раскладки (та же оговорка, что у
/// `pct_height_to_px`).
fn pct_limits_to_px(e: &Element, inherited: &Computed) -> Element {
    let of = |l: Option<Len>, base: Option<Len>| match (l, base) {
        (Some(Len::Pct(k)), Some(Len::Px(b))) if b > 0.0 => Some(Len::Px(k * b)),
        _ => l,
    };
    let block_cb = matches!(inherited.display, None | Some(Display::Block));
    let h_base = block_cb.then_some(inherited.height).flatten();
    let mut copy = e.clone();
    copy.style.cascaded_mut().max_width = of(e.style.cascaded().max_width, inherited.width);
    copy.style.cascaded_mut().min_width = of(e.style.cascaded().min_width, inherited.width);
    copy.style.cascaded_mut().max_height = of(e.style.cascaded().max_height, h_base);
    copy.style.cascaded_mut().min_height = of(e.style.cascaded().min_height, h_base);
    copy
}

/// The measured item must see the same computed axes and inherited alignment as its holder.
pub(crate) fn inherit_item_metadata(style: &mut Computed, merged: &Computed, parent: &Computed) {
    style.parent_grid = merged.parent_grid;
    if merged.parent_grid == 0 {
        return;
    }
    style.rtl = merged.rtl;
    style.cb_wm = merged.cb_wm;
    if merged.align_self.is_none() {
        style.align_self = parent.align_items;
        style.align_self_last = parent.align_items_last;
        style.align_self_safe = parent.align_items_safe;
    }
    if merged.justify_self.is_none() {
        style.justify_self = parent.justify_items;
        style.justify_self_last = parent.justify_items_last;
        style.justify_self_safe = parent.justify_items_safe;
    }
}
