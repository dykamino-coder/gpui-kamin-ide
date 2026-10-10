//! Окраска одного блочного ребёнка; ранний возврат завершает текущую итерацию потока.

mod layers;
pub(crate) use layers::finish_element;

mod placement;
pub(crate) use placement::place_element;

mod position;
pub(crate) use position::position_element;

use super::BelowRun;
use crate::animation::frames::transitioned;
use crate::dom::Node;
use crate::interactive::scroll_box::{resizable, scrollable};
use crate::layout::atom::pct_resolved_for_wrapper;
use crate::layout::float::float_flow::float_flow;
use crate::layout::replaced::limits::content_limit_swapped;
use crate::paint::effects::paint_scope;
use crate::paint::effects::paint_scope::inside as inside_deferred;
use crate::paint::stacking::{defers, layered, stacking_context};
use crate::render::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_element(
    e: &crate::dom::Element,
    nodes: &[Node],
    idx: usize,
    inherited: &Computed,
    opts: &RenderOpts,
    ordered_context: bool,
    under_tf: bool,
    frame: &crate::interactive::sticky::element::StickyCell,
    letter_scope: &mut crate::render::first_letter_scope::Scope,
    out: &mut Vec<AnyElement>,
    below: &mut BelowRun,
) {
    // Ключ краски шага 8 — до сборки детей (см. `next_paint_key`).
    let paint_key = next_paint_key();
    // CSS2 Appendix E: descendants paint within their nearest stacking context.
    let layer_ok = !inside_deferred();
    let geometry_layer_ok = !paint_scope::deferred();
    let _deferred_guard = paint_scope::Guard::enter(
        defers(&e.style, inherited, under_tf),
        stacking_context(&e.style),
    );
    // Ряд обтекания: текст рядом с плавающим блоком и остаток под ним.
    if e.tag == "kamin-float" {
        out.push(letter_scope.flow(&e.children, inherited, |s| float_flow(e, s, opts)));
        return;
    }
    if let Some(el) = scrollable(e, inherited, opts) {
        out.push(layered(el, &e.style, inherited, layer_ok, under_tf));
        return;
    }
    if let Some(el) = resizable(e, inherited, opts) {
        out.push(el);
        return;
    }
    if let Some(el) = transitioned(e, inherited, opts) {
        // Наложение считается и для узла с переходом: раньше ветка
        // уходила мимо, и `z-index` у него пропадал.
        out.push(layered(el, &e.style, inherited, layer_ok, under_tf));
        return;
    }
    // `display: contents` — своей коробки у элемента нет: дети
    // становятся детьми родителя, и стиль самого элемента исчезает.
    if e.style.display == Some(Display::Contents) {
        let mut merged = inherit(inherited, &e.style);
        // Своей коробки нет — значит и объёмный контекст она не
        // обрывает: дети берут ячейки ДЕДА (css-display-3
        // §box-generation; transform3d-preserve3d-014 — `rotateX(90)`
        // над `display: contents` над `rotateX(90) scale(2)`).
        if merged.frame_3d.is_none() {
            merged.frame_3d = inherited.frame_3d.clone();
        }
        if merged.perspective_frame.is_none() {
            merged.perspective_frame = inherited.perspective_frame.clone();
        }
        out.extend(blocks(&e.children, &merged, opts));
        return;
    }
    // Ключевое слово содержимого в `min-width`/`max-width` при
    // ширине в точках (css-sizing-3 §4.1, зажим §5.1): used =
    // max(W, kw) либо min(W, kw) — то же самое, что `width: kw` с
    // пределом W. Перестановка отдаёт ключевое слово обёртке-сетке
    // (`content_sized`), а точки — пределу в раскладке (`apply`);
    // блочная ось решается в `apply` (`min-height: max-content`).
    let swapped;
    let e = if let Some(copy) = content_limit_swapped(e) {
        swapped = copy;
        &swapped
    } else {
        e
    };
    // Обёртка `content_sized` — сетка, а дорожка сетки НЕ считает
    // боковые поля ребёнка: коробка `width: max-content` с полем
    // теряла его и уезжала (`pre-wrap-017`: зелёный блок пропадал
    // вовсе). Поэтому элемент строится БЕЗ боковых полей, а поля
    // берёт на себя обёртка.
    // Переносится только ОТРИЦАТЕЛЬНОЕ поле: положительное внутри
    // дорожки работает как надо, а отрицательное дорожка съедает —
    // коробка `width: max-content` с `margin-left: -1em` пропадала
    // вовсе (`pre-wrap-017`).
    let negative = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::Px(v) | Len::Em(v) | Len::Ch(v) | Len::Ex(v)) if v < 0.0
        )
    };
    let hoist_margins = content_sized_wraps(e)
        && !replaced_tag(e)
        && (negative(e.style.margin.left) || negative(e.style.margin.right));
    // Размещение в сетке тоже уезжает на обёртку (см.
    // `content_sized`): в дорожках родителя стоит она. Внутри обёртки
    // (своя сетка в одну дорожку) элемент с прежним `grid-row: 2`
    // уходил бы в её неявный ряд. Прежде обёртка без размещения
    // ставилась авто-размещением (`row-fill-reverse-align-self-001`:
    // `width: min-content; grid-row: 2` в лунках вставал в ряд 1).
    let placement = crate::style::apply::grid_item_placement(&e.style);
    let hoist_place = content_sized_wraps(e)
        && !replaced_tag(e)
        && (placement.0.is_some() || placement.1.is_some());
    let stripped;
    let e = if hoist_margins || hoist_place {
        let mut copy = e.clone();
        if hoist_margins {
            copy.style.margin.left = None;
            copy.style.margin.right = None;
        }
        if hoist_place {
            copy.style.grid_row = None;
            copy.style.grid_col = None;
            copy.style.grid_row_named = [None, None];
            copy.style.grid_col_named = [None, None];
        }
        stripped = copy;
        &stripped
    } else {
        e
    };
    let placement = if hoist_place { placement } else { (None, None) };
    // Коробка по содержимому (`width: min-content | max-content |
    // fit-content`) стоит в обёртке-сетке `content_sized`, и её доли
    // решались бы ОТ ОБЁРТКИ, а не от содержащего блока: `height: 100%`
    // — от неявного ряда (по содержимому: коробка схлопывалась в ноль),
    // `padding-top: 100%` — от области сетки (заливала всё окно).
    // Блочный родитель с известными сторонами решает их сразу
    // (CSS 2.1 §10.5 высота — от высоты содержащего блока, §8.4
    // отступы и §8.3 поля — от его ширины;
    // `intrinsic-percent-replaced-012/013`).
    let resolved_pct;
    let e = match pct_resolved_for_wrapper(e, inherited) {
        Some(copy) => {
            resolved_pct = copy;
            &resolved_pct
        }
        None => e,
    };
    // Анимация оборачивает ЛЮБОЙ элемент: таблицу, список, картинку —
    // раньше она доставалась только простому блоку.
    // Фон КАНВАСА (CSS 2.2 §14.2): фон корневого html — а без него
    // фон body — красит всю область просмотра, включая место за
    // полями. Слой absolute от родителя-корня растягивается на всё
    // окно, с самой коробки краска снимается (иначе двойная альфа).
    let canvas_paint = e.style.canvas_bg;
    // Тело под корнем-донором фона холста при `vertical-rl`: его
    // margin-box (плюс рамка/отбивка корня) — коробка корня по
    // содержимому; её левый край пишется на подготовке тела.
    let record_root = e.tag == "body"
        && inherited.canvas_bg
        && inherited.vertical_rl == Some(true)
        && !matches!(inherited.width, Some(Len::Px(_)));
    let canvas_stripped;
    let e = if canvas_paint {
        canvas_layer(e, opts, out);
        let mut copy = e.clone();
        copy.style.background = None;
        copy.style.gradient = None;
        copy.style.bg_image = None;
        canvas_stripped = copy;
        &canvas_stripped
    } else {
        e
    };
    // Абсолют с КЛЮЧЕВЫМ СЛОВОМ содержимого по оси и краями с обеих
    // сторон этой оси (css-position-3 §3.7-3.8): растяжение краями —
    // только для автоматического размера; заданный ключевым словом
    // размер — по содержимому, а остаток делят auto-поля. Держатель =
    // inset-modified containing block (абсолют с краями элемента,
    // гибкий контейнер вдоль оси), внутри — та же коробка статической,
    // без краёв и полей (`div-{min,max,fit}-content-block-size`,
    // `div-*-auto-margin-*`).
    position_element(
        e,
        inherited,
        opts,
        frame,
        layer_ok,
        under_tf,
        ordered_context,
        geometry_layer_ok,
        nodes,
        idx,
        paint_key,
        out,
        below,
        hoist_margins,
        placement,
        record_root,
    )
}
