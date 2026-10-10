//! Построитель ребёнка полос: раскладка узла при заданной ширине и вырезах.

use crate::dom::{Element, Node};
use crate::layout::block::containing::{AVAIL_W, AvailWGuard, CB_WIDTH, scopeguard_cb};
use crate::layout::float::band_nest::cont_indent;
use crate::layout::float::float_atom::band_atom;
use crate::layout::replaced::image::image;
use crate::layout::writing_mode::native_vertical;
use crate::paint::effects::grouped::grouped;
use crate::paint::effects::paint_scope::DepthScope;
use crate::render::{RenderOpts, blocks, content_wrapper, element, replaced_tag, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement};

#[allow(clippy::too_many_arguments)]
pub(super) fn build_band_kid(
    depth: crate::paint::effects::paint_scope::Depth,
    float: bool,
    node: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    is_nest: bool,
    vertical: bool,
    cb_height: Option<Len>,
    cb_block_w: Option<Len>,
    cb: f32,
    avail: f32,
    shapes: Option<
        std::sync::Arc<(
            Vec<super::super::shapes::FloatShape>,
            Vec<super::super::shapes::FloatShape>,
        )>,
    >,
    height: Option<f32>,
) -> AnyElement {
    let _depth = DepthScope::enter(depth);
    // Ширина содержащего блока — та, что намерил хост: замещаемым
    // без размеров (§10.3.2) и долям внутри (`CB_WIDTH`), блочным
    // детям куска — ширина окна (`AVAIL_W`).
    let cb_prev = CB_WIDTH.get();
    let avail_prev = AVAIL_W.get();
    if cb > 0.0 {
        CB_WIDTH.set(Some(cb));
    }
    AVAIL_W.set((avail > 0.0).then_some(avail));
    let _cb_guard = scopeguard_cb(cb_prev);
    let _avail_guard = AvailWGuard(avail_prev);
    let mut copy = node.clone();
    cont_indent(&mut copy, inherited);
    // Сторону, очистку и поля несёт хост (позиция от полос), на
    // самой коробке они сдвинули бы её ещё раз — как у
    // статического хоста.
    copy.style.float = None;
    copy.style.clear = None;
    copy.style.margin = crate::style::computed::Sides::default();
    // Флоат заводит свой контекст форматирования (§9.4.1), а
    // `float` с копии снят — метка остаётся: по ней дети флоата
    // узнают корень БФК (`parent_bfc` у `wrap_floats` — §10.6.7:
    // высота флоата охватывает флоаты внутри него;
    // `letter-spacing-206`).
    if float {
        copy.style.flow_root = Some(true);
        native_vertical::claim_float_inline_size(&mut copy.style, inherited, &copy.children);
    }
    if is_nest {
        // Коробка `Kind::Nest` — без детей (их кладут полосы) и
        // высотой содержимого из плана.
        copy.children.clear();
        copy.style.height = height.map(Len::Px);
        copy.style.border_box = None;
        return element(&copy, inherited, opts);
    }
    // Доля ширины — от СОДЕРЖАЩЕГО БЛОКА (§10.2), а каркас пробы
    // шириной в окно: решаем её здесь.
    // `Pct(1.0)` — это и `100%`, и `stretch` (`value.rs` пишет
    // ключевое слово долей): `stretch` заполняет ОКНО рядом с
    // флоатом (css-sizing-4 §4.1, Blink — доступный размер из
    // возможности, `block_layout_algorithm.cc`
    // `child_available_inline_size`), и его оставляем каркасу
    // (`bfc-next-to-float-1`); `100%` рядом с флоатом не влез бы
    // ни в какое окно, а ниже флоатов окно и есть содержащий блок.
    if let Some(Len::Pct(k)) = copy.style.width
        && cb > 0.0
        && (float || k != 1.0)
        && !vertical
    {
        copy.style.width = Some(Len::Px(k * cb));
    }
    // Доля ВЫСОТЫ — от высоты содержащего блока, когда она задана
    // точками (§10.5). Ребёнок хоста раскладывается своим корнем
    // с неопределённой высотой, и доля там вырождалась в `auto`:
    // плавающий `height: 100%` в блоке `height: 200px` выходил
    // высотой в свой текст (`flexbox-align-self-horiz-001-ref`).
    if let Some(Len::Pct(k)) = copy.style.height
        && let Some(Len::Px(h)) = cb_height
    {
        copy.style.height = Some(Len::Px(k * h));
    }
    // В вертикальном письме блочный размер — физическая ширина:
    // её доля — от ширины содержащего блока (§10.5 по блочной
    // оси). Каркас пробы ширины не задаёт, и `block-size: 100%`
    // у флоата вырождалась в ноль — флоат с детьми пропадал
    // (эталон `css-break/background-image-001`: колонки-флоаты
    // `block-size: 100%` во `flow-root` `vertical-rl`).
    if vertical
        && let Some(Len::Pct(k)) = copy.style.width
        && let Some(Len::Px(w)) = cb_block_w
    {
        copy.style.width = Some(Len::Px(k * w));
    }
    // Вырезы полос — строкам ЭТОЙ коробки, от её верха (шаг F4):
    // `inline::inherit` начинает слитый стиль с собственного, и
    // вырезы доезжают до прямых строк коробки.
    if shapes.is_some() {
        copy.style.flow_shapes = shapes;
    }
    let table = copy.tag == "table"
        || matches!(
            copy.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        );
    if copy.attr("atoms") == Some("1") {
        // Прогон атомов: `FlowRow` режет строки вырезами полос.
        let atoms: Vec<crate::layout::fragment::types::FlowChild> = copy
            .children
            .iter()
            .filter_map(|n| match n {
                Node::Element(a) => band_atom(a, inherited, opts),
                Node::Text(_) => None,
            })
            .collect();
        let shapes = copy
            .style
            .flow_shapes
            .clone()
            .unwrap_or_else(|| std::sync::Arc::new((Vec::new(), Vec::new())));
        return crate::layout::fragment::types::FlowRow::new(
            atoms,
            shapes,
            inherited.rtl == Some(true),
        )
        .into_any_element();
    }
    // Замещаемый флоат, кроме `<img>` (`embed`, `object`,
    // `video`…), — своей веткой `element` ниже: каркас блока со
    // `blocks(детей)` рисовал вместо картинки пустую коробку, и
    // `object-fit-*-00Ne/o/p` (88 пар `css-images`) теряли
    // содержимое.
    let replaced = replaced_tag(&copy) && copy.tag != "img";
    // Вертикальный флоат — общим путём `element`: только там блок
    // вертикального письма раскладывает детей рядом по
    // горизонтальной оси блочного потока. Каркас `styled_div_with`
    // + `blocks` клал их горизонтальным блоком, строчный размер
    // пустого ребёнка выходил нулём, и флоат с детьми не
    // рисовался вовсе (эталон `css-break/background-image-001`:
    // колонка-флоат с `<div style="block-size:100%; background">`).
    // CSS Lists 3 §2: a floated list item (a `::before`/`::after`
    // with `display: list-item` too) keeps its marker, which only
    // `element`'s list-item painter draws.
    let list_item = copy.style.display == Some(Display::ListItem);
    if float && !table && !replaced && !vertical && !list_item {
        // Флоат — блочная коробка (§9.7) каким бы ни был тег: тем
        // же путём, что у статического хоста (`shape_flow`).
        // Таблица — своей веткой `element` ниже: каркас блока её
        // не соберёт.
        let mut merged = inherit(inherited, &copy.style);
        merged.margin = crate::style::computed::Sides::default();
        if copy.tag == "img" {
            grouped(image(&copy), &copy.style)
        } else {
            grouped(
                styled_div_with(&copy, &merged)
                    .children(blocks(&copy.children, &merged, opts))
                    .into_any_element(),
                &copy.style,
            )
        }
    } else {
        // Общий путь отрисовки узла — тот же, что в потоке:
        // таблица, замещаемый, список строятся своими ветками
        // `element`.
        let el = element(&copy, inherited, opts);
        // Эффекты группы (`clip-path`, маска, фильтр, смешивание)
        // `element` не накладывает — их кладёт поток (`blocks`:
        // `grouped(transformed(animated(..)))`). Вертикальный флоат
        // идёт сюда мимо потока, и `clip-path` у него не резал
        // ничего (`shape-outside-circle-048-ref`: флоат целым
        // прямоугольником при абсолютах с вставками по обеим осям).
        if float {
            grouped(el, &copy.style)
        } else {
            content_wrapper::for_element(el, &copy, inherited, (None, None))
        }
    }
}
