//! inherit_stage, этап размеров: процентная высота в режиме quirks, цвет, длины от кегля родителя, inherit у теней, рамок, отступов, выравнивания.

use super::*;

pub(super) fn inherit_sizes(parent: &Computed, own: &Computed, c: &mut Computed) {
    // Quirks Mode §3.5 «The percentage height calculation quirk»: в режиме
    // quirks содержащий блок для ДОЛИ высоты ищется циклом — предки с
    // `height: auto` пропускаются, пока не найдётся предок с заданной
    // высотой, абсолютный или табличный (тогда он и есть опора). Сама
    // коробка с долей и табличный `display` квирку не подлежат. Без него
    // `<canvas style="height:100%">` в `div` без высоты внутри флоата
    // высотой 100 схлопывался в ноль (`intrinsic-percent-replaced-002/006`,
    // `float-percentage-resolution-quirks-mode`). Blink:
    // `LayoutBox::ContainingBlockLogicalHeightForPercentageResolution`
    // (`SkipContainingBlockForPercentHeightCalculation`).
    if crate::style::select::quirks() {
        use crate::style::computed::Display as D;
        use crate::style::computed::Position as P;
        use crate::style::values::value::Len as L;
        let out_of_flow = matches!(own.position, Some(P::Absolute) | Some(P::Fixed));
        let tabular = matches!(
            own.display,
            Some(D::Table | D::InlineTable | D::TableCell | D::TableRow | D::TableRowGroup)
        );
        if let (Some(L::Pct(k)), false, false, Some(base)) =
            (own.height, c.cb_height_def, tabular, parent.quirk_pct_base)
        {
            c.height = Some(L::Px(k * base));
        }
        c.quirk_pct_base = match c.height {
            Some(L::Px(h)) => Some(h),
            // Доля от определённого блока — тоже опора: флоат `height: 50%`
            // в контейнере 200 даёт потомкам 100 (`intrinsic-percent-
            // replaced-003/004`).
            Some(L::Pct(k)) if !tabular => match parent.height {
                Some(L::Px(h)) => Some(k * h),
                _ => parent.quirk_pct_base.map(|b| k * b),
            },
            None | Some(L::Auto) if !out_of_flow && !tabular && !c.root_box => {
                parent.quirk_pct_base
            }
            _ => None,
        };
    }
    c.color = own.color.or(parent.color);
    // `background-color: inherit` переносит вычисленное значение родителя —
    // вместе с нерешённой относительной функцией (css-color-5 §4.1).
    // Единица `lh` разрешается ЗДЕСЬ: высота строки известна после каскада.
    {
        // css-values-4 §6.1.4, оговорка про font-affecting properties:
        // «when ''lh'' or ''rlh'' units are used in the value of the
        // 'line-height' property or font-affecting properties on the element
        // they refer to, they resolve against the computed 'line-height' and
        // font metrics of the PARENT element». То есть `line-height: 2lh` и
        // `font-size: 2lh` меряются строкой РОДИТЕЛЯ, а не своей: иначе
        // выходит круговая зависимость. Для всего остального (`height: 1lh`)
        // базой остаётся своя строка — та же оговорка, скобка в конце абзаца.
        let parent_font = match parent.font_size {
            Some(crate::style::values::value::Len::Px(v)) => v,
            _ => 16.0,
        };
        let parent_family = parent.font_family.clone().unwrap_or_else(|| {
            if parent.monospace == Some(true) {
                crate::text::metrics::mono_family_for(parent.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        let parent_line = match parent.line_height {
            Some(crate::style::values::value::Len::Px(v)) => v,
            Some(crate::style::values::value::Len::Em(k))
            | Some(crate::style::values::value::Len::Pct(k)) => k * parent_font,
            _ => {
                let f = crate::text::metrics::normal_line(&parent_family);
                if f > 0.0 {
                    f * parent_font
                } else {
                    1.2 * parent_font
                }
            }
        };
        // `c` — это `own.clone()`: слияние с родителем ещё впереди, значит в
        // `c.line_height`/`c.font_size` лежит ровно то, что задано НА ЭТОМ
        // элементе. Незаданное поле — `None`, и ветка молчит.
        let from_parent = |l: &mut Option<crate::style::values::value::Len>| match *l {
            Some(crate::style::values::value::Len::Lh(k)) => {
                *l = Some(crate::style::values::value::Len::Px(k * parent_line))
            }
            Some(crate::style::values::value::Len::LhPx(k, add)) => {
                *l = Some(crate::style::values::value::Len::Px(k * parent_line + add))
            }
            _ => {}
        };
        from_parent(&mut c.line_height);
        from_parent(&mut c.font_size);
        // `lh` — вычисленный `line-height` САМОГО элемента, а `line-height`
        // (как кегль и гарнитура) НАСЛЕДУЕТСЯ: незаданное на элементе берётся у
        // родителя. Ребёнок `height: 3lh` внутри `font: 16px / 32px` обязан
        // выйти 96, а не 3 × normal(16) ≈ 55 (`line-clamp-auto-035`: блок
        // с `height: 3lh` не доставал до потолка и не прятался).
        let font = match c.font_size {
            Some(crate::style::values::value::Len::Px(v)) => v,
            None => parent_font,
            _ => 16.0,
        };
        // `line-height: normal` — доля кегля ПО МЕТРИКАМ шрифта, а не
        // постоянные 1.2: у `lh`-единицы иначе выходила чужая высота строки
        // (`line-clamp-auto-*` меряют высоту в `lh`).
        let family = c
            .font_family
            .clone()
            .or_else(|| parent.font_family.clone())
            .unwrap_or_else(|| {
                if c.monospace.or(parent.monospace) == Some(true) {
                    crate::text::metrics::mono_family_for(c.lang.as_deref()).to_string()
                } else {
                    String::new()
                }
            });
        let line = match c.line_height.or(parent.line_height) {
            Some(crate::style::values::value::Len::Px(v)) => v,
            Some(crate::style::values::value::Len::Em(k))
            | Some(crate::style::values::value::Len::Pct(k)) => k * font,
            _ => {
                let f = crate::text::metrics::normal_line(&family);
                if f > 0.0 { f * font } else { 1.2 * font }
            }
        };
        let fix = |l: &mut Option<crate::style::values::value::Len>| match *l {
            Some(crate::style::values::value::Len::Lh(k)) => {
                *l = Some(crate::style::values::value::Len::Px(k * line))
            }
            Some(crate::style::values::value::Len::LhPx(k, add)) => {
                *l = Some(crate::style::values::value::Len::Px(k * line + add))
            }
            _ => {}
        };
        fix(&mut c.width);
        fix(&mut c.height);
        fix(&mut c.min_width);
        fix(&mut c.min_height);
        fix(&mut c.max_width);
        fix(&mut c.max_height);
        // `background-size: 100px 1lh` — та же единица и та же база
        // (`lh-unit-same-element-*`). Чинится ЗДЕСЬ, а не в `background.rs`:
        // там доступен только запасной кегль 16 (`fallback_len_px` даёт
        // 1.2 × 16 = 19.2 — ровно то, что видно на снимке), а сплошной
        // перевод единиц в самом `background.rs` уже ЗАМЕРЕН в минус
        // (★ `background.rs:2577`, CSS2 4614 → 4610).
        if let crate::style::computed::BgSize::Fixed(w, h) = &mut c.bg_size {
            fix(w);
            fix(h);
        }
    }
    // `box-shadow: inherit` копирует ВЫЧИСЛЕННУЮ тень родителя — вместе с
    // нерешённым `currentColor` (метка отрицательной альфы): решает её
    // отрисовка цветом СВОЕГО элемента.
    if own.shadow_inherit {
        c.shadows = parent.shadows.clone();
        c.inset_shadows = parent.inset_shadows.clone();
    }
    if own.border_inherit {
        c.border_width = parent.border_width;
        c.border_color = parent.border_color.or(parent.color);
        c.border_colors = parent.border_colors;
    }
    // `inherit` по сторонам и по частям рамки. Толщина без рисунка ничего не
    // рисует, поэтому вместе с ней переносится и он: у наследующей стороны
    // своего `border-style` обычно нет.
    for i in 0..4 {
        if own.border_inherit_w[i] {
            match i {
                0 => c.border_width.top = parent.border_width.top,
                1 => c.border_width.right = parent.border_width.right,
                2 => c.border_width.bottom = parent.border_width.bottom,
                _ => c.border_width.left = parent.border_width.left,
            }
        }
        if own.border_inherit_s[i] {
            c.border_visible[i] = parent.border_visible[i];
            c.border_side_styles[i] = parent.border_side_styles[i];
            c.border_dashed = parent.border_dashed;
            c.border_dotted = parent.border_dotted;
        }
        if own.border_inherit_c[i] {
            // Начальное значение `border-color` — `currentColor`, и по
            // css-color-3 наследуется оно КЛЮЧЕВЫМ СЛОВОМ: у родителя, своего
            // цвета рамки не задавшего, наследуется само слово, а решает его
            // цвет РЕБЁНКА (`border-color-011`).
            c.border_colors[i] = parent.border_colors[i].or(parent.border_color).or(c.color);
        }
    }
    if own.padding_inherit {
        c.padding = parent.padding;
    }
    // `inherit` у ненаследуемых выравниваний (css-cascade-4 §7.3.1): значение
    // родителя целиком, с его `safe`/`last` (`place-items: inherit` во
    // вложенной сетке — `grid-self-alignment-baseline-with-grid-001`).
    if own.align_inherit != 0 {
        use crate::style::computed::ainh;
        let on = |b: u8| own.align_inherit & b != 0;
        if on(ainh::ALIGN_ITEMS) {
            c.align_items = parent.align_items;
            c.align_items_safe = parent.align_items_safe;
            c.align_items_last = parent.align_items_last;
        }
        if on(ainh::JUSTIFY_ITEMS) {
            c.justify_items = parent.justify_items;
            c.justify_items_safe = parent.justify_items_safe;
            c.justify_items_last = parent.justify_items_last;
        }
        if on(ainh::ALIGN_CONTENT) {
            c.align_content = parent.align_content;
            c.align_content_safe = parent.align_content_safe;
            c.align_content_block = parent.align_content_block;
        }
        if on(ainh::JUSTIFY_CONTENT) {
            c.justify_content = parent.justify_content;
            c.justify_content_safe = parent.justify_content_safe;
        }
        if on(ainh::JUSTIFY_SELF) {
            c.justify_self = parent.justify_self;
            c.justify_self_safe = parent.justify_self_safe;
            c.justify_self_last = parent.justify_self_last;
            c.justify_self_physical = parent.justify_self_physical;
            c.justify_self_normal = parent.justify_self_normal;
            c.justify_self_own_axis = parent.justify_self_own_axis;
        }
    }
}
