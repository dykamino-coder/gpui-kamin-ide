//! Проверки переноса строк с ограниченными возможностями флоатов.
use super::*;
use crate::layout::float::shapes::FloatShape;
use std::sync::Arc;
#[test]
fn balancing_preserves_the_float_opportunity_instead_of_clearing_it() {
    let mut p = Paragraph::new(
        "aaaa aaaa aaaa".into(),
        vec![],
        px(10.),
        px(20.),
        Align::Left,
        Wrap {
            balance: true,
            ..Default::default()
        },
    );
    p.flow = Arc::new((
        vec![FloatShape::Band {
            top: 0.,
            h: 100.,
            w: 40.,
        }],
        vec![],
    ));
    // Fixed advances keep this geometry test independent of system fonts.
    let segs: Vec<_> = (0..p.text.len())
        .map(|i| Seg {
            start: i,
            end: i + 1,
            offset: px(i as f32 * 10.),
            layout: Arc::new(gpui::LineLayout {
                width: px(10.),
                ..Default::default()
            }),
        })
        .collect();
    for (limit, clearance) in [(100., 0.), (60., 100.)] {
        let lines = p.lay(Some(px(limit)), &segs);
        assert_eq!(lines.len(), 3);
        let balanced = p.balanced(lines, Some(px(limit)), &segs);
        assert_eq!(balanced.len(), 3);
        assert_eq!(
            balanced.iter().map(|l| f32::from(l.clearance)).sum::<f32>(),
            clearance
        );
        assert_eq!(
            balanced[0].indent,
            px(if clearance == 0. { 40. } else { 0. })
        );
    }
}
