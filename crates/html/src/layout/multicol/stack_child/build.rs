//! Построение копии ребёнка для фрагмента колонки (build_copy).

use super::{copy_kids, copy_whole_box, frag_box_div};
use crate::dom::Element;
use crate::layout::build_contracts::Effect;
use crate::layout::fragment::clone::clone_fragment;
use crate::layout::grid::place_named_areas;
use crate::layout::multicol::gap_rules::gap_rule_spec;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::stacking::fixed_cb_layer_box;
use crate::style::computed::{BoxStyle, Display};
use gpui::{IntoElement, ParentElement};

/// Не строить копию через `element(&copy, &merged,
/// opts)` ради сетки и гибкого контейнера внутри
/// стопки (`grid-container-fragmentation-*`): общий
/// путь элемента кладёт слои и выносит абсолюты
/// иначе, чем ждёт стопка, — рамки, тени,
/// `break-between-avoid-*`, `fieldset` ломаются.
/// Возвращать узкой веткой только для сетки.
/// Линейки промежутков (css-gaps-1) у копии
/// фрагмента: слой строится ТОЛЬКО при заданном
/// стиле линейки (`gap_rule_spec`), как в
/// `element()`, — ни одна старая пара сюда не
/// попадает. Буфер проб — СВОЙ на копию: пробы
/// всех копий одного узла иначе сливаются в один
/// буфер, первая копия забирает всё (`take`) и
/// строит дорожки по смеси поднятых на `from`
/// копий. Отрезки красятся в координатах полной
/// раскладки копии, маска колонки (`ColumnStack`,
/// `with_content_mask`) режет их вместе с
/// содержимым — вид `slice` css-break-3 §4.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_copy(
    opts: &RenderOpts,
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    frag_geom: &[(f32, f32)],
    dec: Option<(f32, f32)>,
    merged: &BoxStyle,
    copy: &Element,
    h: f32,
    over: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: &BoxStyle,
    whole: bool,
    first: bool,
    part: usize,
    band: bool,
) -> gpui::AnyElement {
    let vcx = opts.cx.vcx();
    let build = &opts.cx.contracts.build;
    // `box-decoration-break: clone`: копия — САМ
    // фрагмент (`clone_fragment`), и корень, и
    // наследуемый стиль берутся у НЕГО. Со стилем
    // ИСХОДНОЙ коробки каждая копия выходила полной
    // коробкой (`clone-007`: лишний квадрат 100×100,
    // `clone-026`: вылет 240×25 у каждой). НЕпоследний
    // фрагмент режется по съеденному содержимому,
    // последний — по концу видимого переполнения
    // (`over`, `clone-002`). Лишние копии (фрагментов
    // меньше, чем копий) не ставятся и остаются
    // исходной коробкой.
    let frag = match (dec, frag_geom.get(part)) {
        (Some((dt, db)), Some(&(from, fh))) => {
            let clip = frag_geom
                .get(part + 1)
                .map_or((over - dt - db - from).max(fh - dt - db), |n| n.0 - from);
            Some(clone_fragment(copy, dt, db, from, fh, clip))
        }
        _ => None,
    };
    let frag_inner = frag.as_ref().map(|f| box_style(merged, &f.style, vcx));
    let src: &Element = frag.as_ref().unwrap_or(copy);
    let src_inner: &BoxStyle = frag_inner.as_ref().unwrap_or(inner);
    let mut kids = copy_kids(copy, first, src, band);
    let frag_gap_rules = gap_rule_spec(copy, inner, opts);
    let frag_gap_items = frag_gap_rules
        .as_ref()
        .map(|_| crate::paint::gap_rules::GapItems::default());
    let child_opts = opts.inner(|cx| {
        if let Some(items) = &frag_gap_items {
            cx.deferred.gap_items = Some(items.clone());
        }
    });
    // css-break-3 §5.5: «Fragmentation … occurs
    // before relative positioning, transforms,
    // and any other graphical effects. Such
    // effects are applied per fragment». Разрезы
    // трансформ не двигают (`shape_full` его и не
    // читает), но САМ трансформ обязан быть на
    // каждом фрагменте. Общий путь вешает его
    // через `transformed()`; узкая ветка копии шла мимо
    // всех трёх, и `transform` у ребёнка
    // многоколоночника пропадал целиком
    // (`transform-000…005`: `translateX(60px)`
    // контейнера гасил `left:-60px` потомков, а
    // без него содержимое уезжало из колонки).
    // Начало отсчёта пока общее на всю коробку,
    // а не своё на фрагмент, — для `translate`
    // это точно, для `rotate`/`scale` нет.
    // Не строить эту копию через общий `element()`
    // вместо узкой ветки `styled_div_with`: общий
    // путь разваливает страницу целиком у
    // `multi-line-column-flex-fragmentation-035`,
    // `multi-line-row-flex-fragmentation-039/040/
    // 059`, `multicol-nested-013/021`,
    // `multicol-fill-balance-nested-000`.
    // Копия ТАБЛИЦЫ — своим рисователем.
    // `styled_div_with` + `blocks` кладут детей
    // таблицы обычными блоками, и `element()`
    // заворачивает КАЖДЫЙ ряд в СВОЮ анонимную
    // таблицу (ветка `TableRowGroup | TableRow |
    // TableCell` в `element()`): дорожки
    // считаются по одному ряду, ячейка сжимается
    // по содержимому, а фон ряда и ячейки
    // теряется вовсе. Фрагментация идёт ДО
    // графических эффектов и применяется к
    // каждому фрагменту (css-break-3 §5.5), но
    // РАСКЛАДКА фрагмента — та же табличная
    // (css-tables-3 §fragmentation).
    // В узкой ветке фон САМОЙ таблицы и блок с
    // шириной в точках рисуются, а фон ячейки, фон
    // ряда, `width: auto` и `width: 100%` не
    // рисуются НИЧЕМ.
    // `transformed` — как в общей ветке ниже:
    // `table()` его не вешает (в `element()`
    // таблица идёт мимо него), а копия обязана
    // нести трансформ на каждом фрагменте.
    if let Some(value) = copy_whole_box(
        opts,
        line_col_w,
        merged,
        copy,
        h,
        nest_row,
        nest_phase_k,
        inner,
        whole,
        part,
        &mut kids,
    ) {
        return value;
    }
    // Абсолютный потомок ищет ближайшего
    // позиционированного предка (CSS 2.1
    // §10.1), а раскладка под нами знает только
    // непосредственного родителя: коробка, чей
    // родитель содержащим блоком НЕ является,
    // уезжает в слой (`cb_push`). Общий путь
    // `element()` слой заводит, а узкая ветка
    // копии фрагмента возвращается из
    // `element()` раньше
    // (`return d.into_any_element()`) — и
    // до сих пор такая
    // коробка либо всплывала в ЧУЖОЙ внешний
    // слой (позиционированный предок ВЫШЕ
    // многоколоночника), либо, слоя нет,
    // рисовалась на месте: от края случайного
    // родителя вместо содержащего блока.
    //
    // css-position-3 §abspos-breaking: «In a
    // fragmented flow, an absolutely positioned
    // box is positioned relative to its
    // containing block ignoring any
    // fragmentation breaks (as if the flow were
    // continuous). The box may subsequently be
    // broken over several fragmentation
    // containers». Копия и есть этот
    // непрерывный поток: `ColumnStack::prepaint`
    // кладёт её `layout_as_root(Definite(col_w),
    // Definite(full_h))` во всю высоту и
    // поднимает на срез, а колонку вырезает
    // маска — коробке, попавшей в слой КОРНЯ
    // КОПИИ, фрагментация достаётся даром. То же
    // деление у Blink: кандидат, чей содержащий
    // блок внутри контекста, идёт
    // `LayoutFragmentainerDescendants`
    // (`out_of_flow_layout_part.cc:1498`).
    //
    // Предикат — тот же `establishes_cb`, что в
    // `element()`, и по стилю КОПИИ:
    // `hoist_relative` выше снимает только
    // ВСТАВКИ, сам `position: relative` (как и
    // `transform`/`contain`) на копии остаётся.
    // Прямые дети копии ничего не меняют: у них
    // `establishes_cb(inherited)` истинно, они и
    // раньше рисовались на месте.
    // Имена областей сетки — в номера линий, как в
    // общем `element()` (`place_named_areas`): ни
    // GPUI, ни taffy имён не знают, и копия
    // фрагмента клала элементы автоматически
    // (`grid-item-fragmentation-026`: оба в
    // области `a`, второй уезжал во 2-й ряд).
    let kids = match &copy.style.computed().grid_areas {
        Some(areas)
            if matches!(
                copy.style.computed().display,
                Some(Display::Grid) | Some(Display::InlineGrid)
            ) =>
        {
            place_named_areas(areas, kids)
        }
        _ => kids,
    };
    let frag_cb_layer = crate::text::inline::establishes_cb(inner).then(|| {
        crate::layout::positioned::layer_scope::LayerScope::cb(
            &opts.cx.position.state,
            fixed_cb_layer_box(inner),
        )
    });
    let child_build = &child_opts.cx.contracts.build;
    let mut body = child_build.blocks(&kids, src_inner, &child_opts);
    if let Some(layer) = frag_cb_layer {
        body.extend(layer.finish());
    }
    let d = frag_box_div(
        col_vert,
        col_rl,
        copy,
        h,
        src,
        src_inner,
        frag_gap_rules,
        frag_gap_items,
        kids,
        &mut body,
        &opts.cx,
    );
    let el = build.effects(
        d.children(body).into_any_element(),
        Effect::transform(inner, merged, vcx),
    );
    // css-break-3 §5.5: graphical effects apply per fragment; clip-path and
    // masks go through the same group as on the common block path.
    if inner.clip_inset.is_some()
        || inner.clip_shape.is_some()
        || inner.clip_polygon.is_some()
        || inner.mask_image.is_some()
    {
        build.effects(el, Effect::group(inner, vcx))
    } else {
        el
    }
}
