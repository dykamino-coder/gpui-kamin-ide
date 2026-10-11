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
            self.clip_shape = Some(format!("pathdef:{rule}:{d}"));
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
