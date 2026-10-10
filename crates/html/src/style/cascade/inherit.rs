//! Наследование стиля.
// owner: B

use crate::style::computed::{Computed, TextAlign};
use crate::style::values::value::{Color, Len};
use crate::text::inline::{backdrop_root, bidi_controls, establishes_cb};

mod containing;
mod decorate;
mod explicit;
mod fonts;
mod paint;
mod sizes;
mod text_layout;
use containing::inherit_containing;
use decorate::decorate;
use explicit::inherit_explicit;
use fonts::inherit_fonts;
use paint::inherit_paint;
use sizes::inherit_sizes;
use text_layout::inherit_text_layout;

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09): `zoom` (css-viewport-1) как домножение
// использованных длин здесь, в `inherit`, плюс `zoom`/`zoom_eff` и `scale_px`
// в `Computed`. Срез 6315 пар (css-viewport + все пары с `zoom:` + весь
// CSS2): 5722 -> 4918, то есть **+354/-1158**. Потери — сплошь CSS2
// `background-*` (0.00 -> 8.3), таблицы, абсолюты; «приобретения» ложные:
// позеленели JS-тесты `insert-block-in-inlines-*`, у которых обе стороны
// сломались одинаково. Гейт `zoom_eff != 1.0` не удержал: домножение
// задело общий путь длин. Возвращаться только через отдельный проход после
// каскада, а не через самую горячую функцию крейта.
pub fn inherit(parent: &Computed, own: &Computed) -> Computed {
    inherit_stage(parent, own, true)
}

/// Document preparation propagates style; Filter Effects 1 §5 applies colors
/// only when the renderer builds the element, never while removing wrappers.
pub(crate) fn inherit_unpainted(parent: &Computed, own: &Computed) -> Computed {
    inherit_stage(parent, own, false)
}

fn inherit_stage(parent: &Computed, own: &Computed, paint_filter: bool) -> Computed {
    let mut c = own.clone();
    inherit_containing(parent, own, &mut c);
    inherit_sizes(parent, own, &mut c);
    inherit_explicit(parent, own, &mut c);
    inherit_fonts(parent, own, &mut c);
    inherit_text_layout(parent, own, &mut c);
    inherit_paint(parent, own, &mut c, paint_filter);
    // `em` считается от размера шрифта — а он известен только здесь, когда
    // наследование уже произошло. Раньше длина переводилась в точки при
    // разборе, по постоянным 16 точкам, и вложенные кегли не перемножались.
    // База `em` у собственного кегля — ВЫЧИСЛЕННЫЙ кегль родителя:
    // `font-size-adjust` «does not affect the size of em units».
    let parent_px = match parent.font_adjust_base {
        Some((px, _)) => px,
        None => match parent.font_size {
            Some(Len::Px(px)) => px,
            _ => 16.0,
        },
    };
    // Процент у размера шрифта — доля родительского кегля; в точках его надо
    // получить здесь, иначе абзац уходил в запасную ветку переноса (размер
    // «не такой, как у базового») и терял перенос по словам.
    // `larger`/`smaller`: шаг по таблице ключевых кеглей, если кегль
    // родителя в ней стоит (§15.7; таблица та же, что у слов в
    // `computed.rs`). Вне таблицы работает запасной `Len::Em` (1.2 и 5/6).
    if c.font_size_step != 0 {
        const TABLE: [f32; 8] = [9.0, 10.0, 13.0, 16.0, 18.0, 24.0, 32.0, 48.0];
        if let Some(i) = TABLE.iter().position(|t| (t - parent_px).abs() < 0.01) {
            let j = i as i32 + i32::from(c.font_size_step);
            if (0..TABLE.len() as i32).contains(&j) {
                c.font_size = Some(Len::Px(TABLE[j as usize]));
            }
        }
    }
    if let Some(Len::Pct(k)) = c.font_size {
        c.font_size = Some(Len::Px(k * parent_px));
    }
    c.resolve_em(parent_px);
    // Цвет рамки по умолчанию — ЦВЕТ ТЕКСТА: `border: solid 1px` без цвета
    // рисуется в браузере чёрной рамкой, а у нас не рисовалась вовсе —
    // коробка выходила без рамки, и эталоны переносов выглядели сломанными.
    // Известен цвет только здесь: он наследуемый, а рамка нет.
    let has_border = {
        let b = c.borders();
        [b.top, b.right, b.bottom, b.left]
            .iter()
            .any(|w| !matches!(w, None | Some(Len::Px(0.0))))
    };
    // Сторона с `currentColor` из бокового сокращения при ОБЩЕМ цвете рамки:
    // ей положен цвет текста, а не общий (§8.5.4; `border-shorthands-003`).
    // Прочие стороны получают общий цвет ЯВНО: единый цвет квада
    // (`apply::apply_paint`) смотрит только на заданные стороны и иначе
    // выкрасил бы их цветом помеченной. Без общего цвета пустой слот и так
    // даёт цвет текста.
    if c.border_color.is_some() && c.border_side_current.iter().any(|f| *f) {
        let current = c.color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        for i in 0..4 {
            if c.border_colors[i].is_none() {
                c.border_colors[i] = if c.border_side_current[i] {
                    Some(current)
                } else {
                    c.border_color
                };
            }
        }
    }
    if has_border && c.border_color.is_none() && c.border_colors.iter().all(Option::is_none) {
        c.border_color = c.color.or(Some(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }));
    }
    // `tab-size` в длине наследуется АБСОЛЮТНОЙ величиной: `5em` при кегле
    // 10px — это 50px и у ребёнка с кеглем 20px, а не его собственные 5em
    // (`tab-size-inheritance-001`).
    let own_px = match c.font_size {
        Some(Len::Px(px)) => px,
        _ => parent_px,
    };
    c.tab_size_len = match own.tab_size_len {
        Some(len) => Some(Len::Px(crate::text::metrics::spacing_px(
            Some(len),
            &c.font_family.clone().unwrap_or_default(),
            own_px,
        ))),
        None if own.tab_size.is_some() => None,
        None => parent.tab_size_len,
    };
    decorate(parent, own, &mut c, own_px);
    c
}
