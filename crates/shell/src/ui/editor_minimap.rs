//! Минимапа редактора. Zed (`crates/editor/src/element.rs`) рендерит её
//! ОТДЕЛЬНЫМ редактором с `MINIMAP_FONT_SIZE = px(1.0)`; у нас нет второго
//! редактора, поэтому строим силуэт квадами по (indent, len) и накладываем
//! глифы 2px сверху — иначе строки из строчных букв не растеризуются вовсе.
//! Минимапа редактора (порт подхода Zed, PR #26893): силуэт строк
//! (indent+len квады), пропорциональная прокрутка контента минимапы
//! (формула MinimapLayout::calculate_minimap_top_offset), thumb = вьюпорт,
//! клик = центрирование + jump, драг = слежение. Рендер квадами (без
//! шейпинга): при 2-3px/строка глифы Zed — те же «пятна».

pub use crate::ui::editor::scrollbar::scrollbar;
use crate::ui::minimap_geom::ED_LINE_H;
use crate::ui::minimap_geom::MIN_THUMB;
use crate::ui::minimap_geom::MM_FONT;
use crate::ui::minimap_geom::{MM_LINE_H, MM_WIDTH, geom, jump_to};
use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::InputState;
use kamin_theme::Palette;

use crate::colors::rgba;

pub fn minimap(
    input: &Entity<InputState>,
    mirror: Option<&Entity<InputState>>,
    p: &Palette,
) -> AnyElement {
    // Origin канвы для мышиной математики (координаты события — оконные)
    let origin: Rc<Cell<(f32, f32, f32)>> = Rc::new(Cell::new((0.0, 0.0, 0.0)));

    let thumb = {
        let mut c = rgba(p.text_primary);
        c.a = 0.08;
        c
    };
    let thumb_border = {
        let mut c = rgba(p.text_primary);
        c.a = 0.16;
        c
    };
    let silhouette = {
        let mut c = rgba(p.text_muted);
        c.a = 0.45;
        c
    };

    // Zed: минимапа делит буфер с родителем, а её позиция считается из
    // процента прокрутки родителя. У нас зеркало — отдельный InputState,
    // поэтому прокрутку выставляем сами по той же mm_top.
    let sync = {
        let input = input.clone();
        let mirror = mirror.cloned();
        let origin = origin.clone();
        move |cx: &mut gpui::App| {
            let Some(mm) = mirror.as_ref() else { return };
            let (_, _, h) = origin.get();
            if h <= 0.0 {
                return;
            }
            let g = geom(input.read(cx), h);
            let want = -(g.mm_top * MM_LINE_H);
            mm.update(cx, |st, _| {
                let cur = f32::from(st.scroll_handle.offset().y);
                if (cur - want).abs() > 0.5 {
                    st.scroll_handle.set_offset(gpui::point(px(0.0), px(want)));
                }
            });
        }
    };

    let sync = std::rc::Rc::new(sync);
    let sync_move = sync.clone();
    let sync_down = sync.clone();
    let canvas = {
        let input = input.clone();
        let origin = origin.clone();
        gpui::canvas(
            move |bounds, _, _| {
                origin.set((
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.height),
                ));
            },
            move |bounds, _, window, cx| {
                let st = input.read(cx);
                let h = f32::from(bounds.size.height);
                let g = geom(st, h);
                let bx = f32::from(bounds.origin.x);
                let by = f32::from(bounds.origin.y);
                let bw = f32::from(bounds.size.width);
                let start = g.mm_top.floor().max(0.0) as usize;
                let end = ((g.mm_top + g.vis_mm).ceil() as usize).min(g.total as usize);
                // Содержимое рисует ЗЕРКАЛЬНЫЙ редактор (Zed: отдельный
                // minimap-editor), поэтому здесь только геометрия thumb.
                // Раньше на каждый кадр собирались тексты всех видимых
                // строк и тут же выбрасывались — чистая мёртвая работа.
                let _ = (start, end, silhouette);
                let thumb_geom = (g.total, g.vis_ed, g.scroll_row, g.mm_top);
                let (g_total, g_vis_ed, g_scroll_row, g_mm_top) = thumb_geom;
                // Thumb = вьюпорт редактора в координатах минимапы
                if g_total > g_vis_ed {
                    // Zed `for_minimap`: высота thumb = видимые строки
                    // редактора в масштабе минимапы, позиция — от mm_top.
                    // Клампим по треку: overscroll редактора иначе выносит
                    // палку за нижнюю границу панели.
                    let th = (g_vis_ed * MM_LINE_H).max(MIN_THUMB).min(h);
                    let ty = (by + (g_scroll_row - g_mm_top) * MM_LINE_H)
                        .clamp(by, by + (h - th).max(0.0));
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(gpui::point(px(bx), px(ty)), gpui::size(px(bw), px(th))),
                        thumb,
                    ));
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(gpui::point(px(bx), px(ty)), gpui::size(px(1.0), px(th))),
                        thumb_border,
                    ));
                }
            },
        )
        .absolute()
        .size_full()
    };

    let on_jump = {
        let input = input.clone();
        let origin = origin.clone();
        move |wy: f32, cx: &mut gpui::App| {
            let (_, oy, h) = origin.get();
            let local = (wy - oy).clamp(0.0, h);
            input.update(cx, |st, cx| {
                let off = jump_to(st, local, h);
                st.scroll_handle.set_offset(off);
                cx.notify();
            });
        }
    };
    let on_jump_move = on_jump.clone();

    div()
        .id("editor-minimap")
        .w(px(MM_WIDTH))
        .flex_shrink_0()
        .h_full()
        .relative()
        .overflow_hidden()
        // Zed: минимапа — отдельный редактор с MINIMAP_FONT_SIZE, полный
        // layout, вставлен дочерним элементом. Ввод в него не уходит:
        // обработчики мыши висят на контейнере минимапы.
        .when_some(mirror.cloned(), |d, mm| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .text_size(px(MM_FONT))
                    // Высота строки приходит ОТСЮДА: `InputState::minimap()`
                    // отключает жёсткий `Rems(1.25)` внутри `Input`, поэтому
                    // зеркало наследует line-height родителя (см. MM_LINE_H).
                    .line_height(px(MM_LINE_H))
                    .child(
                        gpui_component::input::Input::new(&mm)
                            .h_full()
                            .appearance(false)
                            .hide_scrollbar()
                            // `input_text_size(Size::Size(s))` = `s * 0.875`,
                            // поэтому кегль просим с обратной поправкой
                            .with_size(gpui_component::Size::Size(px(MM_FONT / 0.875))),
                    ),
            )
        })
        .child(canvas)
        .on_scroll_wheel({
            // Колесо над минимапой скроллит ОСНОВНОЙ редактор (в Zed минимапа
            // не имеет своей прокрутки — её положение производно от editor
            // scroll_position). Зеркало собственных листенеров не ставит.
            let input = input.clone();
            let sync = sync.clone();
            let origin_scroll = origin.clone();
            move |e: &gpui::ScrollWheelEvent, _, cx| {
                let dy = f32::from(e.delta.pixel_delta(px(ED_LINE_H)).y);
                // Высота ВЬЮПОРТА минимапы, а не строки: раньше сюда шло 20.0,
                // из-за чего `max_scroll` был завышен на целый экран и кламп
                // прокрутки не работал (ревью ц.7).
                let (_, _, view_h) = origin_scroll.get();
                input.update(cx, |st, cx| {
                    let g = geom(st, view_h.max(1.0));
                    let cur = f32::from(st.scroll_handle.offset().y);
                    let min = -(g.max_scroll * g.line_h);
                    let want = (cur + dy).clamp(min.min(0.0), 0.0);
                    st.scroll_handle.set_offset(gpui::point(px(0.0), px(want)));
                    cx.notify();
                });
                sync(cx);
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, {
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                on_jump(f32::from(e.position.y), cx);
                sync_down(cx);
            }
        })
        .on_mouse_move({
            move |e: &gpui::MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(gpui::MouseButton::Left) {
                    on_jump_move(f32::from(e.position.y), cx);
                    sync_move(cx);
                }
            }
        })
        .into_any_element()
}
