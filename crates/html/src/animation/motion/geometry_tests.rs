//! Тесты геометрии пути: точка и касательная по длине.

use super::{flatten, length, sample};

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn line_point_and_tangent() {
    let poly = flatten("M0 0 L30 40").unwrap();
    assert!(near(length(&poly.pts), 50.0, 1e-3));
    let ((x, y), ang) = sample(&poly.pts, 25.0);
    assert!(near(x, 15.0, 1e-3) && near(y, 20.0, 1e-3));
    assert!(near(ang, 40f32.atan2(30.0), 1e-4));
}

#[test]
fn half_circle_arc_quarter_length() {
    // Полукруг радиуса 50 от (0,0) к (100,0): длина pi*50.
    let poly = flatten("M0 0 A50 50 0 0 1 100 0").unwrap();
    let total = length(&poly.pts);
    assert!(near(total, std::f32::consts::PI * 50.0, 0.1));
    let ((x, y), ang) = sample(&poly.pts, total / 2.0);
    assert!(near(x, 50.0, 0.1) && near(y.abs(), 50.0, 0.1));
    assert!(near(ang.abs(), 0.0, 0.05));
}

#[test]
fn cubic_and_quadratic_agree_when_degenerate() {
    let q = flatten("M0 0 Q50 100 100 0").unwrap();
    let c = flatten("M0 0 C33.3333 66.6667 66.6667 66.6667 100 0").unwrap();
    assert!(near(length(&q.pts), length(&c.pts), 0.05));
    let (pq, _) = sample(&q.pts, length(&q.pts) / 2.0);
    assert!(near(pq.0, 50.0, 0.1) && near(pq.1, 50.0, 0.1));
}

#[test]
fn closed_path_ends_at_start() {
    let poly = flatten("M0 0 H10 V10 Z").unwrap();
    assert!(poly.closed);
    assert!(near(length(&poly.pts), 20.0 + 10.0 * 2f32.sqrt(), 1e-3));
}
