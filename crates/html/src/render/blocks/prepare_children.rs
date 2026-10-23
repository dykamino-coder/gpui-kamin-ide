//! Анонимные коробки, разделение inline/block и схлопывание полей перед рендером.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
use crate::dom::Node;
use crate::layout::block::margin_height;
use crate::layout::block::margins::{CollapseInputs, collapse_margins};
use crate::layout::block::reorder::reorder;
use crate::layout::box_kind::in_flow;
use crate::layout::box_kind::inline_level_box;
use crate::layout::box_kind::replaced_tag;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::relative::hoist_inset_abs;
use crate::layout::table::anon::wrap_anon_tables;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Align, BoxStyle, Computed, Display};
use crate::style::values::value::{Len, px_strict};

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_children(
    nodes: &[Node],
    inherited: &BoxStyle,
    opts: &RenderOpts,
    _cell_bfc: bool,
) -> (Vec<Node>, bool, bool) {
    let vcx = opts.cx.vcx();
    let stripped: Vec<Node>;
    let nodes = if nodes.iter().any(
        |n| matches!(n, Node::Element(e) if e.style.computed().skip_content == Some(true) && !e.children.is_empty()),
    ) {
        stripped = nodes
            .iter()
            .map(|n| match n {
                Node::Element(e) if e.style.computed().skip_content == Some(true) => {
                    let mut copy = e.clone();
                    copy.children.clear();
                    Node::Element(copy)
                }
                other => other.clone(),
            })
            .collect();
        &stripped
    } else {
        nodes
    };
    // `order` в CSS работает ТОЛЬКО внутри гибкого контейнера и сетки; в
    // обычном потоке он не значит ничего. Раньше сортировались дети любого
    // родителя — блоки меняли порядок там, где браузер их не трогает.
    // Барьер `fixed` — не только СВОЙ трансформ родителя, но и его
    // `contain: layout|paint` (css-contain-2 §3.2 п.5, §3.3: «establishes …
    // a fixed positioning containing block»). `transform_ancestor` родителя
    // несёт лишь ЕГО предков (`style::cascade::inherit`), поэтому прямой ребёнок
    // обособленной коробки уходил в слой окна и садился в угол экрана
    // (`contain-layout-007`, `contain-paint-010`), а внук — нет
    // (`contain-*-containing-block-fixed-001` = 0.00). Список совпадает с
    // `fixed_cb_box` — расхождение он прямо запрещает.
    // Обещанное свойство, дающее блок для `fixed` (css-will-change-1 §2.1), —
    // тот же барьер, что `transform` самого родителя (`will-change-fixpos-cb-*`).
    let under_tf = inherited.transform_ancestor
        || inherited.transform.is_some()
        || inherited.contain_layout == Some(true)
        || inherited.contain_paint == Some(true)
        || crate::style::computed::filter_contains_fixed(inherited)
        || inherited.will_change & crate::style::computed::wc::CB_FIXED != 0;
    let ordered_context = matches!(
        inherited.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            // Поток лунок — сеточный контекст: схлопывания отступов нет
            // (css-grid-3), и `order` действует.
            | Some(Display::GridLanes)
    );
    // Схлопывание вертикальных отступов есть ТОЛЬКО в обычном потоке: в
    // гибком контейнере и сетке CSS его запрещает, а мы схлопывали везде —
    // элементы ряда съезжали друг к другу против браузера.
    // Блок внутри строчного разрывает его на анонимные коробки (CSS 2.1
    // §9.2.1.1) — разбиение идёт ДО схлопывания полей: вынесенный блок
    // обязан схлопнуть свои поля с новыми соседями. В гибком контейнере и
    // сетке разрыва нет вовсе: там дети блокифицируются, и куски уехали бы
    // по чужим дорожкам.
    let split = if ordered_context {
        nodes.to_vec()
    } else {
        // Анонимная таблица вокруг ПРОГОНА табличных братьев (§17.2.1 шаг 3)
        // — до разбиения блока в строчном и до схлопывания полей, как это
        // делает и сборщик дерева в браузере.
        crate::layout::block::inline_splits::split_block_in_inline_dir(
            &hoist_inset_abs(&wrap_anon_tables(nodes), inherited, vcx),
            inherited.wm().rtl(),
        )
    };
    // Не снимать АВТОРСКИЙ `align-self` у блока в обычном потоке здесь, в
    // начале `blocks()` («до внутренних постановщиков»): замещаемому
    // абсолюту `align_self` нужен ещё ДО `blocks()`
    // (`absolute-replaced-width-020`, `left-offset-003`,
    // `left-offset-percentage-001`, `anchor-position-inline-005/-006`), хотя
    // `align-self-013`, `flexbox-align-self-vert-001`, `-horiz-002` этого
    // ждут. Вернуть можно только с явным признаком «значение авторское»
    // в `Computed`.
    // §10.3.3: у блока в потоке с `width: auto` боковое `auto`-поле
    // используется НУЛЁМ, а коробка занимает всю ширину. У нас блок — гибкая
    // колонка, и любое auto-поле на поперечной оси отменяет растяжение до
    // дорожки: абзац сжимался по содержимому и уезжал к краю.
    let split = if ordered_context {
        split
    } else {
        split
            .into_iter()
            .map(|n| match n {
                // `justify-self` блока в потоке (css-align-3 §6.1 «Block-Level
                // Boxes»): не-`normal`/`stretch` значение меряет коробку с
                // `width: auto` по содержимому (fit-content — обёртка
                // `content_sized`), auto-поля имеют приоритет над
                // выравниванием; без auto-полей выравнивание выражается ими же
                // (флекс-колонка блока их исполняет). `left`/`right` разбор
                // уже свёл к `start`/`end`; сторона — по письму родителя
                // (`justify-self-auto-margins-2`: `margin: auto` центрирует
                // 100 в 200). Таблица и замещаемый размер по содержимому уже
                // имеют — им только поля.
                Node::Element(mut e)
                    if in_flow(&e.style.computed())
                        && !inline_level_box(&e)
                        && !inherited.wm().vertical()
                        && matches!(
                            e.style.computed().justify_self,
                            Some(Align::Start) | Some(Align::Center) | Some(Align::End)
                        ) =>
                {
                    let auto = |l: Option<Len>| l == Some(Len::Auto);
                    let table = e.tag == "table" || e.style.computed().display == Some(Display::Table);
                    if matches!(e.style.computed().width, None | Some(Len::Auto))
                        && !table
                        && !replaced_tag(&e)
                    {
                        e.style.used_mut().width = Some(Len::FitContent);
                    }
                    if !auto(e.style.computed().margin.left) && !auto(e.style.computed().margin.right) {
                        let rtl = inherited.wm().rtl();
                        match (e.style.computed().justify_self, rtl) {
                            (Some(Align::Center), _) => {
                                e.style.used_mut().margin.left = Some(Len::Auto);
                                e.style.used_mut().margin.right = Some(Len::Auto);
                            }
                            (Some(Align::End), false) | (Some(Align::Start), true) => {
                                e.style.used_mut().margin.left = Some(Len::Auto);
                            }
                            _ => e.style.used_mut().margin.right = Some(Len::Auto),
                        }
                    }
                    Node::Element(e)
                }
                Node::Element(mut e)
                    if in_flow(&e.style.computed())
                        // Orthogonal auto block size fits content; its margins
                        // position it in the containing block (WM 4 section 7.3).
                        && (e.style.cascaded().vertical_decl().or(inherited.vertical_decl()) == Some(true))
                            == (inherited.wm().vertical())
                        && matches!(e.style.computed().width, None | Some(Len::Auto))
                        && (e.style.computed().margin.left == Some(Len::Auto)
                            || e.style.computed().margin.right == Some(Len::Auto)) =>
                {
                    if e.style.computed().margin.left == Some(Len::Auto) {
                        e.style.used_mut().margin.left = Some(Len::Px(0.0));
                    }
                    if e.style.computed().margin.right == Some(Len::Auto) {
                        e.style.used_mut().margin.right = Some(Len::Px(0.0));
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    let nodes: &[Node] = &split;
    let collapsed = if ordered_context {
        reorder(nodes.to_vec())
    } else {
        // Схлопывание идёт ДО наследования стилей, поэтому кегль уровня
        // передаётся отдельно: `margin: 1em` без своего `font-size` меряется
        // от родительского.
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => MEDIUM_FONT_SIZE_PX,
        };
        // Ширина содержащего блока для ПРОЦЕНТНЫХ полей (§8.3: проценты полей
        // считаются от ширины содержащего блока, схлопывание — по уже
        // разрешённым значениям). Известна только заданная в точках.
        let cb_w = px_strict(inherited.width);
        // Определённость высоты блока для ДОЛЕЙ высоты детей — тем же
        // предикатом, что у слитого стиля (`style::cascade::inherit`): он зависит
        // только от родителя, а сырой стиль ребёнка признака ещё не несёт.
        let cb_h_def = inherit(inherited, &Computed::default(), vcx).cb_height_def;
        let vcx = vcx.with_collapse(CollapseInputs {
            font: base,
            width: cb_w,
            height_def: cb_h_def,
        });
        let mut out = collapse_margins(
            nodes,
            matches!(
                inherited.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ),
            opts.cx.constraints.cb_width,
            vcx,
        );
        margin_height::zero_float_blocks(&mut out, inherited, vcx);
        out
    };
    (collapsed, ordered_context, under_tf)
}
