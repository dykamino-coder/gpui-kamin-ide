//! Запись числа пользовательским стилем счётчика: начальные системы, графемы, маркеры.

use super::*;
use crate::document::state::ValueCx;

fn graphemes(s: &str) -> usize {
    unicode_segmentation::UnicodeSegmentation::graphemes(s, true).count()
}

/// §counter-style-generate: fallback follows only the edges actually selected by value.
fn generate(
    rules: &HashMap<String, Raw>,
    v: i64,
    name: &str,
    context: Context,
    admission: &crate::budget::admission::Admission,
) -> Result<String, crate::budget::admission::Error> {
    use crate::budget::admission::Buffer;
    let mut visited = Buffer::default();
    let mut seen = std::collections::HashSet::new();
    let mut current = name.to_string();
    loop {
        admission.work(1)?;
        if !seen.insert(current.clone()) {
            return Ok(v.to_string());
        }
        visited.push(current.clone(), admission)?;
        let Some(r) = resolve_counter_style(rules, &current, admission)?
            .or_else(|| builtin(&current).map(|_| super::resolve::builtin_resolved(&current)))
        else {
            return Ok(v.to_string());
        };
        if in_range(&r, v) {
            let neg = v < 0 && uses_negative(&r.algo);
            if let Some(mut body) = super::initial::initial(
                &r.algo,
                if neg { -i128::from(v) } else { i128::from(v) },
                context,
                admission,
            )? {
                if let Some((width, sym)) = &r.pad {
                    let len = graphemes(&body)
                        + if neg {
                            graphemes(&r.negative.0) + graphemes(&r.negative.1)
                        } else {
                            0
                        };
                    if len < *width {
                        body = format!(
                            "{}{body}",
                            super::initial::repeat(sym, width - len, admission)?
                        );
                    }
                }
                if neg {
                    body = format!("{}{body}{}", r.negative.0, r.negative.1);
                }
                return Ok(body);
            }
        }
        current = r.fallback;
    }
}

fn finish<T>(result: Result<T, crate::budget::admission::Error>, vcx: ValueCx<'_>) -> Option<T> {
    result
        .map_err(|error| {
            let failure = crate::dom::outcome::DocumentFailure::from(error);
            vcx.doc.revision_budget().reject(failure);
            vcx.doc
                .record_observed(crate::diagnostics::Diagnostic::from(&failure));
        })
        .ok()
}

/// Представление значения стилем, заданным правилом документа; `None` —
/// такого правила нет (решает предопределённый путь).
pub fn custom_repr(v: i64, name: &str, context: Context, vcx: ValueCx<'_>) -> Option<String> {
    let guard = vcx.doc.counter_styles.borrow();
    let rules = &*guard;
    (rules.contains_key(name) || anonymous(name).is_some()).then(|| {
        let admission = crate::budget::admission::Admission::new(vcx.doc.revision_budget());
        finish(generate(rules, v, name, context, &admission), vcx).unwrap_or_default()
    })
}

/// Анонимный стиль `symbols()` (css-counter-styles-3 §symbols-function):
/// система (по умолчанию `symbolic`, `additive` и `extends` запрещены),
/// символы — только строки; суффикс — пробел, прочее — по умолчанию.
pub(super) fn anonymous(name: &str) -> Option<Raw> {
    let head = name.get(..8)?;
    if !head.eq_ignore_ascii_case("symbols(") {
        return None;
    }
    let inner = name[8..].trim_end().strip_suffix(')')?;
    let toks = tokenize(inner);
    let (system, rest) = match toks.first()? {
        Tok::Ident(k) => {
            let sys = match k.to_ascii_lowercase().as_str() {
                "cyclic" => System::Cyclic,
                "numeric" => System::Numeric,
                "alphabetic" => System::Alphabetic,
                "symbolic" => System::Symbolic,
                "fixed" => System::Fixed(1),
                _ => return None,
            };
            (sys, &toks[1..])
        }
        _ => (System::Symbolic, &toks[..]),
    };
    let symbols: Vec<String> = rest
        .iter()
        .map(|t| match t {
            Tok::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let need = if matches!(system, System::Alphabetic | System::Numeric) {
        2
    } else {
        1
    };
    if symbols.len() < need {
        return None;
    }
    Some(Raw {
        system: Some(system),
        symbols: Some(symbols),
        suffix: Some(" ".to_string()),
        ..Raw::default()
    })
}

/// Действительна ли запись `symbols(…)` — для разбора `list-style-type`.
pub fn valid_symbols_fn(text: &str) -> bool {
    anonymous(text).is_some()
}

/// Строка маркера: `prefix`, представление, `suffix` (§counter-style-prefix).
/// Префикс и суффикс берутся у самого стиля, даже если значение ушло в
/// резервный.
pub fn custom_marker(v: i64, name: &str, context: Context, vcx: ValueCx<'_>) -> Option<String> {
    let guard = vcx.doc.counter_styles.borrow();
    let rules = &*guard;
    let admission = crate::budget::admission::Admission::new(vcx.doc.revision_budget());
    let r = finish(resolve_counter_style(rules, name, &admission), vcx)??;
    let body = finish(generate(rules, v, name, context, &admission), vcx)?;
    Some(format!("{}{body}{}", r.prefix, r.suffix))
}

#[cfg(test)]
#[path = "generate_tests.rs"]
mod tests;
