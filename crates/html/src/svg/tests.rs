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
    let nodes = parse(
        r##"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#f00"/></svg>"##,
        "",
    );
    let svg = find_svg(&nodes).unwrap();
    let out = serialize(svg);
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
