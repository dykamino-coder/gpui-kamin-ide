//! Project physical edges into the text plane before its quarter-turn paint transform.
pub(super) fn project<T: Copy>(rotated: bool, counter_clockwise: bool, sides: [T; 4]) -> [T; 4] {
    let [top, right, bottom, left] = sides;
    if !rotated {
        sides
    } else if counter_clockwise {
        [left, top, right, bottom]
    } else {
        [right, bottom, left, top]
    }
}

#[cfg(test)]
mod tests {
    use super::project;

    #[test]
    fn edges_match_geometrically_rotated_outward_normals() {
        let normals = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let physical = [2, 3, 5, 7];
        for counter_clockwise in [false, true] {
            let projected = project(true, counter_clockwise, physical);
            for (flat_index, (x, y)) in normals.into_iter().enumerate() {
                let painted = if counter_clockwise { (y, -x) } else { (-y, x) };
                let physical_index = normals
                    .iter()
                    .position(|normal| *normal == painted)
                    .unwrap();
                assert_eq!(projected[flat_index], physical[physical_index]);
            }
        }
    }

    #[test]
    fn opposite_turns_restore_asymmetric_and_optional_values() {
        let values = [Some(11), None, Some(17), Some(19)];
        assert_eq!(project(true, true, project(true, false, values)), values);
        assert_eq!(project(true, false, project(true, true, values)), values);
        for counter_clockwise in [false, true] {
            assert_eq!(project(false, counter_clockwise, values), values);
        }
    }
}
