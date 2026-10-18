//! Этап обработки позиционирования и слоя блочного ребёнка.

mod route;
pub(super) use route::layer_route;

use super::finish_element;
use crate::hooks::IntoHooked;
use crate::layout::block::struts::margin_px;
use crate::paint::stacking::layered;
use crate::render::blocks::flow::BelowRun;
use crate::render::*;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement};

#[allow(clippy::too_many_arguments)]
pub(crate) fn place_element(
    e: &crate::dom::Element,
    under_tf: bool,
    inherited: &Computed,
    ordered_context: bool,
    geometry_layer_ok: bool,
    nodes: &[crate::dom::Node],
    idx: usize,
    built: AnyElement,
    layer_ok: bool,
    paint_key: u64,
    out: &mut Vec<AnyElement>,
    below: &mut BelowRun,
    hoist_margins: bool,
    placement: (Option<gpui::GridLocation>, Option<gpui::GridLineNames>),
    record_root: bool,
    opts: &RenderOpts,
) {
    let (x_set, y_set, fixed, below_icb, to_icb, to_cb, far_fixed) = layer_route(
        opts.paged_build().is_some(),
        e,
        inherited,
        under_tf,
        ordered_context,
        geometry_layer_ok,
        nodes,
        idx,
    );
    let built = if to_icb || to_cb {
        let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
        spot.set(crate::layout::positioned::containing_block::Spot {
            fixed_axes: (x_set, y_set),
            preserve_cb_geometry: to_cb,
            viewport_static: to_icb,
            free_margin: (
                if x_set {
                    0.0
                } else {
                    margin_px(e.style.margin.left, &e.style).unwrap_or(0.0)
                },
                if y_set {
                    0.0
                } else {
                    margin_px(e.style.margin.top, &e.style).unwrap_or(0.0)
                },
            ),
            rtl: inherited.wm().rtl(),
            vertical: inherited.wm().vertical(),
            vertical_rl: inherited.wm().block_rl(),
            own_vertical: e.style.wm().vertical(),
            replaced: matches!(
                e.tag.as_str(),
                "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
            ),
            // The static-position rectangle and the box's self-alignment in
            // it (css-position-3 §staticpos-rect, css-align-3 §justify-abspos)
            // belong to the static position, not to the layer that carries
            // the box: a CB layer above a non-positioned parent aligns the
            // same way as the parent's own late layer
            // (`justify-self-static-position-001`).
            self_align: crate::layout::positioned::static_position::static_self_align(e, inherited),
            ..Default::default()
        });
        let mut source_paint = None;
        let paged = opts.paged_build().filter(|_| to_icb && fixed);
        let sent = if let Some(paged) = paged {
            // Стопка страниц: фиксированный — в свой слой, по копии
            // на лист без сдвига (`PagedBuild::fixed`). Обёртка `LatePlace`
            // та же, что у слоя ICB, — через вложенный слой.
            let icb_layer = crate::layout::positioned::layer_scope::LayerScope::icb();
            let _ = crate::layout::positioned::containing_block::icb_push(spot.clone(), built);
            paged.push_fixed(icb_layer.finish());
            None
        } else {
            // Слой содержащего блока рисуется после его потока, но
            // позиционированные красятся в порядке разметки (шаг 8):
            // ключ ставит коробку слоя среди них. `fixed` в слое ICB —
            // тоже: без ключа он красился раньше собирателя, и
            // поднятый в собиратель предок ложился поверх.
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: `fixed` без ключа — кусок 1704
            // пары +4/−2 (`static-fixed-inside-abspos`,
            // `position-fixed-001`: 0.00 -> «красное видно»); с
            // ключом +4/−0.
            // A positive `z-index` orders the box above the layer's
            // auto/0 boxes (CSS 2.1 §9.9 steps 8–9): the deferral the
            // in-place path gets from `layered` below
            // (`shape-image-009`: `#test` z-index 2 under a z-index 1
            // failure box, both hoisted).
            let built = if !fixed && e.style.z_index.is_some_and(|z| z > 0) {
                layered(built, &e.style, inherited, layer_ok, under_tf)
            } else {
                built
            };
            let built = if paint_last_ok(e, &nodes[idx + 1..]) {
                gpui::PaintLast::new(built)
                    .key(paint_key)
                    .into_any_element()
            } else {
                built
            };
            let built = if to_icb && below_icb {
                crate::hooks::IntoHooked::hooked(crate::paint::effects::underlay::Underlay::new(
                    built,
                ))
            } else {
                built
            };
            let built =
                if crate::layout::positioned::predicates::stays_positioned(&nodes[idx + 1..]) {
                    let mut state = spot.get();
                    state.layout_only = true;
                    spot.set(state);
                    let (layout, paint) =
                        crate::layout::positioned::containing_block::split_source_paint(
                            spot.clone(),
                            built,
                        );
                    source_paint = Some(paint);
                    layout
                } else {
                    built
                };
            if to_icb {
                crate::layout::positioned::containing_block::icb_push(spot.clone(), built)
            } else if far_fixed {
                crate::layout::positioned::containing_block::cb_push_fixed(spot.clone(), built)
            } else {
                crate::layout::positioned::containing_block::cb_push(
                    spot.clone(),
                    built,
                    crate::layout::positioned::containing_block::nearest_cb_index(inherited),
                )
            }
        };
        match sent {
            None => {
                // Пустая ось требует щупа: статическую позицию взять
                // больше неоткуда. При заданных обеих осях на месте
                // не остаётся ничего.
                if !(x_set && y_set) {
                    out.push(crate::layout::positioned::containing_block::spot_probe(
                        spot, true,
                    ));
                }
                if let Some(paint) = source_paint {
                    out.push(paint);
                }
                return;
            }
            Some(kept) if to_icb && !(x_set && y_set) => {
                // Deferred scroll trees are assembled after the document ICB
                // closes. Preserve their hypothetical flow position locally.
                out.push(crate::layout::positioned::containing_block::spot_probe(
                    spot.clone(),
                    true,
                ));
                crate::layout::positioned::containing_block::LatePlace { child: kept, spot }
                    .hooked()
            }
            Some(kept) => kept,
        }
    } else {
        built
    };
    // Абсолютная коробка с ОТРИЦАТЕЛЬНЫМ `z-index` не идёт ни в слой
    // ICB, ни в верхний слой: её место ПОД потоком (§9.9 шаг 3). Но
    // пустая ось у неё считается от СТАТИЧЕСКОЙ позиции, а гибкая
    // раскладка такой коробке её не даёт и ставит в начало содержимого
    // родителя. Нулевая распорка держит место в потоке, и коробка
    // висит от её угла — там, где написана.
    // Исключение — коробка блочного уровня, у которой задана БЛОЧНАЯ ось
    // (`top`/`bottom`), а свободна строчная, в горизонтальном письме
    // слева направо, и содержащий блок — сам родитель. Статическая
    // позиция по строчной оси здесь — левый край содержимого
    // родителя (§10.3.7), её раскладка на месте даёт и так, а
    // заданную ось §10.6.4 считает от СОДЕРЖАЩЕГО БЛОКА. На распорке
    // `top: 1px` отсчитывался от статической позиции — коробка
    // съезжала под весь поток (`margin-collapse-clear-012..016`:
    // красная подложка `z-index: -1` под жёлтым блоком).
    finish_element(
        e,
        y_set,
        x_set,
        inherited,
        ordered_context,
        below,
        built,
        layer_ok,
        under_tf,
        nodes,
        idx,
        paint_key,
        out,
        hoist_margins,
        placement,
        record_root,
        opts,
    )
}
