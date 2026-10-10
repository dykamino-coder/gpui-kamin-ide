//! Общие хелперы сетки.
// owner: A

use crate::dom::Node;

/// Развернуть именованные области сетки в номера линий.
///
/// `grid-template-areas` — способ разложить макет именами вместо цифр. Ни
/// GPUI, ни taffy имён не знают, но знают номера: имя ищется в раскладке
/// контейнера, и ребёнок получает готовый прямоугольник линий.
pub(crate) fn place_named_areas(areas: &[Vec<String>], children: Vec<Node>) -> Vec<Node> {
    use crate::style::computed::Placement;
    children
        .into_iter()
        .map(|n| match n {
            Node::Element(mut e) => {
                let Some(name) = e.style.grid_area_name.clone() else {
                    return Node::Element(e);
                };
                // Прямоугольник имени: первая и последняя строка, первый и
                // последний столбец, где оно встречается.
                let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for (row, cells) in areas.iter().enumerate() {
                    for (col, cell) in cells.iter().enumerate() {
                        if *cell == name {
                            r0 = r0.min(row);
                            r1 = r1.max(row + 1);
                            c0 = c0.min(col);
                            c1 = c1.max(col + 1);
                        }
                    }
                }
                if r0 != usize::MAX {
                    // Линии в CSS считаются с единицы.
                    e.style.grid_row = Some((
                        Placement::Line(r0 as i16 + 1),
                        Placement::Line(r1 as i16 + 1),
                    ));
                    e.style.grid_col = Some((
                        Placement::Line(c0 as i16 + 1),
                        Placement::Line(c1 as i16 + 1),
                    ));
                }
                Node::Element(e)
            }
            other => other,
        })
        .collect()
}
