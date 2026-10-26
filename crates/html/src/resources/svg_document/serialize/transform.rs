//! SVG transform и transform-origin из CSS и презентационных атрибутов.

mod origin;

use crate::dom::Element;
use crate::resources::svg_document::clip::reference_box;

#[allow(clippy::too_many_arguments)]
pub(crate) fn element_transform(e: &Element, viewport: (f32, f32)) -> (Option<String>, bool) {
    let attr_of = |name: &str| {
        e.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    let num_attr = |name: &str| attr_of(name).and_then(|v| v.trim().parse::<f32>().ok());
    let (fx, fy, fw, fh) = (
        num_attr("x").unwrap_or(0.0),
        num_attr("y").unwrap_or(0.0),
        num_attr("width").unwrap_or(0.0),
        num_attr("height").unwrap_or(0.0),
    );
    // SVG without a CSS layout box uses the nearest viewport by default.
    // Blink TransformHelper::ComputeReferenceBox and css-transforms-1 agree:
    // fill/stroke boxes use object bounds; view-box uses viewport dimensions.
    let ref_box = match super::svg_box_style(&e.style.used()).0 {
        Some(k @ (1 | 2)) => Some(reference_box(e, k == 2).unwrap_or((fx, fy, fw, fh))),
        _ => None,
    };
    let (bx, by, bw, bh) = ref_box.unwrap_or_else(|| {
        let (w, h) = viewport;
        (0.0, 0.0, w, h)
    });
    let from_box = |v: f32, o: f32| if ref_box.is_some() { o + v } else { v };
    // CSS `transform-origin` сильнее презентационного атрибута
    // (css-transforms-1 §specificity): доли — от той же опорной коробки, что
    // и у атрибута, точки — от её начала при явной коробке, иначе как есть
    // (`transform-box/fill-box-*`, `svg-origin-relative-length-*`).
    let style_origin = match (
        e.style.used().transform_origin,
        e.style.used().transform_origin_px,
    ) {
        (_, (Some(px), Some(py))) => Some((from_box(px, bx), from_box(py, by))),
        (Some((kx, ky)), (px, py)) => Some((
            px.map_or(bx + bw * kx, |v| from_box(v, bx)),
            py.map_or(by + bh * ky, |v| from_box(v, by)),
        )),
        _ => None,
    };
    let origin = style_origin.or_else(|| {
        attr_of("transform-origin")
            .and_then(|raw| origin::parse(raw, (bx, by, bw, bh), ref_box.is_some()))
    });
    // Начальный `transform-origin` SVG-элемента — `0 0` (UA-лист:
    // `*:not(svg), *:not(foreignObject) > svg { transform-origin: 0 0 }`), и
    // отсчитывается он от опорной коробки: при явной `fill-box`/`stroke-box`
    // это её левый верх, а не начало координат (`fill-box-001` target1:
    // `rotate(90deg)` вокруг (0,0) уводил фигуру за кадр).
    let origin = origin.or(ref_box.map(|(x, y, _, _)| (x, y)));
    // Стилевой transform на SVG-ребёнке СИЛЬНЕЕ презентационного атрибута
    // (css-transforms §specificity) — сериализуется атрибутом для
    // растеризатора.
    // Матрица функций В ПОРЯДКЕ ЗАПИСИ (см. `computed::Transform::lin`);
    // проценты сдвига — от опорной коробки фигуры (css-transforms-1
    // §transform-box: доля — от выбранной reference box).
    let style_t = e.style.used().transform.as_ref().map(|t| {
        let unit = t.lin == [[1.0, 0.0], [0.0, 1.0]] && t.tr == [[0.0; 3]; 2];
        if unit {
            return String::new();
        }
        let tx = t.tr[0][0] + bw * t.tr[0][1] + bh * t.tr[0][2];
        let ty = t.tr[1][0] + bw * t.tr[1][1] + bh * t.tr[1][2];
        format!(
            "matrix({} {} {} {} {} {})",
            t.lin[0][0], t.lin[1][0], t.lin[0][1], t.lin[1][1], tx, ty
        )
    });
    // Отдельное свойство `translate` (css-transforms-2 §individual-transforms)
    // действует и на SVG-фигуре — она transformable element
    // (css-transforms-1 §transformable-element), — но до растеризатора не
    // доезжало: матрица выше собирается из `lin`/`tr` свойства `transform`, а
    // `translate:` живёт отдельным полем `Computed::translate`, и его
    // единственный потребитель `paint/apply/` двигает CSS-КОРОБКУ, внутрь `<svg>`
    // не заходя. Оттого `<rect style="translate: 100px 100px">` стоял на
    // месте: наш зелёный — (10,67)-(134,191) точек устройства, эталонный —
    // (135,192)-(259,316), ровно 100 css-точек по обеим осям, и накрываемый
    // красный оставался виден (`translate/translate-in-svg`).
    //
    // Порядок сборки — css-transforms-2 §ctm: «translate, then rotate, then
    // scale, then transform», то есть сдвиг стоит СЛЕВА и от матрицы свойства
    // `transform`, и от презентационного атрибута `transform=` (тот
    // отображается в то же свойство — css-transforms-1 §svg-transform).
    // Доли — от выбранной опорной коробки (§transform-box).
    //
    // Корневой `<svg>` исключён: он обычная CSS-коробка, и `translate:` ему
    // уже сдвигает `paint/apply/` — иначе сдвиг лёг бы дважды.
    let ind_t = e
        .style
        .used()
        .translate
        .filter(|_| e.tag != "svg")
        .and_then(|(x, y)| {
            let axis = |l: crate::style::values::value::Len, base: f32| match l {
                crate::style::values::value::Len::Px(v) => v,
                crate::style::values::value::Len::Pct(k) => k * base,
                _ => 0.0,
            };
            let (dx, dy) = (axis(x, bw), axis(y, bh));
            (dx != 0.0 || dy != 0.0).then_some((dx, dy))
        });
    // Невалидный список преобразований В АТРИБУТЕ (`rotate(90,)`: запятая без
    // аргумента — грамматика `transform-list`, SVG 1.1 §7.6). Атрибут —
    // презентационная форма свойства `transform` (css-transforms-1
    // §svg-transform), и ошибка разбора отбрасывает его целиком, как
    // невалидное объявление. usvg висячую запятую прощает и поворачивал
    // фигуру (`svg-rotate-3args-invalid-002`, `svg-external-styles-014`).
    let attr_bad = attr_of("transform").is_some_and(|t| {
        let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        s.contains(",)") || s.contains("(,") || s.contains(",,")
    });
    let attr_t = attr_of("transform")
        .filter(|_| !attr_bad)
        .map(str::to_string);
    let base_t = style_t.filter(|t| !t.is_empty()).or(attr_t);
    // css-transforms-2 CTM: translate, rotate, scale, then transform list.
    // Keep the attribute as the list when individual properties synthesized
    // an otherwise identity Computed::transform.
    let mut prefix = String::new();
    if let Some((dx, dy)) = ind_t {
        prefix.push_str(&format!("translate({dx} {dy}) "));
    }
    if e.tag != "svg" {
        if let Some(angle) = e.style.used().rotate_prop {
            prefix.push_str(&format!("rotate({}) ", angle.to_degrees()));
        }
        if let Some((x, y)) = e.style.used().scale_prop {
            prefix.push_str(&format!("scale({x} {y}) "));
        }
    }
    let transform = if prefix.is_empty() {
        base_t
    } else {
        Some(format!("{prefix}{}", base_t.unwrap_or_default()))
    };
    let combined = match (origin, transform) {
        (Some((ox, oy)), Some(t)) => Some(format!(
            "translate({ox} {oy}) {t} translate({} {})",
            -ox, -oy
        )),
        // Сдвиг от `translate:` — такой же повод перебить презентационный
        // атрибут, как и своё свойство `transform`: без этой ветки собранная
        // строка терялась, а в разметку уходил нетронутый атрибут.
        (None, Some(t)) if e.style.used().transform.is_some() || ind_t.is_some() => Some(t),
        _ => None,
    };
    (combined, attr_bad)
}

#[cfg(test)]
mod tests {
    //! Regression checks for the SVG reference box and CTM assembly.

    use super::element_transform;
    use crate::dom::{Element, Node, parse};
    use crate::style::values::value::Len;

    fn rect(markup: &str) -> Element {
        fn find(nodes: &[Node]) -> Option<Element> {
            nodes.iter().find_map(|node| match node {
                Node::Element(e) if e.tag == "rect" => Some(e.clone()),
                Node::Element(e) => find(&e.children),
                Node::Text(_) => None,
            })
        }
        find(&parse(markup, "")).expect("rect")
    }

    #[test]
    fn default_percentages_use_viewport_not_shape_bounds() {
        let mut e = rect(r#"<svg><rect x="25" y="40" width="50" height="60"/></svg>"#);
        e.style.used_mut().transform = Some(Default::default());
        e.style.used_mut().transform_origin = Some((0.25, 0.5));
        e.style.used_mut().translate = Some((Len::Pct(0.5), Len::Pct(0.25)));
        assert_eq!(
            element_transform(&e, (200.0, 300.0)).0.as_deref(),
            Some("translate(50 150) translate(100 75)  translate(-50 -150)")
        );
    }

    #[test]
    fn fill_box_lengths_start_at_object_bounds() {
        let e = rect(
            r#"<svg><rect x="25" y="40" width="50" height="60" style="transform-box:fill-box" transform-origin="10px 20px" transform="scale(2)"/></svg>"#,
        );
        assert_eq!(
            element_transform(&e, (300.0, 150.0)).0.as_deref(),
            Some("translate(35 60) scale(2) translate(-35 -60)")
        );
    }

    #[test]
    fn individual_properties_precede_the_presentation_transform() {
        let mut e = rect(r#"<svg><rect transform="translate(10 20)"/></svg>"#);
        e.style.used_mut().transform = Some(Default::default());
        e.style.used_mut().translate = Some((Len::Px(3.0), Len::Px(4.0)));
        e.style.used_mut().rotate_prop = Some(std::f32::consts::FRAC_PI_2);
        e.style.used_mut().scale_prop = Some((2.0, 3.0));
        assert_eq!(
            element_transform(&e, (300.0, 150.0)).0.as_deref(),
            Some("translate(3 4) rotate(90) scale(2 3) translate(10 20)")
        );
    }
}
