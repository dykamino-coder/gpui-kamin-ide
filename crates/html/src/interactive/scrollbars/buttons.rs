//! РљРЅРѕРїРєРё-СЃС‚СЂРµР»РєРё РєР»Р°СЃСЃРёС‡РµСЃРєРѕР№ РїРѕР»РѕСЃС‹ РїСЂРѕРєСЂСѓС‚РєРё РЅР° РѕР±РѕРёС… РєРѕРЅС†Р°С… РґРѕСЂРѕР¶РєРё: С‚СЂРµСѓРіРѕР»СЊРЅРёРє РІ
//! С†РµРЅС‚СЂРµ РєРЅРѕРїРєРё, РѕС‚РєР»СЋС‡С‘РЅРЅРѕРµ СЃРѕСЃС‚РѕСЏРЅРёРµ РїРѕ С‚РµРєСѓС‰РµРјСѓ РїРѕР»РѕР¶РµРЅРёСЋ РІ РґРёР°РїР°Р·РѕРЅРµ РїСЂРѕРєСЂСѓС‚РєРё,
//! Р°РІС‚РѕСЂСЃРєРёР№ С†РІРµС‚ РїРѕР»Р·СѓРЅРєР° СЃ РїСЂРёРіР»СѓС€С‘РЅРЅРѕР№ Р°Р»СЊС„РѕР№ РґР»СЏ РѕС‚РєР»СЋС‡С‘РЅРЅРѕР№ СЃС‚СЂРµР»РєРё.
use gpui::{Hsla, PathBuilder, Window, point, px};

fn disabled(bar: &super::modern::Bar, handle: &gpui::ScrollHandle, index: usize) -> bool {
    let offset = handle.offset();
    let start = -f32::from(if bar.axis == 0 { offset.x } else { offset.y });
    let start = if bar.reverse {
        bar.maximum - start
    } else {
        start
    };
    if index == 0 {
        start <= 0.0
    } else {
        start >= bar.maximum
    }
}

fn id(handle: &gpui::ScrollHandle, axis: usize, index: usize, state: bool) -> gpui::ScrollNodeId {
    gpui::ScrollNodeId::follower(handle, 2 + axis * 4 + index * 2 + usize::from(state))
}

pub(super) fn follow(bar: &super::modern::Bar, handle: &gpui::ScrollHandle, window: &mut Window) {
    if bar.button < 6.0 {
        return;
    }
    for index in 0..2 {
        for state in [false, true] {
            let source = handle.clone();
            let bar = *bar;
            window.with_scroll_visibility_prepaint(
                id(handle, bar.axis, index, state),
                std::rc::Rc::new(move || disabled(&bar, &source, index) == state),
                |_| {},
            );
        }
    }
}

pub(super) fn paint(
    bar: &super::modern::Bar,
    handle: &gpui::ScrollHandle,
    (arrows, custom): ([Hsla; 2], Option<Hsla>),
    window: &mut Window,
) {
    let (lane, axis, button) = (bar.lane, bar.axis, bar.button);
    if button < 6.0 {
        return;
    }
    let length = f32::from(if axis == 0 {
        lane.size.width
    } else {
        lane.size.height
    });
    for (index, position) in [button / 2.0, length - button / 2.0]
        .into_iter()
        .enumerate()
    {
        let sign = if index == 0 { -1.0 } else { 1.0 };
        let center = if axis == 0 {
            point(
                lane.left() + px(position),
                lane.top() + lane.size.height / 2.0,
            )
        } else {
            point(
                lane.left() + lane.size.width / 2.0,
                lane.top() + px(position),
            )
        };
        let vertex = |main, cross| {
            if axis == 0 {
                point(center.x + px(main), center.y + px(cross))
            } else {
                point(center.x + px(cross), center.y + px(main))
            }
        };
        let mut path = PathBuilder::fill();
        path.move_to(vertex(sign * 2.0, 0.0));
        path.line_to(vertex(-sign * 2.0, -3.0));
        path.line_to(vertex(-sign * 2.0, 3.0));
        path.close();
        if let Ok(path) = path.build() {
            for state in [false, true] {
                let mut color = custom.unwrap_or(arrows[usize::from(!state)]);
                if custom.is_some() && state {
                    color.a *= 0.4;
                }
                window.with_scroll_node_paint(id(handle, axis, index, state), |window| {
                    window.paint_path(path.clone(), color);
                });
            }
        }
    }
}
