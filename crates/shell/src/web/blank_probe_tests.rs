//! Проба пустого accelerated-кадра (INC-2026-0073): решение без GPU.

use super::*;

#[test]
fn пустой_кадр_это_все_точки_с_нулевым_rgb() {
    assert!(all_blank(&[[0, 0, 0, 0], [0, 0, 0, 255]]));
    assert!(!all_blank(&[[0, 0, 0, 0], [0x2e, 0x1e, 0x1e, 255]]));
    assert!(!all_blank(&[]), "без точек судить не о чем");
}

#[test]
fn серия_растёт_на_пустых_и_рвётся_живым_кадром() {
    let mut s = 0;
    for _ in 0..STREAK {
        s = streak_after(s, true);
    }
    assert_eq!(s, STREAK);
    assert_eq!(streak_after(s, false), 0);
}

#[test]
fn точки_лежат_внутри_региона_страницы() {
    let pts = sample_points((10, 64, 400, 200));
    assert_eq!(pts.len(), 5);
    for (x, y) in pts {
        assert!((10..410).contains(&x) && (64..264).contains(&y));
    }
    assert!(sample_points((0, 0, 0, 10)).is_empty());
}

#[test]
fn вью_пуст_только_после_серии_и_забывается_с_браузером() {
    let id = "test-blank-probe";
    forget_view(id);
    for _ in 0..STREAK - 1 {
        note(id, true);
    }
    assert!(!is_blank(id));
    note(id, true);
    assert!(is_blank(id));
    forget_view(id);
    assert!(!is_blank(id));
}
