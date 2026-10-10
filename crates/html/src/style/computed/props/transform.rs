//! Computed::apply_one: transform*, translate/rotate/scale, perspective*, offset-*.

use crate::style::computed::*;
use crate::style::values::value::Len;

mod functions;
mod functions_3d;
mod individual;
use functions::transform_function;
use functions_3d::transform_function_3d;

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
                    transform_function(&mut t, name, arg, &nums, first, angle, &angle_at, &frac_at);
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
            _ => self.apply_transform_individual(key, val, v, hit),
        }
    }
}
