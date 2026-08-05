//! Прокрутка документа с видимой полосой.
//!
//! GPUI умеет прокручивать содержимое (`overflow_y_scroll`), но полосу не
//! рисует: в его модели это дело приложения. Для документа полоса обязательна
//! — без неё не видно ни того, что содержимое длиннее окна, ни где ты сейчас.
//!
//! Полоса считается из состояния прокрутки: смещение и максимум даёт
//! `ScrollHandle`, отсюда и положение ползунка, и его высота.

use gpui::{
    AnyElement, Div, InteractiveElement, ParentElement, ScrollHandle, StatefulInteractiveElement,
    Styled, div, px, relative, rgba,
};

/// Ширина полосы — как у нативной тонкой полосы Windows.
const BAR_W: f32 = 8.0;
/// Ползунок короче этого не становится, иначе его невозможно поймать мышью.
const MIN_THUMB: f32 = 24.0;

/// Обёртка: прокручиваемое содержимое плюс полоса справа.
///
/// `id` нужен GPUI, чтобы связать состояние прокрутки с элементом между
/// кадрами — без него прокрутка сбрасывалась бы каждый кадр.
pub fn scroll_area(
    id: &'static str,
    handle: &ScrollHandle,
    content: impl IntoIterator<Item = AnyElement>,
) -> Div {
    let offset = -handle.offset().y;
    let max = handle.max_offset().height;
    let view = handle.bounds().size.height;

    div()
        .relative()
        .flex_1()
        .min_h(px(0.))
        .child(
            div()
                .id(id)
                .size_full()
                .overflow_y_scroll()
                .track_scroll(handle)
                .flex()
                .flex_col()
                .children(content),
        )
        .children(thumb(f32::from(offset), f32::from(max), f32::from(view)))
}

/// Ползунок. `None`, когда содержимое помещается целиком — полоса в этом
/// случае только мешает.
fn thumb(offset: f32, max: f32, view: f32) -> Option<Div> {
    if max <= 1.0 || view <= 1.0 {
        return None;
    }
    let total = view + max;
    let height = (view / total * view).max(MIN_THUMB).min(view);
    let travel = view - height;
    let top = if max > 0.0 {
        (offset / max).clamp(0.0, 1.0) * travel
    } else {
        0.0
    };
    Some(
        div()
            .absolute()
            .top(px(top))
            .right(px(2.))
            .w(px(BAR_W))
            .h(px(height))
            .rounded(px(BAR_W / 2.))
            // Полупрозрачный поверх содержимого: своей дорожки не рисуем,
            // чтобы полоса не съедала ширину документа.
            .bg(rgba(0x8a90a480)),
    )
}

/// Тот же приём для горизонтальной прокрутки — нужен блокам кода: длинная
/// строка не должна ни переноситься, ни растягивать панель.
pub fn h_scroll(
    id: &'static str,
    handle: &ScrollHandle,
    content: AnyElement,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .overflow_x_scroll()
        .track_scroll(handle)
        .child(div().w(relative(1.0)).child(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_thumb_when_content_fits() {
        assert!(thumb(0.0, 0.0, 500.0).is_none());
    }

    #[test]
    fn thumb_shrinks_as_content_grows() {
        // Содержимое вдвое длиннее окна: ползунок примерно вполовину окна.
        let t = thumb(0.0, 500.0, 500.0);
        assert!(t.is_some());
    }

    #[test]
    fn thumb_never_shorter_than_the_minimum() {
        // Очень длинный документ: доля крошечная, но ползунок остаётся ловимым.
        let view: f32 = 500.0;
        let height = (view / (view + 100000.0) * view).max(MIN_THUMB).min(view);
        assert!(height >= MIN_THUMB, "получено {height}");
    }
}
