//! Computed::apply_one: transform*, translate/rotate/scale, perspective*, offset-*.

use crate::style::computed::*;
use crate::style::values::value::Len;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_transform(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // --- Сдвиг и тень текста ------------------------------------------
            "translate" => {
                let mut it = v.split_whitespace();
                let x = it.next().and_then(Len::parse).unwrap_or(Len::Px(0.0));
                let y = it.next().and_then(Len::parse).unwrap_or(Len::Px(0.0));
                self.translate = Some((x, y));
            }

            // --- Преобразования -----------------------------------------------
            // motion-1 §2: свойства пути хранятся СЫРЫМИ. Сэмплер зовётся
            // после каскада (`dom::walk`), когда известны и путь, и точка
            // отсчёта, и авторский `transform` — раньше собрать нечего.
            "offset-path" => self.offset_path = (v.trim() != "none").then(|| v.trim().to_string()),
            // `<length-percentage>`: смесь `calc(12.5% + 200px)` доживает
            // индексом арены — доля решается длиной пути во втором проходе
            // (`motion::path_css`; offset-path-shape-xywh-002).
            "offset-distance" => self.offset_distance = Len::parse_mixed(v.trim()),
            "offset-rotate" => self.offset_rotate = Some(v.trim().to_string()),
            "offset-anchor" => self.offset_anchor = Some(v.trim().to_string()),
            "offset-position" => self.offset_position = Some(v.trim().to_string()),
            "transform" if v.trim() == "inherit" => self.inherit_bits |= inh::TRANSFORM,
            "transform-origin" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::TRANSFORM_ORIGIN
            }
            "transform" if has_font_units(v) => self.transform_raw = Some(v.to_string()),
            "transform-origin" if has_font_units(v) => {
                self.transform_origin_raw = Some(v.to_string())
            }
            "transform" => {
                let mut t = self.transform.unwrap_or_default();
                // Невалидный аргумент отбрасывает ВСЁ объявление (CSS-каскад),
                // а не превращается в ноль (`scale(invalid)` схлопывал фигуру,
                // хотя обязан быть проигнорирован — svg-document-styles-005).
                let mut invalid = false;
                for call in v.split(')') {
                    let Some((name, arg)) = call.split_once('(') else {
                        continue;
                    };
                    let (name, arg) = (name.trim(), arg.trim());
                    let nums: Vec<f32> = arg
                        .split(',')
                        .filter_map(|n| {
                            n.trim()
                                .trim_end_matches("grad")
                                .trim_end_matches("turn")
                                .trim_end_matches("deg")
                                .trim_end_matches("rad")
                                .trim_end_matches("px")
                                .trim_end_matches('%')
                                .parse::<f32>()
                                .ok()
                        })
                        .collect();
                    // Угол i-го аргумента с ЕГО единицей: grad — 400 на
                    // оборот, turn — целый оборот; rad проверяется после
                    // grad («grad» кончается на «rad»).
                    let angle_at = |i: usize| -> f32 {
                        let t = arg.split(',').nth(i).unwrap_or("").trim();
                        let v = nums.get(i).copied().unwrap_or(0.0);
                        if t.ends_with("grad") {
                            v * std::f32::consts::PI / 200.0
                        } else if t.ends_with("turn") {
                            v * std::f32::consts::TAU
                        } else if t.ends_with("rad") {
                            v
                        } else {
                            v.to_radians()
                        }
                    };
                    // Доля i-го аргумента: процент — сотая (scale(50%) = 0.5).
                    let frac_at = |i: usize, def: f32| -> f32 {
                        let t = arg.split(',').nth(i).unwrap_or("").trim();
                        match nums.get(i) {
                            None => def,
                            Some(v) if t.ends_with('%') => v / 100.0,
                            Some(v) => *v,
                        }
                    };
                    if nums.is_empty() && !arg.is_empty() && name != "none" {
                        invalid = true;
                    }
                    let first = nums.first().copied().unwrap_or(0.0);
                    let angle = angle_at(0);
                    // Имена функций регистронезависимы (css-transforms-1
                    // §7, CSS Syntax §4): `scale3D`, `rotatex`, `translateY`
                    // — одно и то же.
                    let lower = name.to_ascii_lowercase();
                    let name = lower.as_str();
                    // `matrix()`/`matrix3d()` — только числа (css-transforms-1
                    // §matrix): единица делает объявление невалидным
                    // (`transform-matrix-008`).
                    if matches!(name, "matrix" | "matrix3d")
                        && arg.split(',').any(|a| a.trim().parse::<f32>().is_err())
                    {
                        invalid = true;
                    }
                    match name {
                        "rotate" | "rotatez" => {
                            t.rotate_rad += angle;
                            t.push(Transform::rot(angle), NO_SHIFT);
                        }
                        "scale" => {
                            let sx = frac_at(0, 1.0);
                            let sy = frac_at(1, sx);
                            t.scale.0 *= sx;
                            t.scale.1 *= sy;
                            t.push(Transform::diag(sx, sy), NO_SHIFT);
                        }
                        "skew" => {
                            let ay = angle_at(1);
                            t.skew_rad.0 += angle;
                            t.skew_rad.1 += ay;
                            t.push([[1.0, angle.tan()], [ay.tan(), 1.0]], NO_SHIFT);
                        }
                        // matrix3d(...16): 4x4 в СТОЛБЦОВОМ порядке
                        // (css-transforms-2 §matrix3d). Сплющивание —
                        // вычёркивание третьей строки и третьего столбца,
                        // то есть ровно matrix(m11 m12 m21 m22 m41 m42) =
                        // аргументы 1,2,5,6,13,14. Раньше вызов не подходил
                        // под `nums.len() == 6` и молча пропадал целиком.
                        "matrix3d" if nums.len() == 16 => {
                            let (a, b, c2, d) = (nums[0], nums[1], nums[4], nums[5]);
                            let (e2, f2) = (nums[12], nums[13]);
                            t.translate.0 += e2;
                            t.translate.1 += f2;
                            t.m33 *= nums[10];
                            // Плоское вложение (третья строка/столбец и строка w
                            // единичные) остаётся на 2D-пути; иначе — полная
                            // 4×4 в СТОЛБЦОВОМ порядке аргументов:
                            // m4[строка][столбец] = nums[столбец·4 + строка]
                            // (m14/m24 = nums[3]/nums[7] — перспективная
                            // строка, transform3d-matrix3d-003/-004; m44 =
                            // nums[15] ≠ 1 — деление на w, -005).
                            let flat2d = [2usize, 3, 6, 7, 8, 9, 11, 14]
                                .iter()
                                .all(|&i| nums[i] == 0.0)
                                && nums[10] == 1.0
                                && nums[15] == 1.0;
                            if flat2d {
                                t.push([[a, c2], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
                            } else {
                                t.push2([[a, c2], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
                                let mut f = [[0.0f32; 4]; 4];
                                for col in 0..4 {
                                    for row in 0..4 {
                                        f[row][col] = nums[col * 4 + row];
                                    }
                                }
                                t.has_3d = true;
                                t.push4(f, [[0.0; 2]; 4]);
                            }
                        }
                        // matrix(a b c d e f): разложение на компоненты
                        // (перенос, поворот, масштаб, скос) — QR-подобное,
                        // как в css-transforms §16 (декомпозиция).
                        "matrix" if nums.len() == 6 => {
                            let (a, b, c, d, e2, f2) =
                                (nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]);
                            t.translate.0 += e2;
                            t.translate.1 += f2;
                            t.push([[a, c], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
                            let sx = (a * a + b * b).sqrt();
                            if sx > 1e-6 {
                                t.rotate_rad += b.atan2(a);
                                let det = a * d - b * c;
                                let sy = det / sx;
                                t.scale.0 *= sx;
                                t.scale.1 *= sy;
                                let shear = (a * c + b * d) / det.max(1e-6);
                                t.skew_rad.0 += shear.atan();
                            }
                        }
                        "skewx" => {
                            t.skew_rad.0 += angle;
                            t.push([[1.0, angle.tan()], [0.0, 1.0]], NO_SHIFT);
                        }
                        "skewy" => {
                            t.skew_rad.1 += angle;
                            t.push([[1.0, 0.0], [angle.tan(), 1.0]], NO_SHIFT);
                        }
                        "scalex" => {
                            let k = frac_at(0, 1.0);
                            t.scale.0 *= k;
                            t.push(Transform::diag(k, 1.0), NO_SHIFT);
                        }
                        "scaley" => {
                            let k = frac_at(0, 1.0);
                            t.scale.1 *= k;
                            t.push(Transform::diag(1.0, k), NO_SHIFT);
                        }
                        // Поворот вокруг оси экрана БЕЗ перспективы — это
                        // прямая проекция на плоскость, то есть сжатие поперёк
                        // оси ровно на косинус угла (css-transforms-2 §11):
                        // `rotateX(60deg)` даёт половину высоты. Перспективы у
                        // нас нет, и приближением это не является — при
                        // `perspective: none` так считает и браузер.
                        "rotatex" => {
                            t.scale.1 *= angle.cos();
                            t.m33 *= angle.cos();
                            t.push2(Transform::diag(1.0, angle.cos()), NO_SHIFT);
                            t.has_3d = true;
                            t.push4(Transform::rot4(1.0, 0.0, 0.0, angle), [[0.0; 2]; 4]);
                        }
                        "rotatey" => {
                            t.scale.0 *= angle.cos();
                            t.m33 *= angle.cos();
                            t.push2(Transform::diag(angle.cos(), 1.0), NO_SHIFT);
                            t.has_3d = true;
                            t.push4(Transform::rot4(0.0, 1.0, 0.0, angle), [[0.0; 2]; 4]);
                        }
                        // Поворот вокруг произвольной оси, сплющенный на
                        // плоскость экрана: это ТОЧНО верхний 2x2 блок
                        // матрицы Родрига (css-transforms-2
                        // §3d-transform-rendering — сплющивание вычёркивает
                        // третью строку и третий столбец). Прежняя формула
                        // `1 - |y|·(1 - cos a)` теряла внедиагональные члены
                        // и для косой оси давала не поворот, а сжатие.
                        // Единица угла берётся из САМОГО аргумента: `contains
                        // ("rad")` срабатывал на «grad» и читал 100grad как
                        // 100 радиан.
                        "rotate3d" => {
                            let (x, y, z) = (
                                nums.first().copied().unwrap_or(0.0),
                                nums.get(1).copied().unwrap_or(0.0),
                                nums.get(2).copied().unwrap_or(0.0),
                            );
                            if (x * x + y * y + z * z) > 0.0 {
                                let a = angle_at(3);
                                let (l, m33) = Transform::axis_rot(x, y, z, a);
                                let len = (x * x + y * y + z * z).sqrt();
                                t.rotate_rad += a * z / len;
                                t.scale.0 *= l[0][0];
                                t.scale.1 *= l[1][1];
                                t.m33 *= m33;
                                // Ось строго Z — обычный `rotate`, остаётся на
                                // 2D-пути (css-transform-3d-rotate3d-Z-*).
                                if x == 0.0 && y == 0.0 {
                                    t.push(l, NO_SHIFT);
                                } else {
                                    t.push2(l, NO_SHIFT);
                                    t.has_3d = true;
                                    t.push4(Transform::rot4(x, y, z, a), [[0.0; 2]; 4]);
                                }
                            }
                        }
                        // Третья ось видна только через `perspective()` в том
                        // же списке — копится в 4×4; `translateZ(0)` остаётся
                        // на 2D-пути (GPU-подсказка, не геометрия).
                        "translatez" => {
                            if first != 0.0 {
                                t.has_3d = true;
                                t.push4(Transform::translate4(0.0, 0.0, first), [[0.0; 2]; 4]);
                            }
                        }
                        // perspective(d): m34 = −1/d; d < 1px считается 1px
                        // (css-transforms-2 §perspective(): «treated as 1px»;
                        // perspective-zero, -zero-point-five).
                        "perspective" => {
                            if !nums.is_empty() {
                                t.has_3d = true;
                                t.push4(Transform::perspective4(first.max(1.0)), [[0.0; 2]; 4]);
                            }
                        }
                        // Масштаб по Z видом не правит, но участвует в m33
                        // (обратная сторона) и вырождает 4×4 при нуле.
                        "scalez" => {
                            t.m33 *= first;
                            if first != 1.0 {
                                t.has_3d = true;
                                t.push4(Transform::scale4(1.0, 1.0, first), [[0.0; 2]; 4]);
                            }
                        }
                        "scale3d" => {
                            let sy = nums.get(1).copied().unwrap_or(1.0);
                            let sz = nums.get(2).copied().unwrap_or(1.0);
                            t.scale.0 *= first;
                            t.scale.1 *= sy;
                            t.m33 *= sz;
                            if sz == 1.0 {
                                t.push(Transform::diag(first, sy), NO_SHIFT);
                            } else {
                                t.push2(Transform::diag(first, sy), NO_SHIFT);
                                t.has_3d = true;
                                t.push4(Transform::scale4(first, sy, sz), [[0.0; 2]; 4]);
                            }
                        }
                        "translate3d" => {
                            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
                            let mut v = NO_SHIFT;
                            for (i, dest) in [&mut t.translate.0, &mut t.translate.1]
                                .into_iter()
                                .enumerate()
                            {
                                if let Some(raw) = parts.get(i) {
                                    let d =
                                        raw.trim_end_matches("px").parse::<f32>().unwrap_or(0.0);
                                    *dest += d;
                                    v[i][0] = d;
                                }
                            }
                            let z = parts
                                .get(2)
                                .and_then(|raw| raw.trim_end_matches("px").parse::<f32>().ok())
                                .unwrap_or(0.0);
                            if z == 0.0 {
                                t.push(Transform::diag(1.0, 1.0), v);
                            } else {
                                t.push2(Transform::diag(1.0, 1.0), v);
                                t.has_3d = true;
                                t.push4(Transform::translate4(v[0][0], v[1][0], z), [[0.0; 2]; 4]);
                            }
                        }
                        // Проценты в сдвиге считаются от СВОЕГО размера —
                        // на этом стоит типовое центрирование
                        // `translate(-50%, -50%)`. Раньше процент срезался как
                        // единица, и элемент уезжал на 50 точек.
                        "translate" | "translatex" | "translatey" => {
                            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
                            let mut v = NO_SHIFT;
                            let mut axis = |i: usize, x: bool| {
                                let Some(raw) = parts.get(i) else { return };
                                let value = raw
                                    .trim_end_matches("px")
                                    .trim_end_matches('%')
                                    .parse::<f32>()
                                    .unwrap_or(0.0);
                                let row = if x { 0 } else { 1 };
                                let dest = if raw.ends_with('%') {
                                    // Доля своей ширины по x, высоты по y.
                                    v[row][1 + row] = value / 100.0;
                                    if x {
                                        &mut t.translate_pct.0
                                    } else {
                                        &mut t.translate_pct.1
                                    }
                                } else {
                                    v[row][0] = value;
                                    if x {
                                        &mut t.translate.0
                                    } else {
                                        &mut t.translate.1
                                    }
                                };
                                *dest += value / if raw.ends_with('%') { 100.0 } else { 1.0 };
                            };
                            match name {
                                "translatex" => axis(0, true),
                                "translatey" => axis(0, false),
                                _ => {
                                    axis(0, true);
                                    axis(1, false);
                                }
                            }
                            t.push(Transform::diag(1.0, 1.0), v);
                        }
                        // Скос выразить нечем: матрица GPUI хранит поворот и
                        // масштаб, но не сдвиг осей.
                        _ => {}
                    }
                }
                // `none` is no transform at all (css-transforms-1 §transform
                // property): it neither establishes a stacking context nor a
                // containing block (`transform-stacking-002`); a value with no
                // transform function at all is invalid and is ignored
                // (`transform-stacking-003`: `transform: quasit`).
                if !v.contains('(') {
                    if matches!(
                        v.trim().to_ascii_lowercase().as_str(),
                        "none" | "initial" | "unset"
                    ) {
                        self.transform = None;
                    }
                } else if !invalid {
                    self.transform = Some(t);
                }
            }
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
