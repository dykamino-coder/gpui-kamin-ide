//! Позиционированные дети потока блоков: держатель краёв и слой статической позиции.
// owner: A

use crate::dom::Element;
use crate::layout::block::struts::margin_px;
use crate::layout::positioned::static_position::{static_line_align, static_self_align};
use crate::paint::stacking::{layered, stacking_context};
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn static_position_layer(
    e: &Element,
    inherited: &Computed,
    built: AnyElement,
    layer_ok: bool,
    under_tf: bool,
    nodes: &[crate::dom::Node],
    idx: usize,
    paint_key: u64,
    out: &mut Vec<gpui::AnyElement>,
    below_zs: &mut Vec<i32>,
    mut below_run_start: usize,
    mut below_run_end: usize,
) -> (usize, usize) {
    // Позиционированный элемент рисуется ПОВЕРХ обычного
    // содержимого (CSS 2.1 §9.9, шаг 8) и без заданного `z-index`:
    // без верхнего слоя следующий за ним сосед закрашивал его
    // собой — блок стоял на месте, но был не виден (проба:
    // абсолютный кусок между «AA» и «BB» пропадал целиком, хотя
    // один в блоке рисовался верно).
    //
    // Позиционированный элемент рисуется ПОВЕРХ обычного
    // содержимого (CSS 2.1 §9.9, шаг 8), а порядок отрисовки у нас
    // — порядок детей. Отложенная отрисовка тут не работает ни в
    // каком виде (пробовали трижды: css-position 31 → 0, css-text
    // 966 → 810, падение процесса), поэтому содержимое уходит
    // ПОСЛЕДНИМ ребёнком родителя, а на своём месте остаётся
    // нулевая распорка с холстом-щупом. Разницу их положений
    // элемент забирает отрицательным полем — так он оказывается
    // там же, где был, но рисуется последним.
    // Отрицательный `z-index` рисуется ПОД содержимым потока
    // (CSS 2.1 §9.9, шаг 3), поэтому в верхний слой он не идёт:
    // там его место — поверх всего.
    let below = e.style.z_index.is_some_and(|z| z < 0);
    let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
    spot.set(crate::layout::positioned::containing_block::Spot {
        rtl: inherited.rtl == Some(true),
        vertical: inherited.vertical == Some(true),
        vertical_rl: inherited.vertical_rl == Some(true),
        own_vertical: e.style.vertical == Some(true),
        replaced: matches!(
            e.tag.as_str(),
            "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
        ),
        line_align: static_line_align(e, inherited),
        self_align: static_self_align(e, inherited),
        ..Default::default()
    });
    let probe = crate::layout::positioned::containing_block::spot_probe(spot.clone(), true);
    // Поля сдвигают абсолютный элемент ОТ статической позиции
    // (CSS 2.1 §10.3.7: auto-края = static + margin). Раскладка
    // под нами поля у absolute без краёв не считает — сдвиг
    // даёт absolute-обёртка (clip-path-rectangle-ref и родня:
    // эталонный зелёный стоял без своих margin: 50px).
    let ml = margin_px(e.style.margin.left, &e.style).unwrap_or(0.0);
    let mt = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
    // Под потоком (`below`) коробка остаётся абсолютной на месте
    // распорки, и поле от статической позиции ей уже даёт сама
    // раскладка (taffy: `static_position + margin`); обёртка
    // прибавляла его второй раз (`tab-size-inheritance-001`:
    // красная подложка на 50 точек правее).
    // Обёртка — содержащий блок коробки для раскладки, и её ширина
    // — доступная ширина shrink-to-fit (CSS 2.1 §10.3.7: ширина
    // содержащего блока минус статическое смещение и поля;
    // Blink `absolute_utils.cc` ComputeAbsoluteInlineSize берёт
    // `available_size` от края до края содержащего блока). Без
    // правого края обёртка была нулевой ширины, и `<h1>` с
    // полями по умолчанию ломался после каждого слова
    // (min-content). Правый край — только при ltr в
    // горизонтальном письме: rtl ставит коробку от правого края
    // обёртки, вертикальный заместитель нулевой и так.
    let mr = margin_px(e.style.margin.right, &e.style).unwrap_or(0.0);
    let stretch = inherited.rtl != Some(true) && inherited.vertical != Some(true);
    let built = if (ml != 0.0 || mt != 0.0) && !below {
        let wrap = div().absolute().left(px(ml)).top(px(mt));
        let wrap = if stretch { wrap.right(px(mr)) } else { wrap };
        wrap.child(built).into_any_element()
    } else {
        built
    };
    // A positive `z-index` orders the box above the auto/0
    // positioned boxes (CSS 2.1 §9.9 steps 8–9), as on the
    // CB-layer path above: `scalex` — a static-position abspos
    // with `z-index: 11` painted under its `z-index: 10` sibling.
    let built = if !below && e.style.z_index.is_some_and(|z| z > 0) {
        layered(built, &e.style, inherited, layer_ok, under_tf)
    } else {
        built
    };
    // Абсолют на статической позиции — тоже шаг 8: в собирателе
    // он встаёт среди позиционированных по ключу, а не поверх
    // всех соседей контейнера.
    let taken = if below {
        Some(built)
    } else if paint_last_ok(e, &nodes[idx + 1..]) {
        crate::layout::positioned::containing_block::late_push(
            spot,
            gpui::PaintLast::new(built).key(paint_key).into_any_element(),
        )
    } else {
        crate::layout::positioned::containing_block::late_push(spot, built)
    };
    match taken {
        None => out.push(probe),
        Some(kept) => {
            // ЗАМЕРЕНО И ОТКАЧЕНО: заворачивать `kept` в
            // `Underlay`, чтобы коробка с отрицательным `z-index`
            // легла ПОД поток (§9.9 шаг 3) — срез из 259 пар семей
            // *shape*: 0 и 0, НИ ОДНО число не сдвинулось.
            // Подложка порядок не меняет: нижним слоям сцена даёт
            // общий номер, а сортировка устойчива. Тройку
            // `spec-examples/shape-outside-004…006` сломал коммит
            // `05b7a4f` (28.08, гейт `x_set && y_set` в `movable`),
            // и возвращать её надо порядком краски, а не слоем.
            // Отрицательный `z-index` принадлежит БЛИЖАЙШЕМУ контексту
            // наложения (CSS 2.1 прил. E, шаг 3), а позиционированный
            // родитель с `z-index: auto` его не образует: ребёнок
            // обязан лечь ПОД его фон (шаг 8 родителя выше шага 3
            // корня). Держатель на месте красил ребёнка после фона
            // родителя — красный поверх зелёного
            // (`z-index-abspos-001`). Подложка — тот же приём, что у
            // относительного с `z<0` ниже по функции. Прежний замер
            // обёртки (запись выше, 259 пар *shape*: 0/0) шёл без
            // гейта по родителю; `!cb_ancestor` держит правку там,
            // где контекст наверняка корневой (у `fixed-pos-stacking-001`
            // выше стоит `fixed` — он контекст, туда не заходим).
            let kept = if below
                && matches!(
                    inherited.position,
                    Some(crate::style::computed::Position::Relative)
                        | Some(crate::style::computed::Position::Absolute)
                )
                && !inherited.cb_ancestor
                && !stacking_context(inherited)
            {
                crate::paint::effects::underlay::Underlay::new(kept).into_any_element()
            } else {
                kept
            };
            let contiguous = below && out.len() == below_run_end;
            out.push(
                div()
                    .relative()
                    .w_full()
                    .h_0()
                    .flex_shrink_0()
                    .child(kept)
                    .into_any_element(),
            );
            if below {
                if !contiguous {
                    below_run_start = out.len() - 1;
                    below_zs.clear();
                }
                below_zs.push(e.style.z_index.unwrap_or(0));
                let mut at = below_zs.len() - 1;
                while at > 0 && below_zs[at - 1] > below_zs[at] {
                    below_zs.swap(at - 1, at);
                    out.swap(below_run_start + at - 1, below_run_start + at);
                    at -= 1;
                }
                below_run_end = out.len();
            }
        }
    }
    (below_run_start, below_run_end)
}

pub(crate) fn inset_holder_box(
    e: &Element,
    block_axis: bool,
    inherited: &Computed,
    opts: &RenderOpts,
    kw_len: &impl Fn(Option<Len>) -> bool,
) -> AnyElement {
    let mut holder = Computed::default();
    holder.position = e.style.position;
    holder.inset = e.style.inset;
    holder.z_index = e.style.z_index;
    holder.display = Some(Display::Flex);
    holder.flex_dir = Some(if block_axis {
        crate::style::computed::FlexDir::Col
    } else {
        crate::style::computed::FlexDir::Row
    });
    // Поля вдоль оси остаются у ВНУТРЕННЕЙ коробки: auto-поля
    // элемента гибкого контейнера забирают остаток, а при нехватке
    // места обнуляются (css-flexbox-1 §8.1) — ровно как auto-поля
    // абсолюта (§3.8; `fit-content-block-size-abspos` с
    // переполнением). Поперечные не-auto поля — у держателя.
    // Поперечные поля — у держателя целиком, включая auto: с
    // заданным размером и краями с обеих сторон они центрируют
    // сам абсолют (css-position-3 §3.8; `inline-size: 100px;
    // margin: auto; inset: 0`).
    if block_axis {
        holder.margin.left = e.style.margin.left;
        holder.margin.right = e.style.margin.right;
    } else {
        holder.margin.top = e.style.margin.top;
        holder.margin.bottom = e.style.margin.bottom;
    }
    // Поперечный размер держателя — border-box внутренней коробки:
    // её рамка и отбивка прибавляются (у держателя своих нет).
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let bd = e.style.borders();
    if block_axis {
        let extra = px_of(e.style.padding.left)
            + px_of(e.style.padding.right)
            + px_of(bd.left)
            + px_of(bd.right);
        holder.width = match e.style.width {
            Some(Len::Px(w)) => Some(Len::Px(w + extra)),
            other => other.filter(|l| !kw_len(Some(*l))),
        };
    } else {
        let extra = px_of(e.style.padding.top)
            + px_of(e.style.padding.bottom)
            + px_of(bd.top)
            + px_of(bd.bottom);
        holder.height = match e.style.height {
            Some(Len::Px(h)) => Some(Len::Px(h + extra)),
            other => other.filter(|l| !kw_len(Some(*l))),
        };
    }
    let mut inner = e.clone();
    inner.style.position = None;
    inner.style.inset = Default::default();
    if block_axis {
        inner.style.margin.left = None;
        inner.style.margin.right = None;
    } else {
        inner.style.margin.top = None;
        inner.style.margin.bottom = None;
    }
    inner.style.z_index = None;
    crate::style::apply::apply(div(), &holder)
        .child(element(&inner, inherited, opts))
        .into_any_element()
}
