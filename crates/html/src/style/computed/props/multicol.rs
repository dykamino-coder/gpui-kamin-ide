//! Computed::apply_one: column*, break-*, *-rule*, margin-trim, page, orphans/widows.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_multicol(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // css-break-3 §4.4: `<integer [1,∞]>`; ноль и отрицательное
            // невалидны — объявление отбрасывается.
            "orphans" | "widows" => {
                if let Ok(n) = v.parse::<u16>()
                    && n >= 1
                {
                    if key == "orphans" {
                        self.orphans = Some(n);
                    } else {
                        self.widows = Some(n);
                    }
                }
            }

            // --- Многоколоночный поток ----------------------------------------
            // Невалидное значение НЕ затирает прежнее (каскад CSS
            // отбрасывает объявление целиком): `column-count: -1` после
            // `column-count: 2` оставляет двойку. Ноль и минус невалидны.
            "column-count" => {
                if v.trim() == "auto" {
                    self.column_count = None;
                } else if let Ok(n) = v.trim().parse::<u16>()
                    && n > 0
                {
                    self.column_count = Some(n);
                }
            }
            "column-width" => {
                if v.trim() == "auto" {
                    self.column_width = None;
                } else if let Some(l) = Len::parse(v.trim()) {
                    // Отрицательная и нулевая ширина колонки невалидны.
                    if !matches!(l, Len::Px(w) if w <= 0.0) {
                        self.column_width = Some(l);
                    }
                }
            }
            // `column-height: auto | <length [0,∞]>` (css-multicol-2 §ch):
            // отрицательная невалидна и не затирает прежнее; ноль — законная
            // высота (`columns: 2 / 0`, `column-height-021…023`).
            "column-height" => {
                if v.trim() == "auto" {
                    self.column_height = None;
                } else if let Some(l) = Len::parse(v.trim())
                    && !matches!(l, Len::Px(h) if h < 0.0)
                {
                    self.column_height = Some(l);
                }
            }
            // `column-wrap: auto | nowrap | wrap` (css-multicol-2 §cwr);
            // `auto` — отсутствие значения, решается в укладке по
            // `column-height`.
            "column-wrap" => {
                self.column_wrap = match v.trim() {
                    "wrap" => Some(true),
                    "nowrap" => Some(false),
                    _ => None,
                };
            }
            // `page: auto | <custom-ident>` (css-page-3 §"Using named pages").
            // Имя регистрозависимо; `auto` — ключевое слово без регистра и
            // хранится отсутствием значения.
            "page" => {
                let t = v.trim();
                self.page = (!t.is_empty() && !t.eq_ignore_ascii_case("auto"))
                    .then(|| t.to_string());
            }
            // `column-fill`: балансировать ли колонки (дефолт balance).
            "column-fill" => self.column_fill_auto = Some(v.trim() == "auto"),
            // `scroll-marker-group: none | [before|after] || [links|tabs]`
            // (css-overflow-5): нужна только сторона, `links`/`tabs` — роль.
            "scroll-marker-group" => {
                let words: Vec<&str> = v.split_whitespace().collect();
                self.scroll_marker_group = if words.contains(&"before") {
                    Some(true)
                } else if words.contains(&"after") {
                    Some(false)
                } else {
                    None
                };
            }
            // `column-span: all` — растяжка на все колонки.
            "column-span" => self.column_span = Some(v.trim() == "all"),
            // `avoid`, `avoid-column`, `avoid-page`, `avoid-region` — все
            // запрещают разрыв ВНУТРИ коробки; `auto` разрешает.
            // `page-break-inside` — устаревшее написание того же (css-break-3
            // §6.4 требует считать их одним свойством).
    /// ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v153, `scout-boxdeco-2026-09.md`):
    /// `box-decoration-break: clone` — украшение на каждом фрагменте
    /// (21 хунк: разбор, `Kid::clone_dec`, ветка в `fill_at`, `frags_of`,
    /// `clone_fragment`). Срез `L-brk` 2874: +2/−2 при ожидании +6…+19 —
    /// `clone-004`, `-012` взяты, но `clone-005.tentative` 0.00 → 99.00 и
    /// `clone-007` 0.00 → 2.08. Ветка `clone` в `fill_at` ломает уже
    /// работавший `slice` у вложенных случаев; нужен отдельный проход
    /// планирования фрагментов, а не правка общей укладки.
            "break-inside" | "page-break-inside" => {
                self.break_inside_avoid = v.trim().starts_with("avoid")
            }
            // Префиксное написание — то же свойство (`border-image-000`,
            // `-webkit-box-decoration-break: clone`).
            "box-decoration-break" | "-webkit-box-decoration-break" => {
                self.bdb_clone = v.trim().eq_ignore_ascii_case("clone")
            }
            "break-before" | "page-break-before" => {
                self.break_before_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
                // css-break-4 §3.1 «avoid break values». Тип фрагментации не
                // различаем — ровно как уже написанный `break_inside_avoid`
                // (`starts_with("avoid")`). Blink здесь строже
                // (`fragmentation_utils.cc:108-121 IsAvoidBreakValue`:
                // `avoid-page` в колонках не действует); упрощение осознанное
                // и безопасное, пока страницы вне зоны патча (§2 отчёта).
                self.break_before_avoid = v.trim().starts_with("avoid");
            }
            "break-after" | "page-break-after" => {
                self.break_after_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
                self.break_after_avoid = v.trim().starts_with("avoid");
            }
            // Лонгхенды линеек промежутков (css-gaps-1 §color-style-width):
            // список через запятую с `repeat()`, `rule-*` ставит обе оси.
            // Первое значение уходит в скаляры — ими живёт многоколонник.
            "column-rule-width" | "row-rule-width" | "rule-width" => {
                if let Some(l) = gap_list(v, gap_width) {
                    if key != "row-rule-width" {
                        self.set_gap_widths(true, &l);
                    }
                    if key != "column-rule-width" {
                        self.set_gap_widths(false, &l);
                    }
                }
            }
            "column-rule-style" | "row-rule-style" | "rule-style" => {
                if let Some(l) = gap_list(v, gap_style) {
                    let double = v.trim().eq_ignore_ascii_case("double");
                    if key != "row-rule-style" {
                        self.set_gap_styles(true, &l);
                        self.column_rule_double = double;
                    }
                    if key != "column-rule-style" {
                        self.set_gap_styles(false, &l);
                        self.row_rule_double = double;
                    }
                }
            }
            "column-rule-color" | "row-rule-color" | "rule-color" => {
                // Ненаследуемое свойство со словом `inherit` (css-cascade-4 §7.2):
                // цвет линейки родителя целиком, а не `currentcolor` своего текста
                // (`multicol-rule-color-inherit-001/002`).
                if v == "inherit" {
                    if key != "row-rule-color" {
                        self.inherit_bits |= inh::COLUMN_RULE_C;
                    }
                    if key != "column-rule-color" {
                        self.inherit_bits |= inh::ROW_RULE_C;
                    }
                    return;
                }
                if let Some(l) = gap_list(v, gap_color) {
                    if key != "row-rule-color" {
                        self.set_gap_colors(true, &l);
                    }
                    if key != "column-rule-color" {
                        self.set_gap_colors(false, &l);
                    }
                }
            }
            // §inset: `[column-|row-]rule-inset[-cap|-junction][-start|-end]`.
            // Без стороны — обе стороны, без вида — и концы, и стыки; два
            // значения — начало и конец. Слоты: [cap-start, cap-end,
            // junction-start, junction-end].
            k if k
                .strip_prefix("column-")
                .or_else(|| k.strip_prefix("row-"))
                .unwrap_or(k)
                .starts_with("rule-inset") =>
            {
                let tail = k
                    .strip_prefix("column-")
                    .or_else(|| k.strip_prefix("row-"))
                    .unwrap_or(k);
                let tail = &tail["rule-inset".len()..];
                let plain = tail.is_empty() || tail == "-start" || tail == "-end";
                let cap = plain || tail.starts_with("-cap");
                let junction = plain || tail.starts_with("-junction");
                let start = !tail.ends_with("-end");
                let end = !tail.ends_with("-start");
                let toks = split_outside_parens(v);
                let (vs, ve) = match toks.as_slice() {
                    [a] => (gap_inset(a), gap_inset(a)),
                    [a, b] => (gap_inset(a), gap_inset(b)),
                    _ => (None, None),
                };
                if let (Some(vs), Some(ve)) = (vs, ve) {
                    let slots = [cap && start, cap && end, junction && start, junction && end];
                    for column in [true, false] {
                        if (column && k.starts_with("row-")) || (!column && k.starts_with("column-")) {
                            continue;
                        }
                        let arr = if column {
                            &mut self.column_rule_inset
                        } else {
                            &mut self.row_rule_inset
                        };
                        let mut cur = arr.unwrap_or([GapInset::Len(Len::Px(0.0)); 4]);
                        for (i, on) in slots.iter().enumerate() {
                            if *on {
                                cur[i] = if i % 2 == 0 { vs } else { ve };
                            }
                        }
                        *arr = Some(cur);
                    }
                }
            }
            // §visibility-items: 0 normal, 1 all, 2 around, 3 between.
            "rule-visibility-items" | "column-rule-visibility-items" | "row-rule-visibility-items" => {
                let code = match v.trim() {
                    "normal" => Some(0u8),
                    "all" => Some(1),
                    "around" => Some(2),
                    "between" => Some(3),
                    _ => None,
                };
                if let Some(code) = code {
                    if key != "row-rule-visibility-items" {
                        self.column_rule_visibility = Some(code);
                    }
                    if key != "column-rule-visibility-items" {
                        self.row_rule_visibility = Some(code);
                    }
                }
            }
            // §overlap: порядок краски пересекающихся линеек.
            "rule-overlap" => match v.trim() {
                "row-over-column" => self.rule_column_over_row = Some(false),
                "column-over-row" => self.rule_column_over_row = Some(true),
                _ => {}
            },
            // §break: `none` 0, `normal` 1, `intersection` 2.
            "column-rule-break" | "row-rule-break" | "rule-break" => {
                let code = match v.trim() {
                    "none" => Some(0u8),
                    "normal" => Some(1),
                    "intersection" => Some(2),
                    _ => None,
                };
                if let Some(code) = code {
                    if key != "row-rule-break" {
                        self.column_rule_break = Some(code);
                    }
                    if key != "column-rule-break" {
                        self.row_rule_break = Some(code);
                    }
                }
            }
            // §margin-trim: `none | block | [ block-start || block-end ]`.
            "margin-trim" => {
                let mut bits = 0u8;
                let mut ok = true;
                for token in v.split_whitespace() {
                    match token {
                        "none" => {}
                        "block" => bits |= 3,
                        "block-start" => bits |= 1,
                        "block-end" => bits |= 2,
                        // Строчные края — текущая редакция спеки: их
                        // исполняют гибкий контейнер и сетка («Flex
                        // Containers», «Grid Containers»); блочный контейнер
                        // их не видит (`block-container-inline-001`).
                        "inline" => bits |= 12,
                        "inline-start" => bits |= 4,
                        "inline-end" => bits |= 8,
                        _ => ok = false,
                    }
                }
                if ok {
                    self.margin_trim = bits;
                }
            }
            // Сокращения линеек (css-gaps-1 §rule-shorthands): список
            // `<gap-rule>` через запятую с `repeat()`; `rule` — обе оси.
            // Незнакомый токен делает недействительным ВСЁ объявление
            // (CSS 2.1 §4.1.7), а токены с пробелами внутри скобок
            // (`rgba(0, 0, 255, 0.5)`) больше не рвутся.
            "column-rule" | "row-rule" | "rule" => self.gap_rule_shorthand(key, v),
            "columns" => {
                // `columns: [<ширина> || <число>] [/ <column-height>]?`
                // (css-multicol-2 §columns): ширина и число в любом порядке;
                // `auto` оставляет сторону нерешённой (не затирать уже
                // разобранную ширину). Короткая форма сбрасывает
                // `column-height` и `column-wrap` в начальные
                // (`columns-shorthand-reset-wrap`, `columns: 2 / 0`).
                let (head, tail) = v.split_once('/').map_or((v, None), |(a, b)| (a, Some(b)));
                // Грамматика css-multicol-1 §columns: «<<'column-width'>> ||
                // <<'column-count'>>» — не больше двух слов, каждое не больше раза.
                // Иное объявление НЕВАЛИДНО и отбрасывается ЦЕЛИКОМ (css-syntax-3
                // §consume-declaration), ничего не сбрасывая. Прежде слова брались
                // поштучно: `columns: 8 auto 6em` давал `column-count: 8` и
                // `column-width: 6em`, и с пакетом B (`column_width` из `merged`,
                // в точках) колонок стало min(8, ⌊240/120⌋) = 2 вместо 4 —
                // `multicol-columns-invalid-002` 0.00 (v39) → 0.67 (v40).
                let words: Vec<&str> = head.split_whitespace().collect();
                if words.is_empty() || words.len() > 2 {
                    return;
                }
                let mut count: Option<u16> = None;
                let mut width: Option<Len> = None;
                for token in &words {
                    if token.eq_ignore_ascii_case("auto") {
                        continue;
                    }
                    match token.parse::<u16>() {
                        Ok(n) if n > 0 && count.is_none() => count = Some(n),
                        // `0` — как прежде: без действия (как длина `column-width: 0`
                        // у нас отвергается и в полной форме).
                        Ok(0) => {}
                        Ok(_) => return,
                        Err(_) => match Len::parse(token) {
                            Some(l)
                                if width.is_none()
                                    && !matches!(
                                        l,
                                        Len::Auto
                                            | Len::Pct(_)
                                            | Len::MinContent
                                            | Len::MaxContent
                                            | Len::FitContent
                                    )
                                    && !matches!(l, Len::Px(w) | Len::Em(w) if w < 0.0) =>
                            {
                                width = Some(l)
                            }
                            _ => return,
                        },
                    }
                }
                self.column_height = None;
                self.column_wrap = None;
                if let Some(t) = tail {
                    self.apply_one("column-height", t.trim());
                }
                if let Some(n) = count {
                    self.column_count = Some(n);
                }
                if let Some(l) = width {
                    self.column_width = Some(l);
                }
            }
            _ => *hit = false,
        }
    }
}

impl Computed {
    /// Ширины линеек одной оси: первое значение — в скаляр (многоколонник),
    /// список — только когда значений больше одного или есть авто-повтор.
    pub(crate) fn set_gap_widths(&mut self, column: bool, l: &GapList<Len>) {
        let (scalar, list) = if column {
            (&mut self.column_rule_width, &mut self.column_rule_widths)
        } else {
            (&mut self.row_rule_width, &mut self.row_rule_widths)
        };
        *scalar = l.first().or(*scalar);
        *list = l.is_plural().then(|| l.clone());
    }

    pub(crate) fn set_gap_styles(&mut self, column: bool, l: &GapList<bool>) {
        let (scalar, list) = if column {
            (&mut self.column_rule_visible, &mut self.column_rule_styles)
        } else {
            (&mut self.row_rule_visible, &mut self.row_rule_styles)
        };
        *scalar = l.first().or(*scalar);
        *list = l.is_plural().then(|| l.clone());
    }

    pub(crate) fn set_gap_colors(&mut self, column: bool, l: &GapList<Option<Color>>) {
        let (scalar, list) = if column {
            (&mut self.column_rule_color, &mut self.column_rule_colors)
        } else {
            (&mut self.row_rule_color, &mut self.row_rule_colors)
        };
        *scalar = l.first().flatten();
        *list = l.is_plural().then(|| l.clone());
    }

    /// `column-rule`/`row-rule`/`rule` (css-gaps-1 §rule-shorthands): каждая
    /// часть сокращения ставит СВОЙ список; неназванные части сбрасываются в
    /// начальные (`medium`, `none`, `currentcolor`), как у любого сокращения.
    pub(crate) fn gap_rule_shorthand(&mut self, key: &str, v: &str) {
        let Some(list) = gap_list(v, gap_rule) else { return };
        let widths = list.map(|r| r.0.unwrap_or(Len::Px(3.0)));
        let styles = list.map(|r| r.1.unwrap_or(false));
        let colors = list.map(|r| r.2.flatten());
        let double = !v.contains(',')
            && split_outside_parens(v).iter().any(|t| t.trim().eq_ignore_ascii_case("double"));
        for column in [true, false] {
            if (column && key == "row-rule") || (!column && key == "column-rule") {
                continue;
            }
            if column {
                self.column_rule_double = double;
            } else {
                self.row_rule_double = double;
            }
            self.set_gap_widths(column, &widths);
            self.set_gap_styles(column, &styles);
            self.set_gap_colors(column, &colors);
        }
    }
}

/// Ширина линейки промежутка: ключевые слова css-gaps-1 §width те же, что у
/// рамок; отрицательная недействительна.
fn gap_width(t: &str) -> Option<Len> {
    match t.trim() {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        t => Len::parse(t).filter(|l| {
            matches!(l, Len::Px(w) if *w >= 0.0) || matches!(l, Len::Em(k) if *k >= 0.0)
        }),
    }
}

/// Стиль линейки: `none`/`hidden` — не рисовать, прочие — рисовать (все
/// стили пока красятся сплошной полосой).
fn gap_style(t: &str) -> Option<bool> {
    match t.trim() {
        "none" | "hidden" => Some(false),
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset" => {
            Some(true)
        }
        _ => None,
    }
}

/// Цвет линейки; `currentcolor` — `None` (цвет текста контейнера).
fn gap_color(t: &str) -> Option<Option<Color>> {
    let t = t.trim();
    if t.eq_ignore_ascii_case("currentcolor") {
        return Some(None);
    }
    Color::parse(t).map(Some)
}

/// Втяжка конца (css-gaps-1 §inset): длина/доля или `overlap-join`.
fn gap_inset(t: &str) -> Option<GapInset> {
    let t = t.trim();
    if t == "overlap-join" {
        return Some(GapInset::OverlapJoin);
    }
    if t == "0" {
        return Some(GapInset::Len(Len::Px(0.0)));
    }
    Len::parse(t)
        .filter(|l| matches!(l, Len::Px(_) | Len::Pct(_) | Len::Em(_)))
        .map(GapInset::Len)
}

/// `<gap-rule> = <line-width> || <line-style> || <color>`: любой порядок,
/// каждая часть не более одного раза; лишний токен — недействительно.
fn gap_rule(entry: &str) -> Option<(Option<Len>, Option<bool>, Option<Option<Color>>)> {
    let (mut w, mut s, mut c) = (None, None, None);
    for token in split_outside_parens(entry) {
        if s.is_none() && let Some(v) = gap_style(&token) {
            s = Some(v);
        } else if w.is_none() && let Some(v) = gap_width(&token) {
            w = Some(v);
        } else if c.is_none() && let Some(v) = gap_color(&token) {
            c = Some(v);
        } else {
            return None;
        }
    }
    Some((w, s, c))
}

/// Список css-gaps-1 §lists: значения через запятую вне скобок; `repeat(N, …)`
/// раскрывается на месте, `repeat(auto, …)` допустим один раз и делит список
/// на ведущие и хвостовые. Любой неразобранный элемент — весь список
/// недействителен.
fn gap_list<T: Copy>(v: &str, one: impl Fn(&str) -> Option<T>) -> Option<GapList<T>> {
    let mut out = GapList { lead: vec![], auto: vec![], tail: vec![] };
    let mut seen_auto = false;
    for entry in crate::style::css::split_args(v) {
        let entry = entry.trim();
        let Some(inner) = entry
            .strip_prefix("repeat(")
            .and_then(|r| r.strip_suffix(')'))
        else {
            let val = one(entry)?;
            if seen_auto {
                out.tail.push(val);
            } else {
                out.lead.push(val);
            }
            continue;
        };
        let args = crate::style::css::split_args(inner);
        let (count, vals) = args.split_first()?;
        let vals: Vec<T> = vals.iter().map(|s| one(s.trim())).collect::<Option<Vec<T>>>()?;
        if vals.is_empty() {
            return None;
        }
        if count.trim() == "auto" {
            if seen_auto {
                return None;
            }
            seen_auto = true;
            out.auto = vals;
        } else {
            let n: usize = count.trim().parse().ok().filter(|n| *n >= 1)?;
            let dst = if seen_auto { &mut out.tail } else { &mut out.lead };
            for _ in 0..n {
                dst.extend_from_slice(&vals);
            }
        }
    }
    (out.lead.len() + out.auto.len() + out.tail.len() > 0).then_some(out)
}
