//! inherit_stage, этап содержащего блока: bidi-управление, признаки cb_* (многоколонность, подложка, трансформ, письмо), растяжение и определённость высоты.

use super::*;

pub(super) fn inherit_containing(parent: &Computed, own: &Computed, c: &mut Computed) {
    c.first_letter_excluded = own.first_letter_excluded || parent.first_letter_excluded;
    bidi_controls::resolve(parent, c);
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
        && matches!(
            c.display,
            None | Some(crate::style::computed::Display::Block)
        )
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
    c.stretched =
        laid_out_parent && !matches!(own.height, Some(crate::style::values::value::Len::Px(_)));
    // Высота, выведенная из `aspect-ratio` при определённой ширине, —
    // определённая (css-sizing-4 §5.1: «the resulting size is definite if
    // its input sizes are also definite»): проценты детей решаются от неё
    // (`percentage-resolution-001/002`, `flex-aspect-ratio-047/048`).
    let ratio_height = parent.aspect_ratio.is_some()
        && matches!(
            parent.height,
            None | Some(crate::style::values::value::Len::Auto)
        )
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
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                ),
            });
}
