//! Computed::apply_one: filter, blend, isolation, animation, transition, will-change, contain*, content, counters, quotes, lists, svg, cursor and the rest.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_effects(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // `pre` сохраняет переводы строк — это не то же самое, что запрет
            // переноса: раньше `pre` помечался как `nowrap`, и текст склеивался
            // в одну строку.
            "cursor" => self.cursor = Some(v.to_string()),
            // Заливка SVG-геометрии: свойство презентации доезжает до
            // разметки при растеризации (SVG 2 §presentation attributes).
            "fill" => self.svg_fill = Some(v.to_string()),
            // Обводка — то же семейство. Значение уходит в разметку как есть:
            // разбирать цвет здесь незачем, его знает usvg.
            "stroke" => self.svg_stroke = Some(v.to_string()),
            // Вычисленное `stroke-width` — «the absolute length, or percentage»
            // (fill-stroke-3 §stroke-width): `calc()` из точек сворачивается
            // здесь. usvg его не понимает и рисовал толщину по умолчанию 1
            // (`zoom/stroke`: эталон `calc(6px * var(--scale))`). В Blink —
            // `UnzoomedLength` через `ConvertUnzoomedLength`
            // (css_properties.json5:6150-6159).
            "stroke-width" => {
                self.svg_stroke_width = Some(match crate::style::values::value::calc_pct_px(v) {
                    Some((pct, px)) if pct == 0.0 => format!("{px}"),
                    _ => v.to_string(),
                });
            }
            // `x`/`y` — геометрические СВОЙСТВА фигуры (SVG 2 §Geometry).
            // У HTML-коробки таких свойств нет, поэтому имена свободны, а в
            // разметку они уходят только внутри SVG-поддерева (гейт в svg.rs).
            "x" => self.svg_x = crate::style::values::value::Len::parse(v),
            "y" => self.svg_y = crate::style::values::value::Len::parse(v),
            "caption-side" => self.caption_bottom = Some(v.eq_ignore_ascii_case("bottom")),
            "list-style-position" => {
                self.list_style_inside = Some(v.trim() == "inside");
            }
            "list-style" | "list-style-type" => list_style::apply(self, key, v),
            "backdrop-filter" => {
                // Тот же `<filter-value-list>`, что у `filter`
                // (filter-effects-2 §BackdropFilterProperty): разбор общий,
                // размытие идёт своим проходом, цветовые функции — матрицей.
                let mut tmp = Self::default();
                tmp.apply_one("filter", v);
                let f = tmp.filter.unwrap_or_else(Filter::neutral);
                self.backdrop_blur = (f.blur > 0.0).then_some(f.blur);
                self.backdrop_color = f.color_matrix().map(|_| Filter { blur: 0.0, ..f });
                // Корень подложки — любое значение, кроме `none`
                // (filter-effects-2 Overview.bs:119; Blink
                // paint_property_tree_builder.cc:1846): тождественная
                // `invert(0)` матрицы не даёт, но корнем остаётся
                // (`backdrop-filter-backdrop-root-backdrop-filter`).
                self.backdrop_filter_set = !v.trim().eq_ignore_ascii_case("none");
                // `url(#id)` — SVG `<filter>` (`render::svg_filter_matrix`).
                self.backdrop_ref = tmp.filter_ref;
            }
            // `view-transition-name` не `none` — корень подложки
            // (css-view-transitions-1 Overview.bs:577-582 «Form a backdrop
            // root»; Blink paint_property_tree_builder.cc:1858-1862
            // `NeedsEffectForViewTransition`).
            "view-transition-name" => {
                self.vt_name = !v.trim().eq_ignore_ascii_case("none");
            }
            // css-will-change-1 §2.1: обещанное свойство даёт коробке то, что
            // дало бы его неначальное значение, — содержащий блок для
            // `absolute`/`fixed` и контекст наложения (`will-change-fixpos-cb-*`,
            // `-abspos-cb-*`, `-fixedpos-cb-*`, `-stacking-context-z-index-2/3`).
            // `position` даёт блок только абсолютам (`-fixpos-cb-position-1`).
            // `auto`, `scroll-position`, `contents` и прочие свойства — ноль:
            // `will-change: height` не меняет ничего (`-fixpos-cb-height-1`).
            // Здесь же признак корня подложки (filter-effects-2
            // Overview.bs:122: «will-change specifying any property that
            // would create a Backdrop Root on non-initial value»). Арма ОДНА:
            // вторая с тем же ключом в этом `match` недостижима — так с
            // b47ecf2 разряды `wc::*` не ставились вовсе.
            "will-change" => {
                let mut bits = 0u8;
                let mut root = false;
                for part in v.split(',') {
                    let name = part.trim().to_ascii_lowercase();
                    root |= matches!(
                        name.as_str(),
                        "opacity"
                            | "filter"
                            | "mask"
                            | "mask-image"
                            | "-webkit-mask"
                            | "-webkit-mask-image"
                            | "mask-border"
                            | "clip-path"
                            | "-webkit-clip-path"
                            | "backdrop-filter"
                            | "-webkit-backdrop-filter"
                            | "mix-blend-mode"
                            | "view-transition-name"
                    );
                    bits |= match name.as_str() {
                        "transform"
                        | "translate"
                        | "rotate"
                        | "scale"
                        | "perspective"
                        | "-webkit-perspective"
                        | "transform-style"
                        | "offset-path"
                        | "contain" => wc::BOX,
                        "filter" | "backdrop-filter" | "-webkit-backdrop-filter" => {
                            wc::CB_ABS | wc::CB_FIXED | wc::STACK
                        }
                        "position" => wc::CB_ABS | wc::STACK,
                        "opacity"
                        | "isolation"
                        | "mix-blend-mode"
                        | "clip-path"
                        | "-webkit-clip-path"
                        | "mask"
                        | "mask-image"
                        | "-webkit-mask"
                        | "-webkit-mask-image"
                        | "mask-border"
                        | "view-transition-name" => wc::STACK,
                        "z-index" => wc::STACK_Z,
                        _ => 0,
                    };
                }
                self.will_change = bits;
                self.will_change_root = root;
            }
            // Свисающая пунктуация: знак выходит ЗА край коробки, чтобы край
            // текста читался ровным. Значения складываются: `first last`.
            // `zoom` (css-viewport-1 §zoom-property): число или доля, ноль
            // читается единицей («A 0 value is treated as if it was 1»),
            // отрицательное недействительно. `normal`/`reset` — старые слова
            // IE/WebKit, равны единице. Здесь ТОЛЬКО запись поля: применяет
            // его проход `zoom::resolve`, слияние стилей поля не читает.
            "zoom" => {
                let k = match v {
                    "normal" | "reset" => Some(1.0),
                    _ => match v.strip_suffix('%') {
                        Some(p) => p.trim().parse::<f32>().ok().map(|p| p / 100.0),
                        None => v.parse::<f32>().ok(),
                    },
                };
                if let Some(k) = k.filter(|k| k.is_finite() && *k >= 0.0) {
                    self.zoom = Some(if k == 0.0 { 1.0 } else { k });
                }
            }

            // --- Прочее ------------------------------------------------------
            "pointer-events" => self.pointer_events_none = Some(v == "none"),
            "table-layout" => self.table_fixed = Some(v == "fixed"),

            // --- Псевдоэлементы и шрифт ---------------------------------------
            "counter-reset" | "counter-increment" | "counter-set" => counters::apply(self, key, v),
            "quotes" => quotes::apply(self, v),
            "content" => {
                match v {
                    // ПУСТАЯ строка — не то же самое, что `none`: коробка
                    // псевдоэлемента создаётся, просто в ней нет знаков. На
                    // этом стоит целый приём эталонов WPT — `::after` с
                    // `content: ""` и `inset: 0` накрывает красное зелёным
                    // (`overflow-wrap-anywhere-001` и родня).
                    "none" | "normal" => {
                        self.content = None;
                        self.content_none = Some(v == "none");
                    }
                    // Негодная запись НЕ применяется вовсе, прежнее значение
                    // остаётся (CSS 2.1 §4.1.8): иначе мусор вроде
                    // `counter(a,b,c)` печатался литералом и `counters-002`
                    // показывал слово FAIL.
                    other => {
                        if let Some(list) = parse_content(other) {
                            self.content = Some(list);
                        }
                    }
                }
            }
            "accent-color" => self.accent_color = Color::parse(v),

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
            "container-type" => {
                // css-conditional-5 §container-type:
                // `normal | [ [ size | inline-size ] || scroll-state ]`.
                // `size` — «Applies style containment and size containment to
                // the principal box»; `inline-size` — то же, но обособление
                // одной строчной оси. Обособления РАСКЛАДКИ в этом списке НЕТ,
                // и ставить его нельзя: у нас `contain_layout` делает элемент
                // содержащим блоком для `absolute` и `fixed`
                // (`inline::establishes_cb`, `inline::inherit`), а корпус
                // требует обратного — `no-layout-containment-abspos`,
                // `-fixedpos`, `-baseline` (все 0.00) проверяют, что абсолют,
                // `fixed` и базовая линия проходят СКВОЗЬ контейнер.
                //
                // Правило `@container` этим шагом ещё не разбирается: здесь
                // только побочное действие свойства. Оно само по себе отвечает
                // за `contain-size-014` (коробка с `container-type: size`
                // обязана мериться пустой, а росла по `<img height=200>`) и
                // делает истинным `@supports (container-type: …)`, на котором
                // висят `chrome-legacy-skip-recalc` и обе
                // `svg-*-no-size-container`.
                let mut size = false;
                let mut inline = false;
                let mut known = false;
                for w in v.split_whitespace() {
                    match w {
                        "size" => {
                            size = true;
                            known = true;
                        }
                        "inline-size" => {
                            inline = true;
                            known = true;
                        }
                        // `scroll-state` — контейнер по состоянию прокрутки, к
                        // размеру отношения не имеет; `normal` — начальное
                        // значение. Оба грамматически годны и не делают ничего.
                        "scroll-state" | "normal" => known = true,
                        // Слово вне грамматики (например `anchored` из
                        // css-anchor-position-2) делает объявление негодным
                        // ЦЕЛИКОМ (CSS 2.1 §4.1.7), а не «частично годным»:
                        // `anchored-fallback-style-containment` (0.01) обязана
                        // остаться нетронутой.
                        _ => return,
                    }
                }
                if !known {
                    return;
                }
                // Только ВЗВОД: `container-type: normal` не имеет права снять
                // обособление, объявленное в том же блоке через `contain`, —
                // это разные свойства, и начальное значение одного ничего не
                // отменяет у другого.
                if size {
                    self.contain_size = Some(true);
                }
                if inline {
                    self.contain_inline_size = Some(true);
                }
                if size || inline {
                    self.contain_style = Some(true);
                    // css-conditional-5 §container-type: `size`/`inline-size`
                    // делают элемент контейнером запросов размера. Такой
                    // элемент обособлен, и css-grid-2 §subgrid-listing лишает
                    // его подсеточности. Признак ОТДЕЛЬНЫЙ от `contain_size`,
                    // потому что `contain: size` подсетку не отменяет; и
                    // отдельный от `contain_layout`, который у нас делает
                    // элемент содержащим блоком для абсолюта, а корпус требует
                    // обратного (`no-layout-containment-abspos` и родня).
                    self.container_size_query = true;
                }
            }
            "content-visibility" => {
                // `hidden` = size+layout+paint containment, содержимое
                // пропускается целиком (css-contain-2 §4). `auto` для
                // reftest без прокрутки всегда «релевантен» = visible.
                if v.trim() == "hidden" {
                    self.contain_size = Some(true);
                    self.contain_paint = Some(true);
                    self.contain_layout = Some(true);
                    self.skip_content = Some(true);
                }
            }
            "contain-intrinsic-size" => {
                // Одно или два значения; `auto <длина>` — длина как запас.
                let nums: Vec<f32> = v
                    .split_whitespace()
                    .filter(|w| *w != "auto")
                    .filter_map(|w| match Len::parse(w) {
                        Some(Len::Px(px)) => Some(px),
                        _ => None,
                    })
                    .collect();
                self.contain_intrinsic = match nums.as_slice() {
                    [one] => (Some(*one), Some(*one)),
                    [w, h, ..] => (Some(*w), Some(*h)),
                    _ => (None, None),
                };
            }
            "contain-intrinsic-width" => {
                if let Some(Len::Px(w)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.contain_intrinsic.0 = Some(w);
                }
            }
            "contain-intrinsic-height" => {
                if let Some(Len::Px(h)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.contain_intrinsic.1 = Some(h);
                }
            }
            "contain-intrinsic-block-size" => {
                if let Some(Len::Px(h)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.logical().ci_block = Some(h);
                }
            }
            "contain-intrinsic-inline-size" => {
                if let Some(Len::Px(w)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.logical().ci_inline = Some(w);
                }
            }
            "mix-blend-mode" => {
                // Номера совпадают с формулами в шейдере: смешивание считается
                // при сборке буфера группы, поэтому доступны все режимы CSS,
                // включая те, где цвет берётся целиком (тон, насыщенность).
                self.blend = match v {
                    "multiply" => Some(1),
                    "screen" => Some(2),
                    "darken" => Some(3),
                    "lighten" => Some(4),
                    "overlay" => Some(5),
                    "color-dodge" => Some(6),
                    "color-burn" => Some(7),
                    "hard-light" => Some(8),
                    "soft-light" => Some(9),
                    "difference" => Some(10),
                    "exclusion" => Some(11),
                    "hue" => Some(12),
                    "saturation" => Some(13),
                    "color" => Some(14),
                    "luminosity" => Some(15),
                    _ => Some(0),
                }
            }
            "isolation" => self.isolate = Some(v == "isolate"),
            "user-select" | "-webkit-user-select" => self.no_select = Some(matches!(v, "none")),
            _ => *hit = false,
        }
    }
}
