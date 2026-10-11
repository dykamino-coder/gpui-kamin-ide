//! Построение копии ребёнка для фрагмента колонки (build_copy).

use super::{copy_kids, copy_whole_box, frag_box_div};
use crate::dom::Element;
use crate::layout::fragment::clone::clone_fragment;
use crate::layout::grid::place_named_areas;
use crate::layout::multicol::gap_rules::gap_rule_spec;
use crate::paint::effects::transform::transformed;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::render::{RenderOpts, blocks};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use gpui::{IntoElement, ParentElement};

#[allow(clippy::too_many_arguments)]
pub(super) fn build_copy(
    e: &Element,
    opts: &RenderOpts,
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    frag_geom: &[(f32, f32)],
    dec: Option<(f32, f32)>,
    merged: &Computed,
    copy: &Element,
    h: f32,
    over: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: &Computed,
    copy_ix: &std::cell::Cell<usize>,
    whole: bool,
    first: bool,
    part: usize,
) -> gpui::AnyElement {
    // `box-decoration-break: clone`: копия — САМ
    // фрагмент (`clone_fragment`), и корень, и
    // наследуемый стиль берутся у НЕГО. ★ Откат
    // 07.09 (v153) держался ровно здесь: корень
    // строился со стилем ИСХОДНОЙ коробки, и
    // каждая копия выходила полной коробкой
    // (`clone-007` 2.08 = ровно квадрат 100×100,
    // `-026` 4.98 = 240×25×4 вылета). НЕпоследний
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
    let frag_inner = frag.as_ref().map(|f| inherit(merged, &f.style));
    let src: &Element = frag.as_ref().unwrap_or(copy);
    let src_inner: &Computed = frag_inner.as_ref().unwrap_or(inner);
    let mut kids = copy_kids(e, copy, first, src);
    let frag_gap_rules = gap_rule_spec(copy, inner, opts);
    let frag_gap_key = frag_gap_rules.as_ref().map(|_| {
        let ix = copy_ix.get();
        copy_ix.set(ix + 1);
        (copy.node_id ^ opts.doc_salt) ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
    });
    let mut frag_gap_guard = frag_gap_key.map(crate::paint::gap_rules::GapGuard::enter);
    // css-break-3 §5.5: «Fragmentation … occurs
    // before relative positioning, transforms,
    // and any other graphical effects. Such
    // effects are applied per fragment». Разрезы
    // трансформ не двигают (`shape_full` его и не
    // читает), но САМ трансформ обязан быть на
    // каждом фрагменте. Общий путь вешает его
    // через `transformed()` (render.rs:1719,
    // :5938, :8185); узкая ветка копии шла мимо
    // всех трёх, и `transform` у ребёнка
    // многоколоночника пропадал целиком
    // (`transform-000…005`: `translateX(60px)`
    // контейнера гасил `left:-60px` потомков, а
    // без него содержимое уезжало из колонки).
    // Начало отсчёта пока общее на всю коробку,
    // а не своё на фрагмент, — для `translate`
    // это точно, для `rotate`/`scale` нет.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строить эту
    // копию через общий `element()` вместо узкой
    // ветки `styled_div_with`. Срез 3029 пар:
    // 1900 -> 1902 (+14/-12), и семь потерь —
    // грубые (99.00, страница разъезжается):
    // `multi-line-column-flex-fragmentation-035`,
    // `multi-line-row-flex-fragmentation-039/040/
    // 059`, `multicol-nested-013/021`,
    // `multicol-fill-balance-nested-000`. Тот же
    // путь, на котором прежде мерился откат -52.
    // Копия ТАБЛИЦЫ — своим рисователем.
    // `styled_div_with` + `blocks` кладут детей
    // таблицы обычными блоками, и `element()`
    // заворачивает КАЖДЫЙ ряд в СВОЮ анонимную
    // таблицу (ветка `TableRowGroup | TableRow |
    // TableCell`, render.rs:11622): дорожки
    // считаются по одному ряду, ячейка сжимается
    // по содержимому, а фон ряда и ячейки
    // теряется вовсе. Фрагментация идёт ДО
    // графических эффектов и применяется к
    // каждому фрагменту (css-break-3 §5.5), но
    // РАСКЛАДКА фрагмента — та же табличная
    // (css-tables-3 §fragmentation).
    // ЗАМЕРЕНО пробами (`target/probe-bt/`,
    // стенд v150): фон САМОЙ таблицы рисуется
    // (`p4-2col-bgtbl` 0.00) и блок с шириной в
    // точках рисуется (`p5-b-w100-div50` 0.00), а
    // фон ячейки (`p6-td-bg`), фон ряда
    // (`p6-tr-bg`), `width: auto`
    // (`p5-c-w100-divauto`) и `width: 100%`
    // (`p6-div-w100pct`) не рисуются НИЧЕМ —
    // 15625 красных точек из 15625.
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
        &mut frag_gap_guard,
    ) {
        return value;
    }
    // Абсолютный потомок ищет ближайшего
    // позиционированного предка (CSS 2.1
    // §10.1), а раскладка под нами знает только
    // непосредственного родителя: коробка, чей
    // родитель содержащим блоком НЕ является,
    // уезжает в слой (`cb_push`,
    // render.rs:4230 `to_cb`). Общий путь
    // `element()` слой заводит
    // (render.rs:13511-13529), а узкая ветка
    // копии фрагмента возвращается из
    // `element()` раньше
    // (`return d.into_any_element()`,
    // render.rs:13274) — и до сих пор такая
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
    // непрерывный поток: `flow.rs` `prepaint`
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
    let kids = match &copy.style.grid_areas {
        Some(areas)
            if matches!(
                copy.style.display,
                Some(Display::Grid) | Some(Display::InlineGrid)
            ) =>
        {
            place_named_areas(areas, kids)
        }
        _ => kids,
    };
    let frag_cb_layer = crate::text::inline::establishes_cb(inner);
    if frag_cb_layer {
        crate::layout::positioned::containing_block::cb_open_with(fixed_cb_layer_box(inner));
    }
    let mut body = blocks(&kids, src_inner, opts);
    if frag_cb_layer {
        body.extend(crate::layout::positioned::containing_block::cb_close());
    }
    drop(frag_gap_guard);
    let d = frag_box_div(
        col_vert,
        col_rl,
        copy,
        h,
        src,
        src_inner,
        frag_gap_rules,
        frag_gap_key,
        kids,
        &mut body,
    );
    transformed(d.children(body).into_any_element(), inner, merged)
}
