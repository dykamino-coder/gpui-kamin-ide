//! Computed::apply_transform, хвост цепочки: rotate/scale, transform-origin, backface-visibility, perspective*, transform-style, transform-box, vector-effect. Ветви в исходном порядке после ветвей offset-* и transform.

use super::*;

mod perspective;

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
            _ => self.apply_transform_perspective(key, val, v, hit),
        }
    }
}
