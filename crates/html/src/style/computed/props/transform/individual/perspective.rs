//! Computed::apply_transform_individual, хвост цепочки: backface-visibility, perspective, perspective-origin, transform-style, transform-box, vector-effect. Ветви в исходном порядке.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_transform_perspective(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
            // Трёхмерной сцены нет: без объёмных преобразований перспектива
            // ничего не меняет, поэтому разбирается и не делает ничего.
            // Трёхмерной сцены нет, но обратная сторона видна и на плоской
            // проекции: `rotateY(180deg)` — это scaleX(-1), и элемент с
            // `backface-visibility: hidden` обязан исчезнуть
            // (css-transforms-2 §backface-visibility, признак m33 < 0).
            "backface-visibility" => self.backface_hidden = Some(v == "hidden"),
            // `perspective` (css-transforms-2 §perspective-property): длина в
            // точках, «values less than 1px must be treated as 1px» — так и
            // `perspective: 0` (perspective-zero-2/-3, transform3d-
            // perspective-005). `none`, `inherit` и относительные единицы
            // сюда не доезжают (None). Ячейка заводится здесь, при разборе:
            // у родителя и его детей будет один и тот же Rc.
            "perspective" => {
                self.perspective = match Len::parse(v) {
                    Some(Len::Px(d)) => Some(d.max(1.0)),
                    _ => None,
                };
                self.perspective_frame = self.perspective.map(|_| PerspectiveFrame::default());
            }
            // `perspective-origin` (§perspective-origin-property) — та же
            // грамматика <position>, что у `transform-origin` двумя осями:
            // ключевые слова несут свою ось, длина остаётся точками до
            // отрисовки, доля — от коробки самого элемента (она же — коробка
            // родителя для его детей).
            "perspective-origin" => {
                let axis = |t: &str, default: f32| -> f32 {
                    match t {
                        "left" | "top" => 0.0,
                        "center" => 0.5,
                        "right" | "bottom" => 1.0,
                        other => match Len::parse(other) {
                            Some(Len::Pct(p)) => p,
                            _ => default,
                        },
                    }
                };
                let px_axis = |t: &str| -> Option<f32> {
                    match Len::parse(t) {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    }
                };
                let mut xs: Option<&str> = None;
                let mut ys: Option<&str> = None;
                let mut free: Vec<&str> = vec![];
                for t in v.split_whitespace() {
                    match t {
                        "left" | "right" => xs = Some(t),
                        "top" | "bottom" => ys = Some(t),
                        other => free.push(other),
                    }
                }
                let mut free = free.into_iter();
                let first = xs.or_else(|| free.next()).unwrap_or("center");
                let second = ys.or_else(|| free.next()).unwrap_or("center");
                self.perspective_origin_px = (px_axis(first), px_axis(second));
                self.perspective_origin = Some((axis(first, 0.5), axis(second, 0.5)));
            }
            // `transform-style` (css-transforms-2 §transform-style-property):
            // `preserve-3d` держит детей в одном объёмном контексте с собой.
            // Ячейка заводится здесь, при разборе, — тогда у `e.style`
            // владельца, у его `merged` и у `inherited` детей один и тот же
            // `Rc`, а `inline::inherit` начинает с `own.clone()`, поэтому
            // внукам ячейка не достаётся: плоский ребёнок обрывает контекст
            // (css-transforms-2 §3d-rendering-context, лист контекста).
            "transform-style" => {
                self.preserve_3d = Some(v.trim() == "preserve-3d");
                self.frame_3d = match self.preserve_3d {
                    Some(true) => Some(Frame3d::default()),
                    _ => None,
                };
            }
            // css-transforms-1 §transform-box: `fill-box` переносит опорную
            // коробку и НАЧАЛО отсчёта на bounding box фигуры; по умолчанию
            // (`view-box`) длины в `transform-origin` считаются от вьюпорта.
            "transform-box" => {
                self.transform_box_fill = Some(v.trim() == "fill-box");
                self.transform_box = match v.trim() {
                    "view-box" => Some(0),
                    "fill-box" | "content-box" => Some(1),
                    "stroke-box" | "border-box" => Some(2),
                    _ => None,
                };
            }
            // SVG 2 §vector-effect: `non-scaling-stroke` — толщина обводки в
            // точках экрана; нужна опорной коробке и толщине в `svg.rs`.
            "vector-effect" => self.svg_non_scaling = Some(v.trim() == "non-scaling-stroke"),
            _ => *hit = false,
        }
    }
}
