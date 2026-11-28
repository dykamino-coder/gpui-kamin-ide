//! The prepared child list of a block container (`prepare`): box-tree fixups, margin
//! collapsing, first-line and first-letter routes, float hosts and the paint order of
//! positioned children. It reads the width only through tracked reads, so a container
//! keeps it across frames (`layout::reuse::FlowReuse::child_plan`).

use super::*;

/// What `blocks_flow` builds the children of a container from.
pub(super) struct ChildPlan {
    pub(super) nodes: Vec<Node>,
    /// Indices in `nodes` before which the pending text run closes.
    pub(super) run_breaks: Vec<usize>,
    pub(super) letter_scope: first_letter_scope::Scope,
    pub(super) ordered_context: bool,
    pub(super) under_tf: bool,
    /// A child is sticky: the flow starts with its probe.
    pub(super) sticky: bool,
}

/// The scalars of the context `prepare` reads besides the container's style and nodes.
pub(super) fn inputs(sizes: &BoxDimensions, opts: &RenderOpts, cell_bfc: bool) -> [u32; 5] {
    let bits = |v: Option<f32>| v.map_or(u32::MAX, f32::to_bits);
    [
        bits(crate::style::values::value::px_strict(sizes.preferred[0])),
        bits(opts.cx.constraints.avail_h),
        u32::from(cell_bfc),
        opts.base_size().to_bits(),
        u32::from(opts.cx.vcx().clearance_possible),
    ]
}

pub(super) fn prepare(
    nodes: &[Node],
    inherited: &BoxStyle,
    sizes: &BoxDimensions,
    opts: &RenderOpts,
    cell_bfc: bool,
) -> ChildPlan {
    let vcx = opts.cx.vcx();
    // `content-visibility: hidden`: содержимое пропускается целиком
    // (css-contain-2 §4) — коробка остаётся, детей нет.
    let (collapsed, ordered_context, under_tf) =
        prepare_children(nodes, inherited, sizes, opts, cell_bfc);
    // Плавающий блок и выравнивание по базовой линии на элементе гибкого
    // контейнера или сетки НЕ действуют — так велит CSS. Без этого правила
    // `float: right` на элементе ряда выкидывал его из раскладки родителя.
    let collapsed = ordered_children(collapsed, ordered_context, inherited, opts);
    let flex_ctx = matches!(
        inherited.reset_box_layout.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let letter_scope = first_letter_scope::Scope::new(&collapsed, inherited);
    // Буквица `initial-letter` расшивается в плавающий узел ДО обтекания —
    // дальше её ведёт `wrap_floats` наравне с авторскими флоатами. В гибком
    // контейнере и сетке `::first-letter` не действует — там не трогаем.
    let collapsed = if ordered_context {
        collapsed
    } else {
        initial_letter_float(
            first_letter_descendants::route(
                first_line_descendants::route(collapsed, inherited),
                inherited,
            ),
            inherited,
            opts,
        )
    };
    // §8.3.1: поле первого ребёнка примыкает к верхнему полю содержащего
    // блока, только если того не отделяют ни рамка, ни отбивка и он не
    // заводит своего контекста форматирования.
    let cb_top_open = !own_context_style(inherited)
        && zero_len(inherited.reset_box_layout.padding.top)
        && zero_len(inherited.borders().top);
    // Измеряемый бандовый хост (`band_flow.rs`) — только в БЛОЧНОМ контейнере
    // горизонтального письма: в гибком и сетке `float` не
    // действует (css-flexbox-1 §3, css-grid-1 §6.1).
    // Хост работает и в вертикальном письме: план в логических осях,
    // перевод в физику при сборке. Float sides are line-relative (left =
    // line-left), independent of `direction` (css-writing-modes-4 §6.3), so
    // RTL hosts use the same plan; paragraphs handle RTL within the bands.
    // `sideways-lr` uses the same logical bands; physical holders and shape
    // profiles start at the bottom line edge (css-writing-modes-4 §6.3).
    // Page stacks still need the measured host's break opportunities before
    // they can fragment it (CSS Break 3 section 4.1). Deferred clamp children
    // retain the counting scope, including the float's independent BFC.
    let measured_ok = !flex_ctx
        && !matches!(
            inherited.reset_box_layout.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && (!inherited.wm().block_rl() || inherited.wm().vertical());
    let collapsed = replaced_used_style::inline_nodes(collapsed, || opts.cx.constraints.avail_w());
    let collapsed = by_layer(
        wrap_floats(
            collapsed,
            inherited,
            cb_top_open,
            match inherited.inh.font.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            },
            measured_ok,
            own_context_style(inherited) || inherited.reset_flex.flex_item,
            cell_bfc,
            || opts.cx.constraints.cb_width(),
            opts.cx.constraints.avail_h,
            vcx,
        ),
        flex_ctx,
    );
    // Блок мы изображаем гибкой колонкой, а её дети по умолчанию сжимаются —
    // в обычном потоке этого нет: ребёнок выше родителя обязан вылезти, а не
    // ужаться. Поэтому в потоке сжатие детям выключается, если разметка не
    // просила обратного.
    let flex_context = matches!(
        inherited.reset_box_layout.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let collapsed = flow_children(collapsed, ordered_context, inherited, opts, flex_context);
    // §9.9 шаг 8: позиционированная коробка рисуется ПОВЕРХ содержимого
    // потока, а порядок краски у нас — порядок детей. В обычном потоке это
    // делает верхний слой, но в сетке и гибком контейнере он выключен
    // (`!ordered_context` у гейтов ниже), и абсолютный ребёнок оказывался под
    // соседями. Здесь его достаточно переставить в конец: место он берёт не
    // из потока (в раскладку сетки такой ребёнок не входит), поэтому
    // перестановка меняет только краску.
    // Вынесенный абсолют всё же РВЁТ прогон текста: каждая непрерывная
    // последовательность текстовых детей — свой анонимный элемент
    // (css-flexbox-1 §4, css-grid-2 §6.1), а абсолютный ребёнок в неё не
    // входит. После перестановки куски «Two » и «lines» оказывались соседями
    // и склеивались в один абзац (`anonymous-flex-item-004`,
    // `anonymous-grid-item-001`). `run_breaks` — индексы в новом списке, перед
    // которыми накопленный абзац закрывается.
    let mut run_breaks: Vec<usize> = vec![];
    let collapsed: Vec<Node> = if ordered_context {
        let in_flow = |n: &Node| match n {
            Node::Element(e) => {
                e.style.used().reset_positioned.position
                    != Some(crate::style::computed::Position::Absolute)
                    || e.style
                        .used()
                        .reset_positioned
                        .z_index
                        .is_some_and(|z| z < 0)
            }
            Node::Text(_) => true,
        };
        let mut flow: Vec<Node> = vec![];
        let mut over: Vec<Node> = vec![];
        for n in collapsed {
            if in_flow(&n) {
                flow.push(n);
            } else {
                if run_breaks.last() != Some(&flow.len()) {
                    run_breaks.push(flow.len());
                }
                over.push(n);
            }
        }
        flow.into_iter().chain(over).collect()
    } else {
        collapsed
    };

    let sticky = collapsed.iter().any(|n| match n {
        Node::Element(e) => {
            e.style.used().reset_positioned.position
                == Some(crate::style::computed::Position::Sticky)
        }
        _ => false,
    });
    ChildPlan {
        nodes: collapsed,
        run_breaks,
        letter_scope,
        ordered_context,
        under_tf,
        sticky,
    }
}
