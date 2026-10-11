//! Computed::apply_effects, продолжение цепочки: animation*, transition*, resize, filter, contain. Ветви в исходном порядке; не совпавший ключ уходит в apply_effects_containment.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_effects_timing(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // --- Время --------------------------------------------------------
            "animation"
            | "animation-name"
            | "animation-duration"
            | "animation-iteration-count"
            | "animation-direction"
            | "animation-delay"
            | "animation-play-state" => {
                let mut a = self.animation.clone().unwrap_or(AnimSpec {
                    name: String::new(),
                    seconds: 0.0,
                    infinite: false,
                    alternate: false,
                    delay: 0.0,
                    paused: false,
                    names: Vec::new(),
                });
                // В сокращении второе время — задержка (css-animations §5).
                let mut times = 0usize;
                let set_time = |a: &mut AnimSpec, sec: f32, times: &mut usize| match key {
                    "animation-delay" => a.delay = sec,
                    "animation-duration" => a.seconds = sec,
                    _ => {
                        if *times == 0 {
                            a.seconds = sec;
                        } else {
                            a.delay = sec;
                        }
                        *times += 1;
                    }
                };
                for token in v.split_whitespace() {
                    if let Some(sec) = token.strip_suffix("ms").and_then(|n| n.parse::<f32>().ok())
                    {
                        set_time(&mut a, sec / 1000.0, &mut times);
                    } else if let Some(sec) =
                        token.strip_suffix('s').and_then(|n| n.parse::<f32>().ok())
                    {
                        set_time(&mut a, sec, &mut times);
                    } else if token == "paused" {
                        a.paused = true;
                    } else if token == "infinite" {
                        a.infinite = true;
                    } else if token == "alternate" {
                        a.alternate = true;
                    } else if token.parse::<f32>().is_err()
                        && !matches!(
                            token,
                            "linear"
                                | "ease"
                                | "ease-in"
                                | "ease-out"
                                | "ease-in-out"
                                | "normal"
                                | "reverse"
                                | "both"
                                | "forwards"
                                | "backwards"
                                | "running"
                                | "paused"
                                | "none"
                        )
                    {
                        a.name = token.to_string();
                    }
                }
                // `animation-name: a, b` — СПИСОК (css-animations-1 §3: при
                // общем свойстве побеждает имя, стоящее в списке позже). Цикл
                // выше оставил в `name` последнее имя — одиночный путь прежний;
                // весь список нужен слоению остановленных анимаций (`dom.rs`).
                if key == "animation-name" {
                    let names: Vec<String> = v
                        .split(',')
                        .map(|n| n.trim().to_string())
                        .filter(|n| !n.is_empty() && n != "none")
                        .collect();
                    a.names = if names.len() > 1 { names } else { Vec::new() };
                }
                // Свойства без имени (`animation-play-state` до сокращения)
                // копят состояние: имя может прийти следующей декларацией.
                self.animation = Some(a);
            }
            "transition" | "transition-duration" => {
                // Из записи перехода нужна только длительность: какие свойства
                // меняются, видно по разнице стилей.
                self.transition = v.split_whitespace().find_map(|t| {
                    t.strip_suffix("ms")
                        .and_then(|n| n.parse::<f32>().ok())
                        .map(|ms| ms / 1000.0)
                        .or_else(|| t.strip_suffix('s').and_then(|n| n.parse::<f32>().ok()))
                });
            }
            "resize" => {
                self.resize = match v {
                    "both" => Some((true, true)),
                    "horizontal" => Some((true, false)),
                    "vertical" => Some((false, true)),
                    _ => None,
                }
            }
            "filter" => {
                // `drop-shadow(<color>? && <length>{2,3})` (filter-effects-1
                // §funcdef-filter-drop-shadow) несёт скобки цвета внутри —
                // режется по балансу скобок, а не по первой `)`. Значения —
                // как у box-shadow, но 3-я длина — СИГМА: радиус box-shadow
                // вдвое больше.
                self.drop_shadow = v.find("drop-shadow(").and_then(|at| {
                    let rest = &v[at + "drop-shadow(".len()..];
                    let mut depth = 1usize;
                    let end = rest.char_indices().find_map(|(i, ch)| {
                        match ch {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    return Some(i);
                                }
                            }
                            _ => {}
                        }
                        None
                    })?;
                    parse_shadows(&rest[..end]).first().map(|sh| Shadow {
                        blur: sh.blur * 2.0,
                        ..*sh
                    })
                });
                let mut f = self.filter.unwrap_or_else(Filter::neutral);
                for call in v.split(')') {
                    let Some((name, arg)) = call.split_once('(') else {
                        continue;
                    };
                    let name = name.trim();
                    let arg = arg.trim();
                    // Доля пишется и процентом, и числом.
                    let amount = || -> f32 {
                        match arg.strip_suffix('%') {
                            Some(n) => n.trim().parse::<f32>().unwrap_or(100.0) / 100.0,
                            None => arg.parse::<f32>().unwrap_or(1.0),
                        }
                    };
                    match name {
                        "url" => {
                            let id = arg.trim_matches(|c| c == '"' || c == '\'').trim();
                            if let Some(id) = id.strip_prefix('#') {
                                self.filter_ref = Some(id.to_string());
                            }
                        }
                        "grayscale" => f.grayscale = amount(),
                        "brightness" => f.brightness = amount(),
                        "saturate" => f.saturate = amount(),
                        "invert" => f.invert = amount(),
                        "sepia" => f.sepia = amount(),
                        "opacity" => f.opacity = amount(),
                        "hue-rotate" => {
                            f.hue_rotate = arg.trim_end_matches("deg").parse().unwrap_or(0.0)
                        }
                        "blur" => f.blur = arg.trim_end_matches("px").trim().parse().unwrap_or(0.0),
                        "contrast" => f.contrast = amount(),
                        // `drop-shadow` и цветовые матрицы — не наш случай.
                        _ => {}
                    }
                }
                self.filter = Some(f);
            }
            "contain" => {
                // Разбор по словам: подстрочный поиск ловил «size» в
                // «inline-size» и не видел paint внутри `content`
                // (css-contain-1 §3.1: strict = size layout paint style,
                // content = layout paint style).
                let mut bits = (false, false, false, false);
                let mut inline_only = false;
                for w in v.split_whitespace() {
                    match w {
                        // `paint` и `strict` обрезают содержимое по коробке —
                        // это ровно то, что делает скрытое переполнение;
                        // `size` считает коробку ПУСТОЙ: её размер задают
                        // явные свойства и `contain-intrinsic-size`.
                        "size" => bits.0 = true,
                        "inline-size" => inline_only = true,
                        "layout" => bits.1 = true,
                        "paint" => bits.2 = true,
                        "style" => bits.3 = true,
                        "strict" => bits = (true, true, true, true),
                        "content" => {
                            bits.1 = true;
                            bits.2 = true;
                            bits.3 = true;
                        }
                        _ => {}
                    }
                }
                self.contain_size = Some(bits.0);
                self.contain_inline_size = Some(inline_only);
                self.contain_layout = Some(bits.1);
                self.contain_paint = Some(bits.2);
                self.contain_style = Some(bits.3);
            }
            _ => self.apply_effects_containment(key, val, v, hit),
        }
    }
}
