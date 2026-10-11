//! Break spaces tests for paragraph; split out to keep the owning module within 250 lines.

use gpui::{SharedString, px};

use super::*;

fn wrap() -> Wrap {
    Wrap {
        break_spaces: true,
        keep_spaces: true,
        ..Default::default()
    }
}

/// `white-space: break-spaces` даёт точку разрыва ПОСЛЕ каждого пробела.
/// Пока их не было, строка рвалась только по правилам UAX-14, и
/// сохранённый пробел уходил в конец строки вместо начала следующей.
#[test]
fn every_preserved_space_gives_a_stop() {
    let para = Paragraph::new(
        SharedString::from("X XX X".to_string()),
        vec![],
        px(25.),
        px(25.),
        Align::Left,
        wrap(),
    );
    let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
    assert!(stops.contains(&2), "после первого пробела: {stops:?}");
    assert!(stops.contains(&5), "после второго пробела: {stops:?}");
}
