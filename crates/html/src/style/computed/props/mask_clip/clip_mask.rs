//! Ветвь clip-path / mask / mask-image из Computed::apply_mask_clip: сокращение mask, маска-изображение, опорная коробка, url(#id); базовые фигуры — в basic_shape.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_clip_mask(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        if key == "mask" && mask_shorthand::apply(self, v) {
            return;
        }
        // Маска-ИЗОБРАЖЕНИЕ (url/градиент): источник хранится строкой,
        // растрируется при сборке группы, альфа умножается в композите
        // буфера (css-masking §7.1; mask-image-1a).
        if key != "clip-path" {
            if v.contains("-gradient(") || v.contains("url(") {
                // Слоёв может быть несколько (`url(a), url(b)`) —
                // строка хранится ЦЕЛИКОМ, разбор при отрисовке.
                self.mask_image = Some(v.trim().to_string());
            }
            // Сокращение `mask` несёт и укладку (css-masking §7.9).
            if v.contains("no-repeat") {
                self.mask_no_repeat = Some((true, true));
            }
        }
        // Круг и эллипс — это скруглённый прямоугольник с радиусом в
        // половину стороны; `inset(… round R)` — он же с заданным
        // радиусом. Многоугольник прямоугольной маской не выразить —
        // его гасит по форме сборка буфера группы.
        let v = v.trim();
        // Опорная коробка формы (css-masking §1.3.1.1): слово до или
        // после функции; точки полигона отсчитываются от неё
        // (clip-path-polygon-008: margin-box).
        // У элемента с CSS-коробкой `fill-box` = content-box,
        // `stroke-box`/`view-box` = border-box (css-masking-1 §1.3.1.1).
        self.clip_ref = if v.contains("margin-box") {
            Some(1)
        } else if v.contains("padding-box") {
            Some(2)
        } else if v.contains("content-box") || v.contains("fill-box") {
            Some(3)
        } else if v.contains("border-box") || v.contains("stroke-box") || v.contains("view-box") {
            Some(0)
        } else {
            self.clip_ref
        };
        if key == "clip-path" {
            self.clip_bare_box = !v.is_empty()
                && !v.contains('(')
                && v.split_whitespace().all(|w| w.ends_with("-box"));
        }
        // `clip-path: shape(...)` (css-shapes-2): команды хранятся
        // с `;` вместо запятых (по ним режутся слои), доли резолвит
        // отрисовка по размеру коробки.
        if key == "clip-path"
            && let Some(rest) = v.trim().strip_prefix("shape(")
        {
            // После скобки может стоять опорная коробка
            // (`shape(...) content-box`) — режем по ПОСЛЕДНЕЙ скобке.
            let rest = match rest.rfind(')') {
                Some(i) => &rest[..i],
                None => rest,
            }
            .trim();
            let (rule, body) = match rest.split_once(' ') {
                Some((r @ ("nonzero" | "evenodd"), b)) => (r, b),
                _ => ("nonzero", rest),
            };
            self.clip_shape = Some(format!("shapedef:{rule}:{}", body.replace(',', ";")));
        }
        // `clip-path: path(правило, 'd')` — контур SVG: форма
        // растрируется маской покрытия; запятые в d заменяются
        // пробелами (грамматика SVG им равнозначна), потому что по
        // запятым верхнего уровня режутся СЛОИ маски.
        if key == "clip-path"
            && let Some(rest) = v.trim().strip_prefix("path(")
        {
            let rest = match rest.rfind(')') {
                Some(i) => &rest[..i],
                None => rest,
            }
            .trim();
            let (rule, d) = match rest.split_once(',') {
                Some((r, d)) if matches!(r.trim(), "nonzero" | "evenodd") => (r.trim(), d.trim()),
                _ => ("nonzero", rest),
            };
            let d = d.trim_matches(|c| c == '"' || c == '\'').replace(',', " ");
            // Ломаная до восьми вершин — та же геометрия, что `polygon()`,
            // и рисуется тем же шейдером: иначе один контур давал два разных
            // сглаживания (`corner-shape-*`: фигура коробки идёт
            // многоугольником, а её эталон пишет `path()`).
            match line_polygon(&d) {
                Some(points) if rule == "nonzero" && (3..=8).contains(&points.len()) => {
                    self.clip_polygon = Some(points);
                    self.clip_polygon_evenodd = false;
                }
                _ => self.clip_shape = Some(format!("pathdef:{rule}:{d}")),
            }
        }
        // `clip-path: url(#id)` — ссылка на <clipPath>: форма
        // растрируется маской покрытия при отрисовке.
        if key == "clip-path"
            && let Some(url) = parse_url(v)
            && let Some(id) = url.strip_prefix('#')
        {
            self.clip_shape = Some(format!("clipref:{id}"));
        }
        self.clip_basic_shape(v);
    }
}

/// Вершины пути из одних абсолютных `M`/`L` и замыкающего `Z` (одна
/// фигура); иначе None.
fn line_polygon(d: &str) -> Option<Vec<(Len, Len)>> {
    let mut spaced = String::with_capacity(d.len() + 8);
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() && !matches!(ch, 'e' | 'E') {
            spaced.push(' ');
            spaced.push(ch);
            spaced.push(' ');
        } else {
            spaced.push(ch);
        }
    }
    let mut toks = spaced.split_whitespace();
    if toks.next()? != "M" {
        return None;
    }
    let mut pts = Vec::new();
    let mut closed = false;
    while let Some(t) = toks.next() {
        if closed {
            return None;
        }
        let x = match t {
            "L" => toks.next()?,
            "Z" | "z" => {
                closed = true;
                continue;
            }
            n => n,
        };
        let y = toks.next()?;
        pts.push((Len::Px(x.parse().ok()?), Len::Px(y.parse().ok()?)));
    }
    (!pts.is_empty()).then_some(pts)
}

#[cfg(test)]
mod line_polygon_tests {
    #[test]
    fn only_absolute_lines_make_a_polygon() {
        let p = super::line_polygon("M 112 40 L 120 70 L120 90 Z").unwrap();
        assert_eq!(p.len(), 3);
        assert!(super::line_polygon("M 0 0 C 1 1 2 2 3 3 Z").is_none());
        assert!(super::line_polygon("M 0 0 l 5 5 L 1 0 Z").is_none());
        assert!(super::line_polygon("M 0 0 L 5 5 L 1 0 Z M 9 9 L 8 8 L 7 9 Z").is_none());
    }
}
