//! Формы детей (KidShape): рекурсивный замер вырезов каждого ребёнка.

use crate::dom::{Element, Node};
use crate::layout::float::block_like_float;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::line_shape::{basis_sized, inline_content};
use crate::render::out_of_flow;
use crate::style::values::value::Len;

pub(super) fn kid_shapes(
    c: &Element,
    depth: u8,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    cx: ShapeCx,
    kids: &Vec<&Node>,
    flex_col: bool,
    no_descent: bool,
) -> Option<
    Vec<(
        f32,
        f32,
        f32,
        Vec<(f32, f32)>,
        Vec<f32>,
        Vec<(f32, f32)>,
        bool,
        bool,
        f32,
    )>,
> {
    let inner: Option<Vec<KidShape>> = if depth == 0 || no_descent {
        None
    } else {
        kids.iter()
            .map(|n| match n {
                // Абсолют высоты стопке не даёт и разреза
                // не мешает: нулевая запись, а не отказ
                // от всей укладки (`out-of-flow-in-
                // multicolumn-*`, корень A2). Но НУЛЬ в
                // девятом поле означал бы, что его вовсе
                // нет во фрагментации, а css-position-3
                // §abspos-breaking требует обратного: «an
                // absolutely positioned box is positioned
                // relative to its containing block ignoring
                // any fragmentation breaks (as if the flow
                // were continuous). The box may
                // subsequently be broken over several
                // fragmentation containers». Значит
                // содержащий блок обязан ДОТЯНУТЬСЯ до его
                // низа — иначе колонок под него не
                // родится (Blink
                // `column_layout_algorithm.cc:1125`:
                // `actual_column_count +=
                // column_balancing_info.num_new_columns`).
                // Плавающий сюда не входит: он не
                // позиционированный, и содержащего блока
                // собой не задаёт.
                Node::Element(k) if out_of_flow(&k.style) => {
                    let abs = matches!(
                        k.style.position,
                        Some(crate::style::computed::Position::Absolute)
                    );
                    let reach = if abs {
                        // `top: 100vh` у страниц — от page area (эталоны
                        // `fixedpos-*` ставят копии `top: N00vh`; без этого
                        // досягаемость нулевая, лист один).
                        let top = px_or(&k.style.inset.top, false).unwrap_or(0.0);
                        // Собственная высота абсолюта — той
                        // же мерой: она уже включает дотяг
                        // ЕГО внепоточных потомков, и
                        // цепочка `abs > abs` складывается
                        // сама (`out-of-flow-in-multicolumn-
                        // 022/025`).
                        let own = shape_full(k, depth - 1, cx).map(|s| s.0).unwrap_or(0.0);
                        (top + own).max(0.0)
                    } else {
                        0.0
                    };
                    Some((
                        0.0,
                        0.0,
                        0.0,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        false,
                        false,
                        reach,
                    ))
                }
                Node::Element(k)
                    if !k.inline
                        && (k.style.position.is_none()
                            || k.style.position
                                == Some(crate::style::computed::Position::Relative))
                        && (k.style.float.unwrap_or(0) == 0 || block_like_float(&k.style)) =>
                {
                    // Главный размер элемента КОЛОНКИ flex — его `flex-basis`
                    // (css-flexbox-1 §9.2 шаг 3): `content` — по содержимому,
                    // а `height` при этом не действует; в точках — сама база.
                    // Контейнер `height: auto` свободного места не даёт, и
                    // гибкость базу не меняет (§9.7).
                    let based = if flex_col { basis_sized(c, k) } else { None };
                    let k = based.as_ref().unwrap_or(k);
                    shape_full(k, depth - 1, cx).map(|(h, mt, mb, cuts, forced, solid)| {
                        // Монолит-потомок — весь диапазон
                        // его высоты; иначе — его собственные
                        // монолиты.
                        let solid = if solid_box(k) && !(inline_content(k) && !cuts.is_empty()) {
                            vec![(0.0, h)]
                        } else {
                            solid
                        };
                        (
                            h,
                            mt,
                            mb,
                            cuts,
                            forced,
                            solid,
                            // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                            // передаётся коробке (css-break-4
                            // §break-propagation) — у страниц; колонки
                            // не трогаются (отдельный замер).
                            // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                            // передаётся коробке (css-break-3 §5.1
                            // break-propagation; Blink `InitialBreakBefore`)
                            // одинаково у страниц и у колонок: правило не
                            // про вид фрагментаинера. Гейт `cx.paged` был
                            // «пока не замерено» — `single-line-row-flex-
                            // fragmentation-016` с разрывом на внуке стоит
                            // красной ровно из-за него (проба
                            // `target/probe-9g/p-…-016.html` = 0.00).
                            edge_break(k, false),
                            edge_break(k, true),
                            // Дотяг внепоточных ЭТОГО потомка в
                            // поток родителя не переходит: у
                            // него свой содержащий блок.
                            0.0,
                        )
                    })
                }
                _ => None,
            })
            .collect()
    };
    inner
}

pub(super) type KidShape = (
    f32,
    f32,
    f32,
    Vec<(f32, f32)>,
    Vec<f32>,
    Vec<(f32, f32)>,
    bool,
    bool,
    f32,
);
