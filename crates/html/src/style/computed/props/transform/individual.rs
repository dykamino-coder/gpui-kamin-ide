//! Computed::apply_transform, хвост цепочки: rotate/scale, transform-origin, backface-visibility, perspective*, transform-style, transform-box, vector-effect. Ветви в исходном порядке после ветвей offset-* и transform.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_transform_individual(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): доводить `rotate:`/`scale:` до
            // матрицы отрисовки отдельными полями (`rotate_prop`/`scale_prop`)
            // и прятать вырожденную матрицу. Срез 2327 пар
            // (transforms/contain/overflow/masking/position): вместе с
            // `backface-visibility` вышло 1563 -> 1571 (+14/-6), без него —
            // 1563 -> 1575 (+13/-1). То есть сам этот рукав дал ОДНУ пару
            // (`individual-transform-3`) против ШЕСТИ потерь, все —
            // анимационные: `rotate-explicit-and-implicit-keyframes`,
            // `scale-explicit-and-implicit-keyframes`,
            // `scale-and-rotate-both-specified-on-animation-keyframes`,
            // `change-rotate-property`, `change-scale-property`. Эталоны этих
            // тестов ждут КОНЕЧНОЕ состояние анимации, а мы рисуем начальное:
            // рукав вернётся вместе с проигрыванием ключевых кадров.
            "rotate" => {
                let mut t = self.transform.unwrap_or_default();
                let raw = v.trim();
                let value = raw
                    .trim_end_matches("deg")
                    .trim_end_matches("rad")
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(0.0);
                t.rotate_rad = if raw.contains("rad") {
                    value
                } else {
                    value.to_radians()
                };
                self.transform = Some(t);
                // Угол вокруг оси z — ещё и отдельным полем: в матрицу
                // отрисовки его приставляет `transformed()` СЛЕВА от списка
                // `transform` (css-transforms-2 §ctm п.4). Рукав из ★ выше
                // возвращён вместе с запеканием остановленных кадров
                // (`render.rs`, `bake_frozen`); его шесть потерь 05.09 —
                // скриптовые пары («вне цели: скрипт» в `rep-all-v219.txt`).
                // Ось (`x 45deg`, `0 1 0 44deg`) плоскому пути не выразима.
                let angle_only = raw.split_whitespace().count() == 1
                    && raw
                        .trim_end_matches("deg")
                        .trim_end_matches("rad")
                        .trim()
                        .parse::<f32>()
                        .is_ok();
                self.rotate_prop = angle_only.then_some(t.rotate_rad);
            }
            "scale" => {
                let mut t = self.transform.unwrap_or_default();
                // Процент — это доля: `scale: 150%` равно 1.5, а не 150.
                let nums: Vec<f32> = v
                    .split_whitespace()
                    .filter_map(|n| match n.strip_suffix('%') {
                        Some(p) => p.parse::<f32>().ok().map(|v| v / 100.0),
                        None => n.parse::<f32>().ok(),
                    })
                    .collect();
                let x = nums.first().copied().unwrap_or(1.0);
                t.scale = (x, nums.get(1).copied().unwrap_or(x));
                self.transform = Some(t);
                // …и отдельным полем для матрицы отрисовки (css-transforms-2
                // §ctm п.5). Ноль по третьей оси делает матрицу необратимой, и
                // элемент не рисуется (css-transforms-1 §transform-rendering;
                // `individual-transform-3`: `scale: 1 1 0`) — плоский путь
                // выражает это нулевым масштабом, как `scale(0)`.
                self.scale_prop = (!nums.is_empty()).then(|| {
                    if nums.get(2).is_some_and(|z| *z == 0.0) {
                        (0.0, 0.0)
                    } else {
                        t.scale
                    }
                });
            }
            "transform-origin" => {
                // Точка отсчёта хранится ДОЛЯМИ коробки. Точечная запись
                // (`transform-origin: 0 0`, `20px 40px`) до неё не доводилась
                // и молча превращалась в центр — скос и поворот шли вокруг
                // другой точки (`css-skew-001`). Точки в доли переводит
                // отрисовка (`transform_origin_px`) — размер известен там.
                let axis = |t: &str, default: f32| -> f32 {
                    match t {
                        "left" | "top" => 0.0,
                        "center" => 0.5,
                        "right" | "bottom" => 1.0,
                        other => match Len::parse(other) {
                            Some(Len::Pct(p)) => p,
                            // Точки заданы — доля НОЛЬ, а не центр: отрисовка
                            // складывает долю с точками (css-transforms-1 §5.2).
                            Some(Len::Px(_)) => 0.0,
                            _ => pct_px_pair(other).map_or(default, |(p, _)| p),
                        },
                    }
                };
                let px_axis = |t: &str| -> Option<f32> {
                    match Len::parse(t) {
                        Some(Len::Px(v)) => Some(v),
                        _ => pct_px_pair(t).map(|(_, x)| x),
                    }
                };
                // Ключевые слова несут СВОЮ ось (css-transforms-1 §5.2):
                // одиночное `top` значит `center top`, `top left` = `left top`.
                let mut xs: Option<&str> = None;
                let mut ys: Option<&str> = None;
                let mut free: Vec<&str> = vec![];
                // Резать вне скобок: `calc(50px + 50%)` — одно значение,
                // а `split_whitespace` рассыпал его на три слова.
                let parts = split_outside_parens(v);
                for t in &parts {
                    match t.as_str() {
                        "left" | "right" => xs = Some(t.as_str()),
                        "top" | "bottom" => ys = Some(t.as_str()),
                        other => free.push(other),
                    }
                }
                let mut free = free.into_iter();
                let first = xs.or_else(|| free.next()).unwrap_or("center");
                let second = ys.or_else(|| free.next()).unwrap_or("center");
                self.transform_origin_px = (px_axis(first), px_axis(second));
                self.transform_origin = Some((axis(first, 0.5), axis(second, 0.5)));
                // Третье значение — только <length>, z точки отсчёта
                // (css-transforms-2 §transform-origin); видна лишь объёмному
                // пути (transform3d-translate3d-001: `0 0 0` против эталона
                // `10px 30px -10px`).
                self.transform_origin_z = free.next().and_then(px_axis);
            }
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
