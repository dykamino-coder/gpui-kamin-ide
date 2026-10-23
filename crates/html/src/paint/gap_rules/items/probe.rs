//! Проба элемента сетки/гибкого контейнера для геометрии промежутков: абсолютный элемент во
//! весь padding box записывает в буфер border-box элемента (расширенный на толщину рамки, так
//! как промежуток идёт между рамочными краями, css-gaps-1 §gap-grid/§gap-flex) без округления
//! к точкам устройства и его хвостовые поля.

use crate::hooks::{IntoHooked, PaintOnly, Painter};
use crate::paint::gap_rules::GapItems;
use gpui::{AnyElement, App, Bounds, LayoutId, Window, px};

/// Проба элемента сетки: как `edge_probe`, но пишет только границы.
///
/// Абсолютная проба `top: 0; left: 0; size: 100%` ложится на ПАДДИНГ-бокс
/// элемента (содержащий блок абсолютного потомка — padding box, CSS 2.1
/// §10.1), а геометрия промежутков строится по РАМОЧНЫМ коробкам элементов
/// (css-gaps-1 §gap-grid/§gap-flex: промежуток — между краями элементов,
/// Blink `GapGeometry` берёт border-box фрагментов). `border` — толщины
/// рамки элемента [top, right, bottom, left]: на них проба расширяется
/// (`grid-gap-decorations-008`: элементы с `border: 1px`, линейки вставали
/// на 1px внутрь от первой и последней линии сетки).
pub fn gap_item_probe(
    items: GapItems,
    border: [f32; 4],
    margins: [f32; 2],
    carried_row_gap: f32,
) -> AnyElement {
    PaintOnly(GapItemProbe {
        items,
        border,
        margins,
        carried_row_gap,
    })
    .hooked()
}

/// Элемент пробы: границы берутся БЕЗ округления к точке устройства.
/// Линейка ставится по середине промежутка между элементами, а эталон
/// кладёт её абсолютной коробкой от точного начала (`top: 64.17px`) —
/// от округлённых краёв элементов середина уезжает на долю точки, и при
/// масштабе 1.25 край линейки округлялся на строку ниже
/// (`flex-gap-decorations-048`: строка y=86 лишняя).
struct GapItemProbe {
    pub(crate) items: GapItems,
    pub(crate) border: [f32; 4],
    pub(crate) margins: [f32; 2],
    pub(crate) carried_row_gap: f32,
}

impl Painter for GapItemProbe {
    type Prepaint = ();

    fn style(&self) -> gpui::Style {
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        style
    }

    fn prepaint(&mut self, state: LayoutId, window: &mut Window, _cx: &mut App) -> Self::Prepaint {
        let origin = window.layout_origin_unrounded(state);
        let size = window.layout_size_unrounded(state);
        let [t, r, b, l] = self.border;
        let mut buffer = self.items.borrow_mut();
        if self.carried_row_gap > 0.0
            && let Some(item) = window.parent_layout_id(state)
        {
            buffer.carried_row_gaps.insert(item, self.carried_row_gap);
        }
        buffer.margins.push(self.margins);
        buffer.push(Bounds {
            origin: gpui::point(origin.x - px(l), origin.y - px(t)),
            size: gpui::size(size.width + px(l + r), size.height + px(t + b)),
        });
    }
}
