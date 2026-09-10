//! Якорь hover-пилюли строки/группы сайдбара, привязанный к id (BR-29).
//!
//! Overlay рисует пилюлю только по якорю той же строки, что и `hover_pill`,
//! а переход в rename и dismiss сбрасывают geometry вместе с состоянием —
//! координаты одной строки нельзя использовать для другой.

use gpui::prelude::*;

/// Якорь hovered-строки/группы (лог. px), привязанный к её id: overlay
/// рисует пилюлю только когда id совпадает с `hover_pill`, так что
/// координаты одной строки нельзя использовать для другой (BR-29).
#[derive(Clone, Debug, PartialEq)]
pub struct PillAnchor {
    pub id: String,
    pub bounds: [f32; 4],
}

pub fn pill_anchor() -> &'static std::sync::Mutex<Option<PillAnchor>> {
    static S: std::sync::OnceLock<std::sync::Mutex<Option<PillAnchor>>> =
        std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

/// Bounds якоря, если он принадлежит именно `id`.
pub fn anchor_for(id: &str) -> Option<[f32; 4]> {
    bounds_if(pill_anchor().lock().unwrap().as_ref(), id)
}

/// Сбросить geometry вместе с hover-состоянием (rename, dismiss).
pub fn clear_pill_anchor() {
    *pill_anchor().lock().unwrap() = None;
}

fn bounds_if(anchor: Option<&PillAnchor>, id: &str) -> Option<[f32; 4]> {
    anchor.filter(|a| a.id == id).map(|a| a.bounds)
}

pub(crate) fn anchor_probe(id: String) -> impl gpui::IntoElement {
    gpui::canvas(
        move |bounds, _, _| {
            *pill_anchor().lock().unwrap() = Some(PillAnchor {
                id: id.clone(),
                bounds: [
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                ],
            });
        },
        |_, _, _, _| {},
    )
    // Инсеты вместо `size_full` — иначе канвас участвует в раскладке строки
    // (см. `probe_registry::probe_area`)
    .absolute()
    .top_0()
    .left_0()
    .right_0()
    .bottom_0()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_geometry_is_scoped_to_its_row() {
        let a = PillAnchor {
            id: "s1".into(),
            bounds: [1.0, 2.0, 3.0, 4.0],
        };
        assert_eq!(bounds_if(Some(&a), "s1"), Some([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(
            bounds_if(Some(&a), "s2"),
            None,
            "чужие координаты не выдаются"
        );
        assert_eq!(bounds_if(None, "s1"), None);
    }
}
