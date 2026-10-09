//! Computed::apply_one: width/height/min/max, logical sizes, padding*, margin*, aspect-ratio, box-sizing.

use crate::style::computed::*;
use crate::style::values::value::Len;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_box_model(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "box-sizing" => self.border_box = Some(v == "border-box"),

            // Отрицательная длина делает объявление размера невалидным
            // (CSS 2.1 §10): `max-height: -1px` доезжал до раскладки и
            // схлопывал коробку в ноль. У `min-*` отрицательное поднимает
            // сама раскладка, но объявление всё равно отбрасывается.
            "width" | "height" | "min-width" | "min-height" if calc_size_arg(v).is_some() => {
                let i = match key {
                    "width" => 0,
                    "height" => 1,
                    "min-width" => 2,
                    _ => 3,
                };
                let slot = match i {
                    0 => &mut self.width,
                    1 => &mut self.height,
                    2 => &mut self.min_width,
                    _ => &mut self.min_height,
                };
                match calc_size_arg(v) {
                    Some(CalcSize::Fixed(px)) => {
                        *slot = Some(Len::Px(px));
                        self.calc_size[i] = None;
                    }
                    Some(CalcSize::Over(f)) => {
                        *slot = if i < 2 { Some(Len::Auto) } else { None };
                        self.calc_size[i] = Some(f);
                    }
                    None => {}
                }
            }
            "width" => {
                self.width_inherit = v == "inherit";
                self.stretch_size[0] = stretch_keyword(v);
                self.fit_arg[0] = fit_content_arg(v);
                self.calc_size[0] = None;
                assign_size(&mut self.width, v);
            }
            "height" => {
                self.height_inherit = v == "inherit";
                self.stretch_size[1] = stretch_keyword(v);
                self.calc_size[1] = None;
                assign_size(&mut self.height, v);
            }
            // Пределы не наследуются, но `inherit` берёт значение родителя
            // явно (§6.2.1). Без этой ветки `assign_size` стирал слот в
            // `None`, и `max-height: inherit` снимал предел вовсе.
            "min-width" => {
                self.minmax_inherit[0] = v == "inherit";
                self.stretch_size[2] = stretch_keyword(v);
                self.fit_arg[1] = fit_content_arg(v);
                self.calc_size[2] = None;
                assign_size(&mut self.min_width, v);
            }
            "min-height" => {
                self.minmax_inherit[1] = v == "inherit";
                self.stretch_size[4] = stretch_keyword(v);
                self.calc_size[3] = None;
                assign_size(&mut self.min_height, v);
            }
            "max-width" => {
                self.minmax_inherit[2] = v == "inherit";
                self.stretch_size[3] = stretch_keyword(v);
                self.fit_arg[2] = fit_content_arg(v);
                assign_size(&mut self.max_width, v);
            }
            "max-height" => {
                self.minmax_inherit[3] = v == "inherit";
                self.stretch_size[5] = stretch_keyword(v);
                assign_size(&mut self.max_height, v);
            }

            "padding" => {
                if v == "inherit" {
                    self.padding_inherit = true;
                    return;
                }
                let parsed = Sides::shorthand(v);
                let neg = |l: &Option<Len>| matches!(l, Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if *n < 0.0);
                if neg(&parsed.top)
                    || neg(&parsed.right)
                    || neg(&parsed.bottom)
                    || neg(&parsed.left)
                {
                    return;
                }
                self.padding = parsed;
                // Спор с логическими сторонами решает порядок объявлений.
                self.side_seq.padding = [self.decl_seq; 4];
            }
            "padding-top" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[0] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.top = crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[0] = self.decl_seq;
            }
            "padding-right" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[1] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.right =
                    crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[1] = self.decl_seq;
            }
            "padding-bottom" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[2] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.bottom =
                    crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[2] = self.decl_seq;
            }
            "padding-left" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[3] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.left =
                    crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[3] = self.decl_seq;
            }
            // Физическая запись ГАСИТ логический слот той же стороны: разбор
            // идёт в порядке каскада, и авторский `margin: 0` обязан бить
            // более ранний `margin-block` таблицы агента — а разрешение
            // логических идёт после каскада и иначе перекрывало бы всё.
            // Соответствие сторон берётся горизонтальное: письмо на разборе
            // ещё неизвестно, а гасят почти всегда сбросом всех сторон.
            "margin" => {
                if v == "inherit" {
                    self.margin_inherit = [true; 4];
                    return;
                }
                self.margin = Sides::shorthand(v);
                self.side_seq.margin = [self.decl_seq; 4];
            }
            "margin-top" => {
                if v == "inherit" {
                    self.margin_inherit[0] = true;
                    return;
                }
                self.margin.top = Len::parse(v);
                self.side_seq.margin[0] = self.decl_seq;
            }
            "margin-right" => {
                if v == "inherit" {
                    self.margin_inherit[1] = true;
                    return;
                }
                // Смесь «доля ± точки» доживает индексом: раскладка складывает
                // её сама (css-values-4 §10.9), а вклад решает долю от нуля
                // (css-sizing-3 §5.2.1, `calc-margins-*`). Вертикальные поля
                // — по-прежнему `parse`: смесь там закрыла бы схлопывание.
                self.margin.right =
                    crate::style::values::value::fold_zero_percentage(Len::parse_mixed(v));
                self.side_seq.margin[1] = self.decl_seq;
            }
            "margin-bottom" => {
                if v == "inherit" {
                    self.margin_inherit[2] = true;
                    return;
                }
                self.margin.bottom = Len::parse(v);
                self.side_seq.margin[2] = self.decl_seq;
            }
            "margin-left" => {
                if v == "inherit" {
                    self.margin_inherit[3] = true;
                    return;
                }
                // Смесь «доля ± точки» доживает (см. `margin-right`).
                self.margin.left =
                    crate::style::values::value::fold_zero_percentage(Len::parse_mixed(v));
                self.side_seq.margin[3] = self.decl_seq;
            }

            // --- Логические свойства ---------------------------------------
            // Письмо у нас только слева направо и сверху вниз, поэтому
            // логические оси совпадают с физическими один в один.
            "inline-size" => self.logical().inline_size = Len::parse(v),
            "block-size" => self.logical().block_size = Len::parse(v),
            "min-inline-size" => self.logical().min_inline = Len::parse(v),
            "min-block-size" => self.logical().min_block = Len::parse(v),
            "max-inline-size" => self.logical().max_inline = Len::parse(v),
            "max-block-size" => self.logical().max_block = Len::parse(v),
            "padding-inline" | "padding-block" | "margin-inline" | "margin-block"
            | "inset-inline" | "inset-block" => {
                let (a, b) = axis_pair(v);
                let block = key.ends_with("block");
                let which = key.split('-').next().unwrap_or("").to_string();
                let seq = self.decl_seq;
                let logical = self.logical();
                let target = match which.as_str() {
                    "padding" => &mut logical.padding,
                    "margin" => &mut logical.margin,
                    _ => &mut logical.inset,
                };
                if block {
                    target.block_start = a;
                    target.block_end = b;
                    target.seq[2] = seq;
                    target.seq[3] = seq;
                } else {
                    target.inline_start = a;
                    target.inline_end = b;
                    target.seq[0] = seq;
                    target.seq[1] = seq;
                }
            }
            "padding-inline-start"
            | "padding-inline-end"
            | "padding-block-start"
            | "padding-block-end"
            | "margin-inline-start"
            | "margin-inline-end"
            | "margin-block-start"
            | "margin-block-end"
            | "inset-inline-start"
            | "inset-inline-end"
            | "inset-block-start"
            | "inset-block-end" => {
                let seq = self.decl_seq;
                let parsed = Len::parse(v);
                let logical = self.logical();
                let target = if key.starts_with("padding") {
                    &mut logical.padding
                } else if key.starts_with("margin") {
                    &mut logical.margin
                } else {
                    &mut logical.inset
                };
                let (slot, i) = if key.ends_with("inline-start") {
                    (&mut target.inline_start, 0)
                } else if key.ends_with("inline-end") {
                    (&mut target.inline_end, 1)
                } else if key.ends_with("block-start") {
                    (&mut target.block_start, 2)
                } else {
                    (&mut target.block_end, 3)
                };
                *slot = parsed;
                target.seq[i] = seq;
            }

            // --- Раскладка --------------------------------------------------
            "aspect-ratio" => {
                // `auto <ratio>` (css-sizing-4 §5.1): слово `auto` означает,
                // что у замещаемого ПРИРОДНОЕ соотношение сильнее заявленного,
                // а у остальных коробок действует заявленное. Прошлый заход
                // отбрасывал слово и отдавал отношение всем подряд — срез
                // css-sizing 264 -> 259 (+3/−8, `replaced-element-020/029/030`);
                // теперь слово живёт флагом, и отрисовка замещаемого его
                // учитывает (`image_with::ratio_of`).
                let body = v
                    .split_whitespace()
                    .filter(|t| !t.eq_ignore_ascii_case("auto"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ratio = match body.split_once('/') {
                    Some((a, b)) => match (a.trim().parse::<f32>(), b.trim().parse::<f32>()) {
                        (Ok(a), Ok(b)) if b != 0.0 => Some(a / b),
                        _ => None,
                    },
                    None => body.trim().parse().ok(),
                }
                // Вырожденное отношение (ноль или бесконечность в любой части)
                // ведёт себя как `auto` (css-sizing-4 Overview.bs:533: «If the
                // <ratio> is degenerate, the property instead behaves as
                // auto»; css-values-4 Overview.bs:1665): `0/1` давал
                // соотношение 0, и коробка `width: 100px` получала бесконечную
                // высоту (`zero-or-infinity-001`).
                .filter(|r: &f32| r.is_finite() && *r > 0.0);
                if v.split_whitespace().any(|t| t.eq_ignore_ascii_case("auto")) {
                    self.aspect_ratio_auto = ratio;
                    self.aspect_ratio = None;
                } else {
                    self.aspect_ratio_auto = None;
                    self.aspect_ratio = ratio;
                }
            }
            "frame-sizing" => {
                self.frame_sizing_height =
                    matches!(v.trim().to_ascii_lowercase().as_str(), "content-height");
            }
            _ => *hit = false,
        }
    }
}
