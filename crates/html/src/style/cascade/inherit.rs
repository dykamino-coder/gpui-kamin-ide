//! Наследование стиля.
// owner: B

use crate::style::computed::{Computed, TextAlign};
use crate::style::values::value::{Color, Len};
use crate::text::inline::{backdrop_root, bidi_controls, establishes_cb};

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

pub(crate) fn inherit_stage(parent: &Computed, own: &Computed, paint_filter: bool) -> Computed {
    let mut c = own.clone();
    c.first_letter_excluded = own.first_letter_excluded || parent.first_letter_excluded;
    bidi_controls::resolve(parent, &mut c);
    c.cb_ancestor = parent.cb_ancestor || establishes_cb(parent);
    c.in_multicol = parent.in_multicol
        || ((parent.column_count.is_some()
            || parent.column_width.is_some()
            || parent.column_height.is_some())
            && !matches!(
                parent.display,
                Some(crate::style::computed::Display::Grid)
                    | Some(crate::style::computed::Display::InlineGrid)
                    | Some(crate::style::computed::Display::GridLanes)
                    | Some(crate::style::computed::Display::Flex)
                    | Some(crate::style::computed::Display::InlineFlex)
            ));
    c.backdrop_root_above = parent.backdrop_root_above || backdrop_root(parent);
    // filter-effects-2 §3 шаг 4: содержимое B — и его СОБСТВЕННЫЙ фон —
    // рисуется поверх уже отфильтрованной подложки. Фон коробки красит сама
    // коробка раньше детей, а канвас подложки — ребёнок; явный
    // `background-clip: border-box` уводит фон в слой-ребёнка
    // `render::clip_layer`, который идёт после декораций.
    if c.backdrop_color.is_some()
        && !c.backdrop_root_above
        && c.bg_clip.is_none()
        && (c.background.is_some() || c.gradient.is_some())
        && matches!(c.display, None | Some(crate::style::computed::Display::Block))
    {
        c.bg_clip = Some(crate::style::computed::BgClip::BorderBox);
    }
    // Ближайший содержащий блок абсолюта по `node_id` — ключ реестра рамок
    // `anchor::CB` (нужен `position-area`); корень даёт 0 = начальный
    // содержащий блок, окно.
    c.cb_node = if establishes_cb(parent) {
        parent.self_node
    } else {
        parent.cb_node
    };
    // Барьер содержащего блока для `position: fixed` — не только трансформ.
    // css-contain-1 §containment-layout п.1: обособление раскладки делает
    // элемент содержащим блоком для потомков и с `absolute`, И С `fixed`;
    // §containment-paint говорит то же про `contain: paint`. Для абсолюта
    // этот список уже полон (`establishes_cb`), а `fixed` признавал барьером
    // только `transform` и висел от окна вместо предка
    // (`contain-layout-containing-block-fixed-001`,
    // `contain-paint-containing-block-fixed-001`).
    c.transform_ancestor = parent.transform_ancestor
        || parent.transform.is_some()
        || parent.preserve_3d == Some(true)
        || parent.contain_layout == Some(true)
        || parent.contain_paint == Some(true)
        // css-will-change-1 §2.1: блок для `fixed` — и от обещанного свойства
        // (`will-change-fixpos-cb-*`, `-fixedpos-cb-*`).
        || parent.will_change & crate::style::computed::wc::CB_FIXED != 0;
    // `will-change: z-index` — контекст наложения ровно там, где `z-index`
    // действует: у позиционированной коробки и у элемента flex/grid
    // (css-flexbox-1 §5.4, css-grid-2 §6.2) — `-stacking-context-z-index-2/3`;
    // у простого блока — нет (`-z-index-4` зелёный и обязан остаться). Это не
    // домножение длин (★ выше): один битовый тест, страница без
    // `will-change` в ветку не заходит.
    if own.will_change & crate::style::computed::wc::STACK_Z != 0
        && (matches!(
            own.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
                | Some(crate::style::computed::Position::Sticky)
        ) || matches!(
            parent.display,
            Some(crate::style::computed::Display::Flex)
                | Some(crate::style::computed::Display::InlineFlex)
                | Some(crate::style::computed::Display::Grid)
                | Some(crate::style::computed::Display::InlineGrid)
        ))
    {
        c.will_change |= crate::style::computed::wc::STACK;
    }
    c.cb_rtl = parent.rtl == Some(true);
    // Внутри повёрнутого абзаца родитель — горизонтальный клон
    // (`render.rs: paragraph`, `horizontal.vertical = None`), и о вертикальном
    // письме содержащего блока говорит только `rotated_line`.
    c.cb_vertical = parent.vertical == Some(true) || parent.rotated_line == Some(true);
    c.cb_vertical_rl = parent.vertical_rl == Some(true);
    c.cb_sideways = parent.sideways == Some(true);
    // Относительный сдвиг строчного предка КОПИТСЯ вниз (§9.4.3: сдвиг несёт
    // с собой всё содержимое коробки). Куски вне потока его получали
    // (`shift_overlays`), а вложенные куски самой строки — нет: сдвиг
    // родителя терялся на первом же слиянии стилей.
    if c.rel_shift.is_none() {
        c.rel_shift = parent.rel_shift;
    }
    // §10.5: доля высоты считается только от ОПРЕДЕЛЁННОЙ высоты содержащего
    // блока. Определена она у корня (его блок — начальный), при высоте
    // родителя в точках, при доле от определённого деда и у абсолютной
    // коробки — та задаёт отсчёт сама.
    // Элемент гибкого контейнера, сетки и содержимое ячейки получают
    // определённую высоту от РАСКЛАДКИ (css-flexbox-1 §9.8 растяжение,
    // css-grid-2 дорожки, §17.5.3 ячейка) — признака у нас на это нет, и
    // считать их блок неопределённым нельзя: замерено CSS3 2355 -> 2348,
    // потери во `flexbox-definite-sizes-*` и `*-subgrid-*`.
    let laid_out_parent = matches!(
        parent.display,
        Some(crate::style::computed::Display::Flex)
            | Some(crate::style::computed::Display::InlineFlex)
            | Some(crate::style::computed::Display::Grid)
            | Some(crate::style::computed::Display::InlineGrid)
            | Some(crate::style::computed::Display::GridLanes)
            | Some(crate::style::computed::Display::TableCell)
    );
    // Растяжение передаётся дальше: у растянутой коробки высота от полосы, и
    // для её потомков блок определён (`column-align-items-005`).
    c.stretched = laid_out_parent && !matches!(own.height, Some(crate::style::values::value::Len::Px(_)));
    // Высота, выведенная из `aspect-ratio` при определённой ширине, —
    // определённая (css-sizing-4 §5.1: «the resulting size is definite if
    // its input sizes are also definite»): проценты детей решаются от неё
    // (`percentage-resolution-001/002`, `flex-aspect-ratio-047/048`).
    let ratio_height = parent.aspect_ratio.is_some()
        && matches!(parent.height, None | Some(crate::style::values::value::Len::Auto))
        && matches!(parent.width, Some(crate::style::values::value::Len::Px(_)));
    // Ребёнок элемента КОЛОНКИ, у которой главный размер неопределён:
    // блок неопределён, и доля высоты ведёт себя как `auto`
    // (css-flexbox-1 §9.8 п.1-2; `percentage-heights-016/020`), кроме
    // случая, когда сам элемент имеет высоту в точках или основу в точках.
    let indefinite_column_item = parent.flex_main_def == Some(false)
        && !matches!(parent.height, Some(crate::style::values::value::Len::Px(_)))
        && !matches!(parent.flex_basis, Some(crate::style::values::value::Len::Px(_)))
        // Соотношение сторон САМО даёт главный размер: строчную ось
        // элемента колонки решает контейнер, и высота выводится из неё
        // (Blink `AspectRatioProvidesBlockMainSize`;
        // `flex-aspect-ratio-032/033` — ширина у элемента даже не задана).
        && parent.aspect_ratio.is_none();
    c.cb_height_def = !indefinite_column_item
        && (parent.root_box
        || laid_out_parent
        || parent.stretched
        || ratio_height
        || match parent.height {
            Some(crate::style::values::value::Len::Px(_)) => true,
            // Доля высоты у абсолютной/фиксированной коробки решается ВСЕГДА
            // (§10.5: оговорка «not absolutely positioned»; её же держит гейт
            // `apply.rs`), значит её высота для детей определена — как у
            // Blink, где внепоточная коробка отдаёт детям свой блочный размер.
            // Прежде `.modal {position: fixed; height: stretch}` передавал
            // ложный признак от `body`, и `height: 100%` цепочки гас
            // (`intrinsic-height-abspos-stretch-percentage-child`: 25 %).
            Some(crate::style::values::value::Len::Pct(_)) => {
                parent.cb_height_def
                    || matches!(
                        parent.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    )
            }
            _ => matches!(
                parent.position,
                Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
            ),
        });
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
            None | Some(L::Auto) if !out_of_flow && !tabular && !c.root_box => parent.quirk_pct_base,
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
            Some(crate::style::values::value::Len::Em(k)) | Some(crate::style::values::value::Len::Pct(k)) => k * parent_font,
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
            Some(crate::style::values::value::Len::Em(k)) | Some(crate::style::values::value::Len::Pct(k)) => k * font,
            _ => {
                let f = crate::text::metrics::normal_line(&family);
                if f > 0.0 { f * font } else { 1.2 * font }
            }
        };
        let fix = |l: &mut Option<crate::style::values::value::Len>| match *l {
            Some(crate::style::values::value::Len::Lh(k)) => *l = Some(crate::style::values::value::Len::Px(k * line)),
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
    // Ненаследуемые свойства со словом `inherit`: значение родителя берётся
    // целиком (§6.2.1). Разбор их слотов слово не выражает — там оно давало
    // умолчание или роняло объявление.
    if own.inherit_bits != 0 {
        use crate::style::computed::inh;
        let on = |b: u32| own.inherit_bits & b != 0;
        if on(inh::BG_REPEAT) {
            c.bg_repeat = parent.bg_repeat;
        }
        if on(inh::Z_INDEX) {
            c.z_index = parent.z_index;
        }
        if own.inherit_bits
            & (inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S | inh::OUTLINE_O)
            != 0
        {
            let from = parent.outline.unwrap_or_default();
            let mut o = c.outline.unwrap_or_default();
            if on(inh::OUTLINE_W) {
                o.width = from.width;
            }
            if on(inh::OUTLINE_C) {
                // `currentColor` вычисляется В СЕБЯ (css-color-4 §resolving):
                // у родителя он хранится ПУСТЫМ слотом, и наследовать надо
                // пустоту — цвет возьмётся от СВОЕГО текста, а не от чужого
                // (`outline-019`: родитель красный, ребёнок зелёный).
                o.color = from.color;
            }
            if on(inh::OUTLINE_S) {
                o.style = from.style;
            }
            if on(inh::OUTLINE_O) {
                o.offset = from.offset;
                // Метка `inset` — часть значения сдвига и наследуется с ним.
                o.inset = from.inset;
            }
            c.outline = Some(o);
        }
        if on(inh::DISPLAY) {
            c.display = parent.display;
            c.inline_display = parent.inline_display;
        }
        if on(inh::BG_IMAGE) {
            c.bg_image = parent.bg_image.clone();
            c.gradient = parent.gradient.clone();
            c.gradient_raw = parent.gradient_raw.clone();
        }
        if on(inh::BG_POS) {
            c.bg_pos = parent.bg_pos;
        }
        if on(inh::CLIP) {
            c.clip_rect = parent.clip_rect;
        }
        if on(inh::BG_ORIGIN) {
            c.bg_origin = parent.bg_origin;
        }
        if on(inh::BG_CLIP) {
            c.bg_clip = parent.bg_clip;
        }
        if on(inh::BG_SIZE) {
            c.bg_size = parent.bg_size;
        }
        if on(inh::TRANSFORM) {
            c.transform = parent.transform;
        }
        if on(inh::TRANSFORM_ORIGIN) {
            c.transform_origin = parent.transform_origin;
            c.transform_origin_px = parent.transform_origin_px;
            c.transform_origin_z = parent.transform_origin_z;
        }
        if on(inh::CLIP_MARGIN) {
            c.clip_margin = parent.clip_margin;
            c.clip_margin_box = parent.clip_margin_box;
        }
    }
    for (i, on) in own.margin_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.margin.top = parent.margin.top,
            1 => c.margin.right = parent.margin.right,
            2 => c.margin.bottom = parent.margin.bottom,
            _ => c.margin.left = parent.margin.left,
        }
    }
    for (i, on) in own.padding_inherit_side.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.padding.top = parent.padding.top,
            1 => c.padding.right = parent.padding.right,
            2 => c.padding.bottom = parent.padding.bottom,
            _ => c.padding.left = parent.padding.left,
        }
    }
    if own.width_inherit {
        c.width = parent.width;
    }
    if own.height_inherit {
        c.height = parent.height;
    }
    for (i, on) in own.minmax_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.min_width = parent.min_width,
            1 => c.min_height = parent.min_height,
            2 => c.max_width = parent.max_width,
            _ => c.max_height = parent.max_height,
        }
    }
    for (i, on) in own.inset_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.inset.top = parent.inset.top,
            1 => c.inset.right = parent.inset.right,
            2 => c.inset.bottom = parent.inset.bottom,
            _ => c.inset.left = parent.inset.left,
        }
    }
    if own.background_inherit {
        c.background = parent.background;
        c.background_rcs = own.background_rcs.clone().or(parent.background_rcs.clone());
        if own.background_all_inherit {
            c.bg_image = parent.bg_image.clone();
            c.bg_repeat = parent.bg_repeat;
            c.bg_pos = parent.bg_pos;
            c.bg_size = parent.bg_size;
            c.bg_fixed = parent.bg_fixed;
            c.gradient = parent.gradient.clone();
            c.gradient_raw = parent.gradient_raw.clone();
        }
    }
    // Относительный цвет решается ЗДЕСЬ: только теперь известен цвет самого
    // элемента. Функция остаётся в поле — её унаследуют дети и решат своим
    // цветом заново.
    if let Some(expr) = c.background_rcs.clone() {
        let current = c.color.unwrap_or(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        if let Some(resolved) = crate::style::values::color_space::resolve_relative(&expr, current) {
            c.background = Some(resolved);
        }
    }
    // `c` — клон `own`, но `lh` в собственном кегле уже решён выше от строки
    // РОДИТЕЛЯ (css-values-4 §6.1.1, `from_parent`). Брать `own` здесь значило
    // вернуть сырое `Len::Lh` — кегль терялся (`lh-unit-002`).
    // Детям уходит ВЫЧИСЛЕННЫЙ кегль, а не подогнанный `font-size-adjust`:
    // «child elements inherit the computed font-size value (otherwise, the
    // effect of font-size-adjust would compound)» (css-fonts-4 §2.5).
    c.font_size = c.font_size.or(match parent.font_adjust_base {
        Some((px, _)) => Some(Len::Px(px)),
        None => parent.font_size,
    });
    // Кегль НОЛЬ вешает набор намертво (DirectWrite-цикл: `font: 0 Ahem` из
    // vars-font-shorthand-001 замораживал страницу навсегда) — клэмп к
    // микроскопическому: визуально то же «ничего», формулы живы.
    if let Some(Len::Px(v)) = c.font_size {
        if v <= 0.0 {
            c.font_size = Some(Len::Px(0.01));
        }
    }
    crate::style::computed::font_weight::inherit(&mut c, parent, own);
    c.italic = own.italic.or(parent.italic);
    c.oblique = own.oblique.or(parent.oblique);
    c.underline = own.underline.or(parent.underline);
    c.line_through = own.line_through.or(parent.line_through);
    // То же для высоты строки: `line-height: 2lh` уже переведён в точки от
    // строки родителя; сырое `Lh` уходило в `apply.rs` как `relative(2)` от
    // СВОЕГО кегля (`lh-unit-001`: 84 вместо 100).
    // У подогнанного родителя числовой `line-height` переведён в точки ЕГО
    // вычисленным кеглем; ребёнок наследует сам множитель.
    c.line_height = c.line_height.or(match parent.font_adjust_base {
        Some((_, lh)) => lh,
        None => parent.line_height,
    });
    c.text_align = own.text_align.or(parent.text_align);
    c.no_justify = own.no_justify.or(parent.no_justify);
    c.ruby_justify = own.ruby_justify.or(parent.ruby_justify);
    c.justify_chars = own.justify_chars.or(parent.justify_chars);
    c.ruby_unit = own.ruby_unit || parent.ruby_unit;
    c.text_align_last = own.text_align_last.or(parent.text_align_last);
    c.hanging = own.hanging.or(parent.hanging);
    c.monospace = own.monospace.or(parent.monospace);
    crate::style::computed::font_family::inherit(&mut c, own, parent);
    c.nowrap = own.nowrap.or(parent.nowrap);
    c.orphans = own.orphans.or(parent.orphans);
    c.widows = own.widows.or(parent.widows);
    // Направление письма наследуется: `writing-mode` ставят на `body`, а ось
    // потока обязана смениться у КАЖДОГО вложенного блока — иначе вертикально
    // становится только сам `body`, а его дети снова текут вниз.
    c.vertical = own.vertical.or(parent.vertical);
    c.vertical_rl = own.vertical_rl.or(parent.vertical_rl);
    c.sideways = own.sideways.or(parent.sideways);
    c.combine_upright = own.combine_upright.or(parent.combine_upright);
    c.rotated_line = own.rotated_line.or(parent.rotated_line);
    c.ortho_limit = own.ortho_limit.or(parent.ortho_limit);
    c.orthogonal_scrollport = parent.orthogonal_scrollport;
    // An absolutely positioned box sizes its inline axis against its own
    // containing block (CSS 2.1 §10.3.7 / §10.6.4 shrink-to-fit), never with
    // the in-flow inline measure of the paragraph it was written in.
    c.orthogonal_inline = if matches!(
        own.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) {
        None
    } else {
        parent.orthogonal_inline
    };
    c.wrap_anywhere = own.wrap_anywhere.or(parent.wrap_anywhere);
    c.word_space_char = own.word_space_char.or(parent.word_space_char);
    c.autospace_alpha = own.autospace_alpha.or(parent.autospace_alpha);
    c.autospace_numeric = own.autospace_numeric.or(parent.autospace_numeric);
    // Сдвиг НЕ наследуется: он принадлежит своему куску, иначе надстрочный
    // знак поднимал бы весь текст после себя.
    c.vertical_shift = own.vertical_shift;
    c.vertical_shift_px = own.vertical_shift_px;
    c.vertical_shift_len = own.vertical_shift_len;
    c.vertical_align_text = own.vertical_align_text;
    // Кегль родителя нужен `text-top`/`text-bottom`: край куска равняется по
    // ЕГО текстовой области.
    c.vertical_align_base = match parent.font_size {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => own.vertical_align_base,
    };
    c.upright = own.upright.or(parent.upright);
    c.text_sideways = own.text_sideways.or(parent.text_sideways);
    // Сетка-родитель и её письмо: у вертикальной сетки оси выравнивания
    // элемента переставляются (`apply.rs`), а по оси x идут группы базовых.
    c.parent_grid = match parent.display {
        Some(crate::style::computed::Display::Grid) | Some(crate::style::computed::Display::InlineGrid) => {
            match (parent.vertical == Some(true), parent.vertical_rl == Some(true)) {
                (false, _) => 1,
                (true, false) => 2,
                (true, true) => 3,
            }
        }
        _ => 0,
    };
    c.parent_lanes = parent.display == Some(crate::style::computed::Display::GridLanes);
    c.parent_subgrid = c.parent_grid != 0 && (parent.subgrid_cols || parent.subgrid_rows);
    c.parent_flex_grid = matches!(
        parent.display,
        Some(crate::style::computed::Display::Flex)
            | Some(crate::style::computed::Display::InlineFlex)
            | Some(crate::style::computed::Display::Grid)
            | Some(crate::style::computed::Display::InlineGrid)
            | Some(crate::style::computed::Display::GridLanes)
    );
    // Наследуемые текстовые свойства из второй волны разбора. Без них
    // `text-transform` на контейнере не доходил до вложенного текста —
    // а в разметке его ставят именно на контейнер.
    c.letter_spacing = own.letter_spacing.or(parent.letter_spacing);
    c.word_spacing = own.word_spacing.or(parent.word_spacing);
    // Сторона подписи таблицы наследуется (CSS 2.1: caption-side inherited) —
    // читается потом С САМОГО заголовка (caption-side-applies-to-012..015:
    // значение на ряде до заголовка не доходит).
    c.caption_bottom = own.caption_bottom.or(parent.caption_bottom);
    c.text_transform = own.text_transform.or(parent.text_transform);
    // Добавки — часть того же значения: своё объявление заменяет их целиком.
    c.text_transform_flags = if own.text_transform.is_some() {
        own.text_transform_flags
    } else {
        parent.text_transform_flags
    };
    c.text_indent = own.text_indent.or(parent.text_indent);
    // `text-box-edge` наследуется (css-inline-3 §text-box-edge, Inherited:
    // yes); срез берёт край у корневой строчной коробки СТРОКИ, то есть у
    // блока, которому она принадлежит (`text-box-trim-accumulation-001…003`).
    if !own.text_box_edge_set {
        c.text_box_over = parent.text_box_over;
        c.text_box_under = parent.text_box_under;
        c.text_box_edge_set = parent.text_box_edge_set;
    }
    c.text_indent_each_line = own.text_indent_each_line.or(parent.text_indent_each_line);
    c.text_indent_hanging = own.text_indent_hanging.or(parent.text_indent_hanging);
    c.break_anywhere = own.break_anywhere.or(parent.break_anywhere);
    c.break_word = own.break_word.or(parent.break_word);
    c.balance_lines = own.balance_lines.or(parent.balance_lines);
    c.break_anywhere_strict = own.break_anywhere_strict.or(parent.break_anywhere_strict);
    c.line_break_loose = own.line_break_loose.or(parent.line_break_loose);
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): снимать `align-self` у коробки, чей
    // родитель не гибкий контейнер и не сетка (css-align-3 §6.2 «does not
    // apply to block-level boxes»). По спеке верно, но наш блок собран
    // колонкой flex, и на `align_self` держатся собственные приёмы сборки:
    // соотношение сторон блока (`blocks()` ставит `Align::Start`), обтекание,
    // сжатие стола. Узкий срез выравнивания (2661 пара) дал +8/−1, а ПОЛНЫЙ
    // свод v18 -> v19 — минус ~50: вся семья `float-applies-to-*` (0.00 ->
    // 3.84), `floats-002/025/147`, `clear-float-001/003`,
    // `block-aspect-ratio-002/015/016/018/043/047`, девять
    // `shape-outside-*-border-radius-*`, `absolute-replaced-width-020/034`.
    // Возвращать вместе с признаком «значение авторское», чтобы приёмы сборки
    // гейт не задевал (`self-align-start-end-flex-001` — цель правки).
    // `normal` у элемента ГИБКОГО контейнера = `stretch` (§6.2), а не
    // «пусто»: пустое значение брало `align-items` родителя
    // (`self-align-normal-flex`).
    if own.align_self_normal
        && matches!(
            parent.display,
            Some(crate::style::computed::Display::Flex) | Some(crate::style::computed::Display::InlineFlex)
        )
    {
        c.align_self = Some(crate::style::computed::Align::Stretch);
    }
    // `self-start`/`self-end` меряются по письму САМОГО элемента (css-align-3
    // §6.2). Значение уже физическое (начало = левый край при ltr), поэтому
    // зеркалим ровно тогда, когда строчная ось элемента смотрит в другую
    // сторону, чем у родителя: `flexbox-align-self-vert-002` даёт элементам
    // `direction: rtl` внутри ltr-колонки и ждёт `self-start` СПРАВА.
    // Вертикальное письмо здесь НЕ зеркалим намеренно: там ось строки уже
    // переставлена поворотом — это территория wm-скаута.
    // Письмо элемента — ДЕЙСТВУЮЩЕЕ (своё или унаследованное): незаданное
    // `direction` у элемента в rtl-колонке — тоже rtl, и зеркалить нечего
    // (поперечную ось rtl-колонки разворачивает сама раскладка,
    // `apply.rs`: `flex_cross_reverse` / `flip`).
    if c.parent_grid == 0 && own.align_self_own_axis && own.rtl.or(parent.rtl).unwrap_or(false) != parent.rtl.unwrap_or(false) {
        c.align_self = match c.align_self {
            Some(crate::style::computed::Align::Start) => Some(crate::style::computed::Align::End),
            Some(crate::style::computed::Align::End) => Some(crate::style::computed::Align::Start),
            other => other,
        };
    }
    // `start`/`end` (и `self-*`) меряются по ПИСЬМУ, а раскладка знает только
    // гибкие концы (`apply::to_items` → `FlexStart`/`FlexEnd`), которые
    // АВТОРСКИЙ `wrap-reverse` родителя переворачивает (css-align-3 §6.1,
    // css-flexbox-1 §5.2). Концы письма меняются местами ровно тогда: taffy
    // развернёт их обратно. Переворот поперёк письма (`apply.rs`: `flip`
    // через тот же `WrapReverse`) здесь не участвует — он виден раскладке и
    // для `flex-*`, и для `start` одинаково (`self-align-start-end-flex-001`).
    if !own.align_self_flex_kw
        && parent.flex_wrap_reverse == Some(true)
        && matches!(
            parent.display,
            Some(crate::style::computed::Display::Flex) | Some(crate::style::computed::Display::InlineFlex)
        )
    {
        c.align_self = match c.align_self {
            Some(crate::style::computed::Align::Start) => Some(crate::style::computed::Align::End),
            Some(crate::style::computed::Align::End) => Some(crate::style::computed::Align::Start),
            other => other,
        };
    }
    c.text_emphasis = own.text_emphasis.clone().or(parent.text_emphasis.clone());
    c.emphasis_under = own.emphasis_under || parent.emphasis_under;
    c.emphasis_color = own.emphasis_color.or(parent.emphasis_color);
    // css-ruby-1 §4.1/§4.3: оба свойства наследуемые.
    c.ruby_under = own.ruby_under.or(parent.ruby_under);
    c.ruby_align = own.ruby_align.or(parent.ruby_align);
    c.ruby_overhang = own.ruby_overhang.or(parent.ruby_overhang);
    c.ruby_merge = own.ruby_merge.or(parent.ruby_merge);
    // `image-orientation` наследуется (css-images-3 §5.4, «Inherited: yes»):
    // в наборе его ставят на `body`, а действует он на каждой картинке.
    c.image_orient_none = own.image_orient_none.or(parent.image_orient_none);
    c.font_synth = (
        own.font_synth.0.or(parent.font_synth.0),
        own.font_synth.1.or(parent.font_synth.1),
        own.font_synth.2.or(parent.font_synth.2),
    );
    c.keep_all = own.keep_all.or(parent.keep_all);
    c.hyphens_auto = own.hyphens_auto.or(parent.hyphens_auto);
    c.lang = own.lang.clone().or(parent.lang.clone());
    c.break_after_spaces = own.break_after_spaces.or(parent.break_after_spaces);
    c.hyphenate = own.hyphenate.or(parent.hyphenate);
    c.tab_size = if own.tab_size_len.is_some() {
        None
    } else {
        own.tab_size.or(parent.tab_size)
    };
    c.list_style_type = own
        .list_style_type
        .clone()
        .or_else(|| parent.list_style_type.clone());
    c.list_style_inside = own.list_style_inside.or(parent.list_style_inside);
    c.vertical_align = own.vertical_align.or(parent.vertical_align);
    c.font_stretch = own.font_stretch.or(parent.font_stretch);
    // `font-size-adjust` наследуется значением; подгонку каждый элемент
    // считает сам, по СВОЕМУ шрифту (`Computed::resolve_em`).
    c.font_size_adjust = own.font_size_adjust.or(parent.font_size_adjust);
    c.no_select = own.no_select.or(parent.no_select);
    c.pointer_events_none = own.pointer_events_none.or(parent.pointer_events_none);
    c.line_clamp = own.line_clamp.or(parent.line_clamp);
    // `block-ellipsis` наследуется (css-overflow-4 §block-ellipsis).
    c.clamp_mark = own.clamp_mark.clone().or_else(|| parent.clamp_mark.clone());
    c.clamp_legacy = own.clamp_legacy.or(parent.clamp_legacy);
    // Гейтовые флаги -webkit-box НЕ наследуются: пара display+orient
    // обязана стоять на самом элементе.
    c.webkit_box = own.webkit_box;
    c.webkit_box_vertical = own.webkit_box_vertical.or(parent.webkit_box_vertical);
    // Фон строчного бокса идёт вниз как текстовое свойство: он принадлежит
    // строке, а не коробке, и вложенный `<b>` внутри подсветки обязан его
    // сохранить.
    c.inline_bg = own.inline_bg.or(parent.inline_bg);
    c.inline_border = own.inline_border.or(parent.inline_border);
    c.inline_pad = own.inline_pad.or(parent.inline_pad);
    c.inline_radius = own.inline_radius.or(parent.inline_radius);
    if c.font_features.is_empty() {
        c.font_features = parent.font_features.clone();
    }
    c.font_kerning = own.font_kerning.or(parent.font_kerning);
    c.font_alternates = own
        .font_alternates
        .clone()
        .or_else(|| parent.font_alternates.clone());
    // `font-feature-settings` наследуется своим значением независимо от
    // `font-variant-*` ребёнка (css-fonts-4 §6.12: `font-variant: none` «does
    // not reset … font-feature-settings»).
    c.font_settings = own
        .font_settings
        .clone()
        .or_else(|| parent.font_settings.clone());
    c.text_shadow = if own.text_shadow_none {
        None
    } else {
        own.text_shadow.or(parent.text_shadow)
    };
    // Хвост списка идёт вместе с первой тенью: своя запись — свой хвост,
    // унаследованная — хвост родителя (css-text-decor-3: `text-shadow`
    // наследуется списком целиком).
    if own.text_shadow_none {
        c.text_shadow_rest.clear();
    } else if own.text_shadow.is_none() {
        c.text_shadow_rest = parent.text_shadow_rest.clone();
    }
    c.rtl = own.rtl.or(parent.rtl);
    // `text-align: start|end` — края СТРОКИ, и разворачиваются они в момент
    // ОТРИСОВКИ, а не здесь: иначе левый край, вычисленный для тела страницы,
    // достаётся по наследству и вложенному блоку справа налево (`physical`
    // зовёт `lines::align_for`). Умолчание CSS — `start`.
    c.text_align = Some(c.text_align.unwrap_or(TextAlign::Start));
    // Фильтр в CSS красит элемент И ВСЁ поддерево. Наследуем его сами и
    // применяем к собственным цветам потомка: раньше фильтр действовал только
    // на узел, где написан, и дети оставались цветными.
    if c.filter.is_none() {
        c.filter = parent.filter;
    }
    // ЕДИНСТВЕННАЯ точка окраски фильтром (каскад цвета не трогает).
    // Красится только ВОЗНИКШЕЕ на этом узле: унаследованный цвет уже
    // покрашен предком — повторная окраска давала f^N по поколениям.
    if paint_filter && let Some(f) = c.filter {
        if own.background.is_some() {
            c.background = c.background.map(|col| f.apply(col));
        }
        if own.color.is_some() || own.filter.is_some() && parent.color.is_none() {
            c.color = c.color.map(|col| f.apply(col));
        }
        if own.border_color.is_some() || own.border_color_is_current {
            c.border_color = c.border_color.map(|col| f.apply(col));
        }
        for (side, own_side) in c.border_colors.iter_mut().zip(own.border_colors.iter()) {
            if own_side.is_some() {
                *side = side.map(|col| f.apply(col));
            }
        }
        if own.gradient.is_some()
            && let Some(g) = c.gradient.as_mut()
        {
            g.from = f.apply(g.from);
            g.to = f.apply(g.to);
            for stop in g.stops.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
        }
        // Растровый градиент (`conic`, `repeating-*`) живёт строкой в
        // `bg_image`: его стопы красятся в самой записи
        // (`filter-function-repeating-*-ref`: `filter: invert(1)` на фоне).
        if own.bg_image.is_some()
            && let Some(img) = c.bg_image.as_deref()
            && crate::style::computed::gradient_as_raster(img)
        {
            c.bg_image = Some(crate::style::computed::filter_gradient_text(img, &f));
        }
        if !own.shadows.is_empty() {
            for sh in c.shadows.iter_mut() {
                sh.color = f.apply(sh.color);
            }
        }
    }
    // `filter: drop-shadow()` у коробки со СПЛОШНЫМ фоном: силуэт такой
    // коробки — border-box со скруглением, и тень фильтра (filter-effects-1
    // §dropshadowEquivalent: размытая альфа входа, сдвиг, цвет — ПОД входом)
    // совпадает с внешней box-shadow без разлёта. Картинку поддерева так не
    // выразить — только коробку; повторное слияние тень не удваивает.
    // Длина размытия у `drop-shadow()` — это σ (filter-effects-1
    // §funcdef-filter-drop-shadow: «standard deviation»), а у `box-shadow`
    // радиус = 2σ (css-backgrounds-3 §box-shadow) — в список внешних теней
    // она идёт удвоенной, чтобы после деления в `apply::apply_paint` σ
    // осталась своей.
    if let Some(sh) = c.drop_shadow
        && c.background.is_some_and(|b| b.a >= 1.0)
    {
        let as_box = crate::style::computed::Shadow {
            blur: sh.blur * 2.0,
            ..sh
        };
        if !c.shadows.contains(&as_box) {
            c.shadows.push(as_box);
        }
    }
    // `background-clip: text` со СПЛОШНОЙ заливкой (css-backgrounds-4
    // §background-clip): фон виден только под глифами элемента и его
    // поточных и плавающих потомков, а сам текст красится ПОВЕРХ фона своим
    // цветом. Для одноцветного непрозрачного фона это ровно «цвет текста
    // поверх заливки» — маска глифов не нужна, а подчёркивания, многоточие и
    // знаки выделения цветом `currentColor` получают тот же цвет сами.
    // Смешивается только ВОЗНИКШЕЕ на узле (как у фильтра выше): унаследованный
    // цвет уже смешан предком. Внепоточные потомки в геометрию текста не входят
    // (`clip-text-out-of-flow-child`) и получают несмешанный цвет обратно.
    let black = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let out_of_flow = matches!(
        own.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    );
    if let Some(fill) = crate::paint::background::text_clip_fill(&c) {
        c.text_clip_raw = c.color;
        c.text_clip_fill = Some(fill);
        c.color = Some(crate::paint::background::over(c.color.unwrap_or(black), fill));
    } else if parent.text_clip_fill.is_some() && out_of_flow {
        c.text_clip_fill = None;
        c.text_clip_raw = None;
        if own.color.is_none() {
            c.color = parent.text_clip_raw;
        }
    } else if let Some(fill) = parent.text_clip_fill {
        c.text_clip_fill = Some(fill);
        if own.color.is_some() {
            c.text_clip_raw = c.color;
            c.color = Some(crate::paint::background::over(c.color.unwrap_or(black), fill));
        } else {
            c.text_clip_raw = parent.text_clip_raw;
        }
    }
    // Наследуемые по CSS, но забытые прежде: без них `white-space: pre` на
    // контейнере не доходил до вложенного текста, а маркер, курсор и зазор
    // ячеек не доставались детям.
    c.preserve_newlines = own.preserve_newlines.or(parent.preserve_newlines);
    c.keep_spaces = own.keep_spaces.or(parent.keep_spaces);
    c.hidden = own.hidden.or(parent.hidden);
    c.cursor = own.cursor.clone().or(parent.cursor.clone());
    c.no_marker = own.no_marker.or(parent.no_marker);
    c.border_collapse = own.border_collapse.or(parent.border_collapse);
    c.empty_cells_hide = own.empty_cells_hide.or(parent.empty_cells_hide);
    c.border_spacing = own.border_spacing.or(parent.border_spacing);
    c.caret_color = own.caret_color.or(parent.caret_color);
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

/// Украшения текста (css-text-decor-3 §2.1): линии не наследуются, а
/// РАСПРОСТРАНЯЮТСЯ на всех потомков в потоке, кроме атомарных строчных
/// (`inline-block`, `inline-table`…) и вынесенных из потока (флоаты,
/// абсолюты); цвет, рисунок, толщина и метрики — от украшающей коробки.
/// `display: contents` коробки не даёт и своих линий не кладёт.
pub(crate) fn decorate(parent: &Computed, own: &Computed, c: &mut Computed, own_px: f32) {
    use crate::style::computed::Display as D;
    use crate::style::computed::Position as P;
    use crate::style::computed::{DECOR_THROUGH, DECOR_UNDER, Decor, DecorFont, DecorLen};
    // Семейство для `ch`/`ex` — как у `resolve_em`: родовое `monospace`
    // имени не даёт, а меряться должно тем шрифтом, которым набран текст.
    let family = c.font_family.clone().unwrap_or_else(|| {
        if c.monospace == Some(true) {
            crate::text::metrics::mono_family_for(c.lang.as_deref()).to_string()
        } else {
            String::new()
        }
    });
    let resolve = |l: DecorLen| match l {
        DecorLen::Raw(raw) => crate::text::metrics::fallback_len_px(raw, &family, own_px)
            .map_or(DecorLen::Auto, DecorLen::Px),
        other => other,
    };
    // Наследуемое смещение — вычисленной длиной (доля остаётся долей).
    c.underline_offset = own.underline_offset.map(resolve).or(parent.underline_offset);
    c.underline_pos = own.underline_pos.or(parent.underline_pos);
    let over_lang = c.lang.as_deref().is_some_and(|l| {
        let l = l.to_ascii_lowercase();
        ["ja", "ko", "mn"].iter().any(|p| l == *p || l.starts_with(&format!("{p}-")))
    });
    let blocked = matches!(own.position, Some(P::Absolute) | Some(P::Fixed))
        || own.float.is_some_and(|f| f != 0)
        || matches!(
            own.display,
            Some(D::InlineBlock | D::InlineTable | D::InlineFlex | D::InlineGrid)
        );
    c.skip_ink = own.skip_ink.or(parent.skip_ink);
    c.skip_spaces = own.skip_spaces.or(parent.skip_spaces);
    c.decors = if blocked { Vec::new() } else { parent.decors.clone() };
    // Линии, пришедшие в блок, рисуются на его анонимной строчной коробке
    // (css-text-decor-3 §2.1, пример 1): метрики и положение — от шрифта
    // этого блока (`text-decoration-subelements-004`).
    let block = match own.display {
        None => own.block_tag && own.inline_display != Some(true),
        Some(D::Contents) => false,
        Some(_) => true,
    };
    if block {
        for d in c.decors.iter_mut() {
            d.font = DecorFont {
                family: c.font_family.clone(),
                monospace: c.monospace,
                weight: c.font_weight,
                italic: c.italic,
                stretch: c.font_stretch,
                size: own_px,
            };
            d.position = c.underline_pos.unwrap_or(0);
            d.over_lang = over_lang;
        }
    }
    if let Some(lines) = own.td_lines.filter(|l| *l != 0)
        && own.display != Some(D::Contents)
    {
        let black = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let thickness = match own.td_thickness.map(resolve).unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            t => t,
        };
        let offset = match c.underline_offset.unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            DecorLen::Px(v) => DecorLen::Px(v),
            _ => DecorLen::Auto,
        };
        let inset = match own.td_inset {
            Some(None) => None,
            Some(Some(pair)) => Some(pair.map(resolve)),
            None => Some([DecorLen::Px(0.0); 2]),
        };
        let d = Decor {
            lines,
            style: own.td_style.unwrap_or_default(),
            color: own.td_color.or(c.color).unwrap_or(black),
            thickness,
            offset,
            position: c.underline_pos.unwrap_or(0),
            inset,
            clone: own.bdb_clone,
            over_lang,
            font: DecorFont {
                family: c.font_family.clone(),
                monospace: c.monospace,
                weight: c.font_weight,
                italic: c.italic,
                stretch: c.font_stretch,
                size: own_px,
            },
        };
        // Повторное слияние того же стиля (`inherit(merged, own)`) не должно
        // класть линию второй раз.
        if c.decors.last() != Some(&d) {
            c.decors.push(d);
        }
    }
    c.underline = Some(c.decors.iter().any(|d| d.lines & DECOR_UNDER != 0));
    c.line_through = Some(c.decors.iter().any(|d| d.lines & DECOR_THROUGH != 0));
}
