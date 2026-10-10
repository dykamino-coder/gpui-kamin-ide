//! Списки линеек промежутков (css-gaps-1): разбор значений по осям (gap_list/gap_rule/gap_*) и запись их в Computed (set_gap_*, gap_rule_shorthand).

use super::*;

/// Ширина линейки промежутка: ключевые слова css-gaps-1 §width те же, что у
/// рамок; отрицательная недействительна.
pub(super) fn gap_width(t: &str) -> Option<Len> {
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
pub(super) fn gap_style(t: &str) -> Option<bool> {
    match t.trim() {
        "none" | "hidden" => Some(false),
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset" => {
            Some(true)
        }
        _ => None,
    }
}

/// Цвет линейки; `currentcolor` — `None` (цвет текста контейнера).
pub(super) fn gap_color(t: &str) -> Option<Option<Color>> {
    let t = t.trim();
    if t.eq_ignore_ascii_case("currentcolor") {
        return Some(None);
    }
    Color::parse(t).map(Some)
}

/// Втяжка конца (css-gaps-1 §inset): длина/доля или `overlap-join`.
pub(super) fn gap_inset(t: &str) -> Option<GapInset> {
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
pub(super) fn gap_rule(entry: &str) -> Option<(Option<Len>, Option<bool>, Option<Option<Color>>)> {
    let (mut w, mut s, mut c) = (None, None, None);
    for token in split_outside_parens(entry) {
        if s.is_none()
            && let Some(v) = gap_style(&token)
        {
            s = Some(v);
        } else if w.is_none()
            && let Some(v) = gap_width(&token)
        {
            w = Some(v);
        } else if c.is_none()
            && let Some(v) = gap_color(&token)
        {
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
pub(super) fn gap_list<T: Copy>(v: &str, one: impl Fn(&str) -> Option<T>) -> Option<GapList<T>> {
    let mut out = GapList {
        lead: vec![],
        auto: vec![],
        tail: vec![],
    };
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
        let vals: Vec<T> = vals
            .iter()
            .map(|s| one(s.trim()))
            .collect::<Option<Vec<T>>>()?;
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
            let dst = if seen_auto {
                &mut out.tail
            } else {
                &mut out.lead
            };
            for _ in 0..n {
                dst.extend_from_slice(&vals);
            }
        }
    }
    (out.lead.len() + out.auto.len() + out.tail.len() > 0).then_some(out)
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
        let Some(list) = gap_list(v, gap_rule) else {
            return;
        };
        let widths = list.map(|r| r.0.unwrap_or(Len::Px(3.0)));
        let styles = list.map(|r| r.1.unwrap_or(false));
        let colors = list.map(|r| r.2.flatten());
        let double = !v.contains(',')
            && split_outside_parens(v)
                .iter()
                .any(|t| t.trim().eq_ignore_ascii_case("double"));
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
