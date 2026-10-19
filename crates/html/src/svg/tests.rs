//! Проверки контракта родительского модуля; вынесены для ограничения размера файлов.

use super::raster::rasterize;
use super::serialize::serialize;
use super::*;
use crate::dom::parse;

fn find_svg(nodes: &[Node]) -> Option<&Element> {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.tag == "svg" {
                return Some(e);
            }
            if let Some(found) = find_svg(&e.children) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn subtree_is_serialised_back_to_markup() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let nodes = parse(
        r##"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#f00"/></svg>"##,
        "",
    );
    let svg = find_svg(&nodes).unwrap();
    let out = serialize(svg, vcx);
    assert!(out.starts_with("<svg"), "корень на месте: {out}");
    assert!(out.contains("<rect"), "потомок на месте: {out}");
    assert!(
        out.contains(r##"fill="#f00""##),
        "атрибуты сохранены: {out}"
    );
}

#[test]
fn size_comes_from_attributes_then_viewbox() {
    let nodes = parse(r#"<svg width="40" height="20"></svg>"#, "");
    assert_eq!(size_of(find_svg(&nodes).unwrap()), (40.0, 20.0));

    // `viewBox` даёт СООТНОШЕНИЕ, а не собственный размер: без своих
    // сторон замещаемый занимает наибольший прямоугольник этого
    // соотношения, влезающий в 300x150 (CSS 2.1 §10.3.2).
    let nodes = parse(r#"<svg viewBox="0 0 64 32"></svg>"#, "");
    assert_eq!(size_of(find_svg(&nodes).unwrap()), (300.0, 150.0));

    // Заданная сторона тянет за собой вторую по тому же соотношению.
    let nodes = parse(r#"<svg width="80" viewBox="0 0 64 32"></svg>"#, "");
    assert_eq!(size_of(find_svg(&nodes).unwrap()), (80.0, 40.0));
}

#[test]
fn colours_survive_rasterisation() {
    // Ключевая проверка: путь через иконочный элемент отдал бы одноцветную
    // маску, а график обязан сохранить цвета.
    let markup = r##"<svg viewBox="0 0 2 1" xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="1" height="1" fill="#ff0000"/>
            <rect x="1" y="0" width="1" height="1" fill="#0000ff"/>
        </svg>"##;
    let img = rasterize(markup, 2.0, 1.0).expect("растрируется");
    let size = img.size(0);
    let bytes = img.as_bytes(0).expect("пиксели доступны");
    let w = u32::from(size.width) as usize;
    let px_at = |x: usize| {
        let i = x * 4;
        [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
    };
    let left = px_at(0);
    let right = px_at(w - 1);
    // Порядок каналов BGRA: у красного велик третий байт, у синего первый.
    assert!(left[2] > 200 && left[0] < 60, "слева красный: {left:?}");
    assert!(right[0] > 200 && right[2] < 60, "справа синий: {right:?}");
}

#[test]
fn broken_markup_yields_nothing_rather_than_a_blank() {
    assert!(rasterize("<svg", 10.0, 10.0).is_none());
}

/// Pixel (BGRA) at user-space point `(x, y)` of a `vw`-wide drawing.
fn pixel(markup: &str, vw: f32, vh: f32, x: f32, y: f32) -> [u8; 4] {
    let img = rasterize(markup, vw, vh).expect("растрируется");
    let size = img.size(0);
    let bytes = img.as_bytes(0).expect("пиксели доступны");
    let w = u32::from(size.width) as usize;
    let k = w as f32 / vw;
    let i = ((y * k) as usize * w + (x * k) as usize) * 4;
    [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
}

const SVG_OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="200" height="100">"#;

fn doc(body: &str) -> String {
    format!("{SVG_OPEN}{body}</svg>")
}

// Ссылочные циклы ресурсов (CSS Masking 1 §ClipPathElement, SVG 2 §16.3):
// раньше usvg ловил только петли длины 1–2 и на длинных уходил в бесконечную
// рекурсию (переполнение стека, 0xC0000409) или вечный цикл по href.

#[test]
fn clip_path_self_reference_is_ignored_on_the_child() {
    // WPT clip-path-recursion-002: ссылка ребёнка на свой clipPath
    // недействительна, сам clipPath режет по прямоугольнику ребёнка.
    let m = doc(
        r##"<clipPath id="c"><rect x="0" width="50" height="50" clip-path="url(#c)"/></clipPath>
        <rect width="100" height="100" fill="#ff0000" clip-path="url(#c)"/>"##,
    );
    assert!(
        pixel(&m, 200.0, 100.0, 25.0, 25.0)[3] > 200,
        "внутри клипа красный"
    );
    assert_eq!(pixel(&m, 200.0, 100.0, 75.0, 75.0)[3], 0, "вне клипа пусто");
}

#[test]
fn clip_path_three_step_cycle_through_a_mask_does_not_overflow() {
    // WPT clip-path-recursion-001: clip → clip2 → clip0 → clip.
    let m = doc(
        r##"<clipPath id="clip0"><rect width="1" height="1" clip-path="url(#clip)"/></clipPath>
        <clipPath id="clip2"><rect width="100" height="100" clip-path="url(#clip0)"/></clipPath>
        <clipPath id="clip"><rect width="1" height="1" clip-path="url(#clip2)"/></clipPath>
        <mask id="m" x="0" y="0" width="1" height="1" maskContentUnits="objectBoundingBox">
          <rect width="1" height="1" clip-path="url(#clip)"/></mask>
        <circle r="500" mask="url(#m)"/>"##,
    );
    assert_eq!(pixel(&m, 200.0, 100.0, 50.0, 50.0)[3], 0, "ничего не видно");
}

#[test]
fn mixed_clip_and_mask_cycle_does_not_overflow() {
    let m = doc(
        r##"<clipPath id="c"><rect width="50" height="50" mask="url(#m)"/></clipPath>
        <mask id="m"><rect width="200" height="100" fill="white" clip-path="url(#c)"/></mask>
        <rect width="100" height="100" fill="#00ff00" clip-path="url(#c)"/>"##,
    );
    assert!(rasterize(&m, 200.0, 100.0).is_some());
}

#[test]
fn pattern_fill_cycle_and_href_cycle_do_not_overflow() {
    let m = doc(
        r##"<pattern id="a" width="10" height="10" patternUnits="userSpaceOnUse">
          <rect width="10" height="10" fill="url(#b)"/></pattern>
        <pattern id="b" width="10" height="10" patternUnits="userSpaceOnUse">
          <rect width="10" height="10" fill="url(#c)"/></pattern>
        <pattern id="c" width="10" height="10" patternUnits="userSpaceOnUse">
          <rect width="10" height="10" fill="url(#a)"/></pattern>
        <pattern id="h1" xlink:href="#h2"/><pattern id="h2" xlink:href="#h3"/>
        <pattern id="h3" xlink:href="#h2"/>
        <rect width="100" height="100" fill="url(#a)"/>
        <rect x="100" width="100" height="100" fill="url(#h1)"/>"##,
    );
    assert!(rasterize(&m, 200.0, 100.0).is_some());
}

#[test]
fn gradient_href_cycle_terminates() {
    // a → b → c → b: раньше итератор href ходил по b↔c вечно.
    let m = doc(r##"<linearGradient id="a" xlink:href="#b"/>
        <linearGradient id="b" xlink:href="#c"/>
        <linearGradient id="c" xlink:href="#b"/>
        <rect width="100" height="100" fill="url(#a) #0000ff"/>"##);
    assert!(rasterize(&m, 200.0, 100.0).is_some());
}

#[test]
fn use_three_step_cycle_is_skipped() {
    let m = doc(r##"<defs><g id="g1"><use xlink:href="#g2"/></g>
          <g id="g2"><use xlink:href="#g3"/></g>
          <g id="g3"><rect width="10" height="10"/><use xlink:href="#g1"/></g></defs>
        <use xlink:href="#g1"/>
        <rect x="100" width="50" height="50" fill="#ff0000"/>"##);
    assert!(
        pixel(&m, 200.0, 100.0, 120.0, 20.0)[3] > 200,
        "остальное рисуется"
    );
}

#[test]
fn filter_fe_image_cycle_through_a_group_does_not_overflow() {
    let m = doc(r##"<filter id="f"><feImage xlink:href="#g"/></filter>
        <g id="g"><rect width="50" height="50" filter="url(#f)"/></g>"##);
    assert!(rasterize(&m, 200.0, 100.0).is_some());
}

#[path = "tests/external.rs"]
mod external;
