//! Тесты пересечений линеек сетки: невидимые (`rule-style: none`) поперечные линейки всё равно
//! образуют junction по геометрии, а `rule-visibility-items: between` его снимает
//! (css-gaps-1 §3.3, §visibility-items).
use super::*;
use crate::style::computed::{GapInset, GapList};
use crate::style::values::value::{Color, Len};

fn rule(visible: bool, visibility: u8) -> GapAxisRule {
    GapAxisRule {
        widths: GapList::single(12.),
        colors: GapList::single(Color {
            r: 0.,
            g: 0.,
            b: 0.,
            a: 1.,
        }),
        styles: GapList::single(visible),
        brk: 2,
        inset: [GapInset::Len(Len::Px(0.)); 4],
        visibility,
        double: false,
        kinds: GapList::single(0),
    }
}

#[test]
fn invisible_cross_rules_still_form_junctions() {
    let tracks = vec![(0., 100.), (120., 220.)];
    let main = rule(true, 0);
    let cross = rule(false, 0);
    let runs = grid_runs_on(
        &[],
        &[],
        tracks.clone(),
        tracks.clone(),
        &main,
        Some(&cross),
        (false, false),
    );
    assert!(runs[0].crossings[0].joins);
    assert_eq!(runs[0].crossings[0].cross_w, 12.);

    let main = rule(true, 3);
    let cross = rule(false, 3);
    let runs = grid_runs_on(
        &[],
        &[],
        tracks.clone(),
        tracks,
        &main,
        Some(&cross),
        (false, false),
    );
    assert!(!runs[0].crossings[0].joins);
}
