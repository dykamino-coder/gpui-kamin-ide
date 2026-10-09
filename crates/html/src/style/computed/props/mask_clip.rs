//! Computed::apply_one: mask*, clip, clip-path, shape-*.

use crate::style::computed::*;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_mask_clip(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // `will-change` (css-will-change-1 §2.1) разбирается ОДНОЙ армой
            // рядом с `backdrop-filter`: вторая арма того же ключа в этом
            // `match` недостижима.
            "shape-outside" => {
                let t = v.trim();
                if t != "none" {
                    self.shape_outside = Some(t.to_string());
                }
            }
            "shape-margin" => self.shape_margin = Len::parse(v.trim()),
            "shape-image-threshold" => {
                self.shape_threshold = v.trim().parse::<f32>().ok().map(|t| t.clamp(0.0, 1.0));
            }
            // Устаревшее `clip` (CSS 2.1): rect с запятыми или пробелами;
            // `auto` в позиции — соответствующий край коробки.
            "clip" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::CLIP;
                    return;
                }
                if let Some(rest) = v.trim().strip_prefix("rect(") {
                    let rest = rest.trim_end_matches(')');
                    // Разделители — ЛИБО три запятые, ЛИБО одни пробелы:
                    // смешанная запись невалидна, свойство игнорируется
                    // (clip-rect-comma-002..004).
                    let commas = rest.matches(',').count();
                    if commas != 0 && commas != 3 {
                        return;
                    }
                    let parts: Vec<&str> = rest
                        .split([',', ' '])
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .collect();
                    if parts.len() == 4 {
                        let side = |t: &str| match t {
                            "auto" => None,
                            // Нулевая длина есть ноль в любой единице, и
                            // `rect(-0em, …)` обязан обрезать, а не читаться
                            // как `auto` (`visufx/clip-076…102`). Кегель на
                            // этом шаге ещё не известен, поэтому ненулевые
                            // относительные единицы по-прежнему мимо.
                            _ => match Len::parse(t) {
                                Some(Len::Px(v)) => Some(v),
                                Some(
                                    Len::Em(v)
                                    | Len::Ex(v)
                                    | Len::Ch(v)
                                    | Len::Ic(v)
                                    | Len::Lh(v)
                                    | Len::Vh(v)
                                    | Len::Vw(v),
                                ) if v == 0.0 => Some(0.0),
                                _ => None,
                            },
                        };
                        self.clip_rect = Some([
                            side(parts[0]),
                            side(parts[1]),
                            side(parts[2]),
                            side(parts[3]),
                        ]);
                        let raw = |t: &str| match t {
                            "auto" => None,
                            _ => Len::parse(t),
                        };
                        self.clip_len = Some([
                            raw(parts[0]),
                            raw(parts[1]),
                            raw(parts[2]),
                            raw(parts[3]),
                        ]);
                    }
                }
            }
            "mask-size" | "-webkit-mask-size" => mask_size::apply(self, v),
            "mask-mode" => {
                self.mask_luminance = Some(v.trim() == "luminance");
                self.mask_alpha_mode = Some(v.trim() == "alpha");
            }
            "mask-type" => self.mask_type_alpha = Some(v.trim() == "alpha"),
            "mask-composite" | "-webkit-mask-composite" => {
                self.mask_composite = Some(
                    v.split(',')
                        .map(|t| match t.trim() {
                            "subtract" => 1,
                            "intersect" => 2,
                            "exclude" => 3,
                            _ => 0,
                        })
                        .collect(),
                );
            }
            // SVG-коробки (css-masking-1 §7.10/7.11, `<geometry-box>`):
            // у элемента с CSS-коробкой fill-box = content-box, stroke-box и
            // view-box = border-box; у SVG-ребёнка их считает
            // `svg::masked_layers` (mask-clip-2, mask-origin-3).
            "mask-origin" | "-webkit-mask-origin" => {
                self.mask_origin = match v.trim() {
                    "padding-box" => Some(2),
                    "content-box" => Some(3),
                    "fill-box" => Some(4),
                    "stroke-box" => Some(5),
                    "view-box" => Some(6),
                    _ => Some(0),
                }
            }
            "mask-clip" | "-webkit-mask-clip" => {
                self.mask_clip = match v.trim() {
                    "padding-box" => Some(2),
                    "content-box" => Some(3),
                    "fill-box" => Some(4),
                    "stroke-box" => Some(5),
                    "view-box" => Some(6),
                    "no-clip" => Some(255),
                    _ => Some(0),
                }
            }
            "mask-repeat" | "-webkit-mask-repeat" => {
                // Пооосно (css-backgrounds §3.4): `repeat-x` = repeat по x,
                // одна плитка по y; два слова — оси по порядку.
                //
                // Запись — СПИСОК по слоям (css-masking-1 §7.6:
                // `<repeat-style>#`). Прежде строка резалась только по
                // пробелам, и запятая уезжала внутрь самого слова
                // (`"no-repeat,"` не равно `"no-repeat"`): весь список
                // `no-repeat, repeat` читался как `repeat` по обеим осям, и
                // плитка первого слоя мостила всю коробку
                // (mask-image-3b/3e 1.27, mask-position-5 1.25).
                let one = |layer: &str| {
                    let t: Vec<&str> = layer.split_whitespace().collect();
                    match t.as_slice() {
                        ["repeat-x"] => (false, true),
                        ["repeat-y"] => (true, false),
                        [a] => (*a == "no-repeat", *a == "no-repeat"),
                        [a, b] => (*a == "no-repeat", *b == "no-repeat"),
                        _ => (false, false),
                    }
                };
                let list: Vec<(bool, bool)> =
                    crate::css::split_args(v).iter().map(|l| one(l)).collect();
                self.mask_no_repeat = Some(list.first().copied().unwrap_or((false, false)));
                self.mask_repeat_list = (!list.is_empty()).then_some(list);
                let mode = |w: &str| match w {
                    "space" => 2u8,
                    "round" => 3,
                    _ => 0,
                };
                let modes: Vec<(u8, u8)> = crate::css::split_args(v)
                    .iter()
                    .map(|l| {
                        let t: Vec<&str> = l.split_whitespace().collect();
                        match t.as_slice() {
                            [a] => (mode(a), mode(a)),
                            [a, b] => (mode(a), mode(b)),
                            _ => (0, 0),
                        }
                    })
                    .collect();
                self.mask_repeat_modes =
                    modes.iter().any(|&(x, y)| x > 0 || y > 0).then_some(modes);
            }
            "mask-position" | "-webkit-mask-position" => {
                let word = |t: &str| match t {
                    "left" | "top" => Some(Len::Pct(0.0)),
                    "center" => Some(Len::Pct(0.5)),
                    "right" | "bottom" => Some(Len::Pct(1.0)),
                    _ => Len::parse(t),
                };
                // Запись — СПИСОК по слоям (css-masking-1 §7.7:
                // `<position>#`): `top, bottom` — своя точка у каждого слоя.
                let one = |layer: &str| -> Option<(Len, Len, bool, bool)> {
                    let toks: Vec<&str> = layer.split_whitespace().collect();
                    // Четырёхзначная запись — пары «край смещение»: `left 40%
                    // bottom 60%` (css-backgrounds-3 §3.6); от правого/нижнего
                    // края доля зеркалится.
                    if toks.len() == 4 {
                        let pair = |edge: &str, off: &str| -> Option<(Len, bool)> {
                            let l = Len::parse(off)?;
                            Some((l, matches!(edge, "right" | "bottom")))
                        };
                        let horiz = matches!(toks[0], "left" | "right");
                        let (xe, xo, ye, yo) = if horiz {
                            (toks[0], toks[1], toks[2], toks[3])
                        } else {
                            (toks[2], toks[3], toks[0], toks[1])
                        };
                        let ((x, fx), (y, fy)) = (pair(xe, xo)?, pair(ye, yo)?);
                        return Some((x, y, fx, fy));
                    }
                    let first = toks.first().and_then(|t| word(t))?;
                    // ОДИНОЧНОЕ слово осевое (css-backgrounds-3 §3.6):
                    // `top` — это `center top`, а не `top center`. Прежде оно
                    // уходило в ось X, и плитка вставала в (0, середина)
                    // вместо (середина, 0) — проба `target/probe/pmask-axis.html`
                    // против эталона корпуса даёт 1.71 против 0.03 у `center top`.
                    let (x, y) = match (toks.len(), toks.get(1).and_then(|t| word(t))) {
                        (_, Some(second)) => (first, second),
                        (1, None) if matches!(toks[0], "top" | "bottom") => {
                            (Len::Pct(0.5), first)
                        }
                        (_, None) => (first, Len::Pct(0.5)),
                    };
                    Some((x, y, false, false))
                };
                let list: Vec<(Len, Len, bool, bool)> = crate::css::split_args(v)
                    .iter()
                    .filter_map(|l| one(l))
                    .collect();
                if let Some((x, y, fx, fy)) = list.first().copied() {
                    self.mask_pos = Some((x, y));
                    self.mask_pos_far = (fx, fy);
                }
                self.mask_pos_list = (!list.is_empty()).then_some(list);
            }
            "clip-path" | "mask" | "mask-image" => {
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
                } else if v.contains("border-box")
                    || v.contains("stroke-box")
                    || v.contains("view-box")
                {
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
                        Some((r, d)) if matches!(r.trim(), "nonzero" | "evenodd") => {
                            (r.trim(), d.trim())
                        }
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
                if let Some(rest) = v.strip_prefix("polygon(") {
                    let rest = match rest.rfind(')') {
                        Some(i) => &rest[..i],
                        None => rest,
                    };
                    // Первым может стоять правило намотки (css-shapes-1
                    // §3.1): `polygon(evenodd, …)`. Вершин любое число —
                    // больше восьми (предел шейдера) и `evenodd` уходят
                    // растровой маской-путём при отрисовке.
                    // css-shapes-2 §basic-shape-polygon: `<fill-rule>? [round
                    // <length>]?` may lead the list; the radius rounds every
                    // vertex (a rectangle becomes a rounded rectangle).
                    let mut round = None;
                    let (rule, rest) = match rest.trim_start().split_once(',') {
                        Some((head, tail))
                            if head.split_whitespace().next().is_some_and(|w| {
                                matches!(w, "nonzero" | "evenodd" | "round")
                            }) =>
                        {
                            let mut words = head.split_whitespace().peekable();
                            let rule = match words.peek() {
                                Some(&w @ ("nonzero" | "evenodd")) => {
                                    words.next();
                                    w
                                }
                                _ => "nonzero",
                            };
                            if words.next() == Some("round") {
                                round = words.next().and_then(Len::parse);
                            }
                            (rule, tail)
                        }
                        _ => ("nonzero", rest),
                    };
                    self.clip_round_len = round.filter(|l| matches!(l, Len::Px(_)));
                    let points: Vec<(Len, Len)> = rest
                        .split(',')
                        .filter_map(|pair| {
                            let mut it = pair.split_whitespace();
                            let x = Len::parse(it.next()?)?;
                            let y = Len::parse(it.next()?)?;
                            Some((x, y))
                        })
                        .collect();
                    if points.len() >= 3 {
                        self.clip_polygon = Some(points);
                        self.clip_polygon_evenodd = rule == "evenodd";
                    }
                } else if v.starts_with("circle(") || v.starts_with("ellipse(") {
                    // Форма растрируется маской: радиус и центр считаются от
                    // размеров коробки при отрисовке.
                    //
                    // Голая `circle()`/`ellipse()` шла запасным путём
                    // скруглённой коробки, где радиус — половина МЕНЬШЕЙ из
                    // заданных `width`/`height`. При `box-sizing: content-box`
                    // это содержимое, а не опорная коробка: у 40×40 с
                    // отбивкой 20 и рамкой 20 выходило 20 вместо 60, и
                    // `circle()` рисовалась слегка скруглённым прямоугольником
                    // вместо вписанного круга. `shape_params` с пустым списком
                    // радиусов даёт ровно `closest-side` (css-shapes-1
                    // §3.1.1.3) от border-box — то, что и требуется.
                    self.clip_shape = Some(format!("shape:{}", v.trim()));
                } else if let Some(rest) = v.strip_prefix("rect(") {
                    // Края видимой области (css-shapes-1 §basic-shape):
                    // top/right/bottom/left от верхнего-левого угла, auto —
                    // край опорной коробки; хвост `round R` — скругление.
                    let inner = rest.trim_end_matches(')');
                    let sides_part = inner.split("round").next().unwrap_or("").trim();
                    let vals: Vec<Option<Len>> = sides_part
                        .split_whitespace()
                        .map(|t| if t == "auto" { None } else { Len::parse(t) })
                        .collect();
                    if vals.len() == 4 {
                        self.clip_edges = Some([vals[0], vals[1], vals[2], vals[3]]);
                        self.clip_round_len = inner.split("round").nth(1).and_then(uniform_round);
                        self.clip_round = inner
                            .split("round")
                            .nth(1)
                            .and_then(uniform_round)
                            .and_then(|l| match l {
                                Len::Px(v) => Some(v),
                                _ => None,
                            })
                            .or(self.clip_round);
                    }
                } else if let Some(rest) = v.strip_prefix("xywh(") {
                    let inner = rest.trim_end_matches(')');
                    let sides_part = inner.split("round").next().unwrap_or("").trim();
                    let vals: Vec<Len> = sides_part
                        .split_whitespace()
                        .filter_map(Len::parse)
                        .collect();
                    if vals.len() == 4 {
                        self.clip_xywh = Some([vals[0], vals[1], vals[2], vals[3]]);
                        self.clip_round_len = inner.split("round").nth(1).and_then(uniform_round);
                        self.clip_round = inner
                            .split("round")
                            .nth(1)
                            .and_then(uniform_round)
                            .and_then(|l| match l {
                                Len::Px(v) => Some(v),
                                _ => None,
                            })
                            .or(self.clip_round);
                    }
                } else if let Some(rest) = v.strip_prefix("inset(") {
                    // Стороны вырезки (css-shapes-1 §3.1.1.1): 1-4 значения
                    // TRBL до слова round; доли резолвит отрисовка.
                    let sides_part = rest
                        .trim_end_matches(')')
                        .split("round")
                        .next()
                        .unwrap_or("")
                        .trim();
                    let vals: Vec<Len> = sides_part
                        .split_whitespace()
                        .filter_map(Len::parse)
                        .collect();
                    let pick = |i: usize| -> Len {
                        match vals.len() {
                            1 => vals[0],
                            2 => vals[i % 2],
                            3 => vals[i.min(2)].to_owned(),
                            4 => vals[i],
                            _ => Len::Px(0.0),
                        }
                    };
                    if !vals.is_empty() {
                        self.clip_inset = Some([pick(0), pick(1), pick(2), pick(3)]);
                    }
                    let inner = rest.trim_end_matches(')');
                    self.clip_round_len = inner.split("round").nth(1).and_then(uniform_round);
                    let radius = inner
                        .split("round")
                        .nth(1)
                        .and_then(uniform_round)
                        .and_then(|l| match l {
                            Len::Px(v) => Some(v),
                            Len::Pct(p) => Some(p),
                            l @ (Len::Em(_)
                            | Len::EmPx(..)
                            | Len::Ch(_)
                            | Len::Ic(_)
                            | Len::Ex(_)
                            | Len::Lh(_)
                            | Len::LhPx(..)) => crate::metrics::fallback_len_px(l, "", 16.0),
                            Len::Vw(_) | Len::Vh(_) | Len::Calc(_) | Len::Anchor(_) => None,
                            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
                        });
                    self.clip_round = Some(radius.unwrap_or(0.0));
                }
            }
            _ => *hit = false,
        }
    }
}
