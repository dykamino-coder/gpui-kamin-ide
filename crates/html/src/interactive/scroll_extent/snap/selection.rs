//! CSS Scroll Snap 1 §5.2.1: a chosen alignment must leave its snap area visible.
use super::Rect;

#[derive(Clone, Copy)]
pub(super) struct Choice {
    pub position: f32,
    pub area: Option<Rect>,
}

pub(super) fn choose(
    mut choices: [Vec<Choice>; 2],
    current: [f32; 2],
    origin: [f32; 2],
    view: [f32; 2],
) -> [f32; 2] {
    let has_area = choices.iter().flatten().any(|choice| choice.area.is_some());
    for axis in 0..2 {
        if choices[axis].is_empty() {
            choices[axis].push(Choice {
                position: current[axis],
                area: None,
            });
        }
    }
    let find = |choices: &[Vec<Choice>; 2]| {
        let mut best = None::<([f32; 2], f32)>;
        for x in &choices[0] {
            for y in &choices[1] {
                if has_area && x.area.is_none() && y.area.is_none() {
                    continue;
                }
                let position = [x.position, y.position];
                let visible = |rect: Rect| {
                    (0..2).all(|axis| {
                        let shift = current[axis] - position[axis];
                        rect[axis + 2] + shift >= origin[axis]
                            && rect[axis] + shift <= origin[axis] + view[axis]
                    })
                };
                if !x.area.is_none_or(visible) || !y.area.is_none_or(visible) {
                    continue;
                }
                let distance =
                    (position[0] - current[0]).powi(2) + (position[1] - current[1]).powi(2);
                if best.is_none_or(|(_, old)| distance < old) {
                    best = Some((position, distance));
                }
            }
        }
        best.map(|(position, _)| position)
    };
    if let Some(position) = find(&choices) {
        return position;
    }
    // When the two mandatory alignments cannot coexist, keep one valid axis
    // rather than snapping both to an empty part of the scrollable canvas.
    for axis in 0..2 {
        choices[axis].push(Choice {
            position: current[axis],
            area: None,
        });
    }
    find(&choices).unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offscreen_area_cannot_supply_an_infinite_snap_edge() {
        let choices = [
            vec![Choice {
                position: 0.0,
                area: None,
            }],
            vec![Choice {
                position: 100.0,
                area: Some([200.0, 100.0, 230.0, 130.0]),
            }],
        ];
        assert_eq!(choose(choices, [0.0; 2], [0.0; 2], [100.0; 2]), [0.0; 2]);
    }

    #[test]
    fn two_dimensional_snapping_keeps_the_contributing_areas_visible() {
        let near = [20.0, 200.0, 50.0, 230.0];
        let far = [200.0, 20.0, 230.0, 50.0];
        let choices = [
            vec![
                Choice {
                    position: 20.0,
                    area: Some(near),
                },
                Choice {
                    position: 200.0,
                    area: Some(far),
                },
            ],
            vec![
                Choice {
                    position: 20.0,
                    area: Some(far),
                },
                Choice {
                    position: 200.0,
                    area: Some(near),
                },
            ],
        ];
        // Independently choosing 20px in both axes would leave both areas offscreen.
        assert_eq!(
            choose(choices, [0.0; 2], [0.0; 2], [100.0; 2]),
            [20.0, 200.0]
        );
    }
}
