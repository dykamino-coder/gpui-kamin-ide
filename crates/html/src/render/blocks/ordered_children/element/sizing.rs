//! Основа flex и ratio/растяжение заменяемых элементов сетки.

use crate::dom::Node;
use crate::layout::options::RenderOpts;
use crate::style::computed::{Align, Computed, Display, FlexDir};
use crate::style::values::value::{Len, px_lossy_zero};

#[allow(clippy::too_many_arguments)]
pub(crate) fn item_sizing(
    mut e: crate::dom::Element,
    inherited: &Computed,
    _opts: &RenderOpts,
) -> Node {
    let positioned_out = matches!(
        e.style.cascaded().position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    );
    // `<canvas>` в сетке: атрибуты `width/height` — природный
    // размер и соотношение сторон, а не CSS-размер (HTML
    // §4.12.5, §15.3.10). Ось, растянутая выравниванием
    // (`stretch`, css-align-3 §6.1) или переносимая из
    // заданной автором другой оси (css-sizing-4 «transferred
    // size»), становится `auto`, соотношение — на коробку
    // (`replaced-element-011`, `grid-item-inline-contribution-*`,
    // `replaced-alignment-with-aspect-ratio-001`).
    if e.tag == "canvas"
        && matches!(
            inherited.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && !positioned_out
        && (e.style.cascaded().attr_sized.0 || e.style.cascaded().attr_sized.1)
    {
        let stretch = |own: Option<Align>, items: Option<Align>| {
            own == Some(Align::Stretch) || (own.is_none() && items == Some(Align::Stretch))
        };
        let sx = stretch(e.style.cascaded().justify_self, inherited.justify_items);
        let sy = stretch(e.style.cascaded().align_self, inherited.align_items);
        // Обе оси пришли из атрибутов, а явный `stretch` — ровно
        // у одной: вторая ось тоже `auto` и выводится из
        // растянутой через соотношение (css-sizing-4 «transferred
        // size»; Blink `length_utils.cc` — `kStretchExplicit` по
        // блочной оси включает соотношение для строчной). Раньше
        // она оставалась атрибутом, и taffy выводил из неё
        // растянутую: 10×10 вместо 100×100
        // (`replaced-alignment-with-aspect-ratio-001/002`).
        // Вертикальную сетку не трогаем: оси там переставлены.
        let both_attrs = e.style.cascaded().attr_sized.0 && e.style.cascaded().attr_sized.1;
        let transfer = both_attrs && sx != sy && !inherited.wm().vertical();
        let free_x =
            e.style.cascaded().attr_sized.0 && (sx || !e.style.cascaded().attr_sized.1 || transfer);
        let free_y =
            e.style.cascaded().attr_sized.1 && (sy || !e.style.cascaded().attr_sized.0 || transfer);
        if free_x || free_y {
            // Явный `stretch` по ОБЕИМ осям задаёт обе стороны
            // растяжением — соотношение не действует
            // (`-003.tentative`: 10×20 в области 100×100 давал
            // 100×200; то же правило — `grid-aspect-ratio-032/033`).
            if let (Some(Len::Px(w)), Some(Len::Px(h))) = (
                e.style.cascaded().attr_width,
                e.style.cascaded().attr_height,
            ) && h > 0.0
                && e.style.cascaded().aspect_ratio.is_none()
                && !(both_attrs && sx && sy)
            {
                e.style.cascaded_mut().aspect_ratio = Some(w / h);
            }
            // Нерастянутая ось при `normal` у коробки с
            // соотношением — `start` (css-grid-2 §6.6.1), иначе
            // taffy растянет её сам (`alignment.rs:122-128`: без
            // заданной ширины умолчание — `Stretch`) и выведет
            // растянутую из неё. Авторское значение не трогаем.
            if transfer {
                if sy
                    && e.style.cascaded().justify_self.is_none()
                    && inherited.justify_items.is_none()
                {
                    e.style.cascaded_mut().justify_self = Some(Align::Start);
                }
                if sx && e.style.cascaded().align_self.is_none() && inherited.align_items.is_none()
                {
                    e.style.cascaded_mut().align_self = Some(Align::Start);
                }
            }
            if free_x {
                e.style.cascaded_mut().width = None;
            }
            if free_y {
                e.style.cascaded_mut().height = None;
            }
        }
    }
    let ratio_ok = e
        .style
        .cascaded()
        .aspect_ratio
        .is_some_and(|r| r.is_finite() && r > 0.0);
    e.style.cascaded_mut().flex_item_ratio = ratio_ok
        && !positioned_out
        && matches!(
            inherited.display,
            Some(Display::Flex) | Some(Display::InlineFlex)
        );
    // `flex-basis` задаёт размер СОДЕРЖИМОГО (css-flexbox-1 §7.2.3:
    // «flex-basis determines the size of the content box, unless
    // otherwise specified such as by box-sizing»), а в раскладку
    // уходит внешний размер — как `width`/`height` в `apply`, основа
    // получает отбивку и рамку по ГЛАВНОЙ оси родителя
    // (`flexbox-mbp-horiz-*`, `flexbox-justify-content-horiz-002`).
    if matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && inherited.writing_mode.is_none()
        && e.style.cascaded().border_box != Some(true)
        && let Some(Len::Px(b)) = e.style.cascaded().flex_basis
    {
        let px_of = px_lossy_zero;
        let bd = e.style.cascaded().borders();
        let row = matches!(
            inherited.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        );
        let extra = if row {
            px_of(e.style.cascaded().padding.left)
                + px_of(e.style.cascaded().padding.right)
                + px_of(bd.left)
                + px_of(bd.right)
        } else {
            px_of(e.style.cascaded().padding.top)
                + px_of(e.style.cascaded().padding.bottom)
                + px_of(bd.top)
                + px_of(bd.bottom)
        };
        e.style.cascaded_mut().flex_basis = Some(Len::Px(b + extra));
    }
    // Элемент сетки с `aspect-ratio` при `normal` (css-grid-2
    // §6.6.1): «sized consistent with the size calculation
    // rules for block-level elements» — строчная ось заполняет
    // область (как stretch), а БЛОЧНАЯ идёт из соотношения, не
    // растягиваясь на ряд: там `start`
    // (`grid-aspect-ratio-001/007/010/038`). `start` и по строчной оси
    // не ставить: ломает `grid-aspect-ratio-018/038`.
    if ratio_ok
        && matches!(
            inherited.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && !positioned_out
        && inherited.writing_mode.is_none()
    {
        let auto_w = matches!(e.style.cascaded().width, None | Some(Len::Auto));
        let auto_h = matches!(e.style.cascaded().height, None | Some(Len::Auto));
        // Обе оси auto: строчная заполняет область, блочная — из
        // соотношения. Блочная определена: строчная — из
        // соотношения (CSS2 §10.3.2 для замещаемого с
        // соотношением; css-sizing-4 §5.1).
        if auto_w
            && !auto_h
            && e.style.cascaded().justify_self.is_none()
            && inherited.justify_items != Some(Align::Stretch)
        {
            e.style.cascaded_mut().justify_self = Some(Align::Start);
        }
        if auto_h
            && e.style.cascaded().align_self.is_none()
            && inherited.align_items != Some(Align::Stretch)
        {
            e.style.cascaded_mut().align_self = Some(Align::Start);
        }
    }
    // `flex-basis: content` — основа по содержимому, и
    // заданный ГЛАВНЫЙ размер при ней не действует. Какая ось
    // главная, знает только родитель, поэтому размер снимается
    // здесь, а не в стиле самого элемента.
    // У ЗАМЕЩАЕМОГО элемента содержимое — он сам, и его
    // размер задаёт собственный пиксель или атрибут: снимать
    // его нельзя, иначе `<canvas width=20>` схлопывается в
    // ноль (`flexbox-flex-basis-content-001a`).
    let own = matches!(
        e.tag.as_str(),
        "img" | "canvas" | "embed" | "iframe" | "video" | "object" | "svg"
    );
    if e.style.cascaded().basis_content == Some(true) {
        match inherited.flex_dir {
            Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                e.style.cascaded_mut().height =
                    own.then_some(e.style.cascaded().attr_height).flatten();
            }
            _ => {
                e.style.cascaded_mut().width =
                    own.then_some(e.style.cascaded().attr_width).flatten()
            }
        }
    }
    // Основа-ДОЛЯ у элемента колонки, чей контейнер не определён
    // по главной оси, — это `content` (css-flexbox-1 §7.2.3: «if
    // that containing block's size is indefinite, the used value
    // for flex-basis is content»; Blink `IsItemFlexBasisDefinite`).
    // Taffy не решает долю и падает на заданную высоту
    // (`flexbox.rs` `flex_basis.or(main_size)`): `flex: 0 0 0%;
    // height: 500px` давал 500 вместо содержимого 100
    // (`flex-basis-010`), а `flex: 1 1; height: 100px` делал
    // блок детей определённым, и `height: 100%` ребёнка
    // закрашивал красное (`percentage-heights-017/018`).
    if e.style.cascaded().flex_main_def == Some(false)
        && matches!(e.style.cascaded().flex_basis, Some(Len::Pct(_)))
    {
        e.style.cascaded_mut().flex_basis = None;
        e.style.cascaded_mut().height = own.then_some(e.style.cascaded().attr_height).flatten();
    }
    Node::Element(e)
}
