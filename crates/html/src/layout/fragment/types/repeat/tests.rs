//! Допуск полос повтора во фрагментаинере и место подвала у фрагмента.

use super::*;
use gpui::IntoElement;

/// Шапка — полоса 20 (секция 20, `head_need` 80), подвал — полоса 15
/// (секция 15, `foot_need` 60).
fn geom() -> RepeatGeom {
    RepeatGeom {
        head: 20.0,
        foot: 15.0,
        head_end: 20.0,
        foot_at: 285.0,
        foot_end: 300.0,
        box_top: 0.0,
        box_end: 300.0,
        head_need: 80.0,
        foot_need: 60.0,
    }
}

#[test]
fn each_page_admits_only_bands_up_to_its_quarter() {
    let bands = |target: f32| {
        let g = geom().admit(target, true);
        (g.head, g.foot)
    };
    // Ровно четверть — повтор («up to one quarter»), меньше страница — нет.
    assert_eq!(bands(80.0), (20.0, 15.0));
    assert_eq!(bands(79.0), (0.0, 15.0));
    assert_eq!(bands(60.0), (0.0, 15.0));
    assert_eq!(bands(59.0), (0.0, 0.0));
}

#[test]
fn bands_never_take_a_whole_fragmentainer() {
    // Колонки: решение о повторе принято там, где таблица началась; в
    // меньшем фрагментаинере полосы остаются, пока телу таблицы есть место.
    let g = geom().admit(36.0, false);
    assert_eq!((g.head, g.foot), (20.0, 15.0));
    for target in [35.0, 20.0, 0.0] {
        let g = geom().admit(target, false);
        assert_eq!((g.head, g.foot), (0.0, 0.0), "{target}");
        assert_eq!(g.head_at(100.0), 0.0);
        assert_eq!(g.foot_for(100.0, target), 0.0);
    }
}

fn frag(from: f32, h: f32, foot: f32) -> Frag {
    Frag {
        kid: 0,
        copy: 1,
        col: 1,
        y: 0.0,
        from,
        h,
        head: 0.0,
        foot,
    }
}

#[test]
fn footer_follows_the_last_row_unless_a_row_is_broken() {
    let r = Repeat {
        head: None,
        foot: Some((285.0, 15.0)),
        geom: geom(),
        row_ends: vec![40.0, 130.0, 285.0],
        head_els: Vec::new(),
        foot_els: vec![gpui::div().into_any_element()],
    };
    // Разрыв между рядами (конец ряда 130): подвал сразу под рядом, зазор —
    // после подвала.
    assert_eq!(r.foot_top(&frag(80.0, 50.0, 50.0)), Some(50.0));
    // Разорванный ряд тянется до края: подвал у низа фрагментаинера.
    assert_eq!(r.foot_top(&frag(80.0, 30.0, 70.0)), Some(85.0));
    // Последний фрагмент без резерва подвала полосы не берёт.
    assert_eq!(r.foot_top(&frag(130.0, 170.0, 0.0)), None);
}
