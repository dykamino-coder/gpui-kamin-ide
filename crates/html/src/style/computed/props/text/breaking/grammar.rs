//! Проверка полной грамматики составных текстовых значений до изменения стиля.

pub(super) fn valid(key: &str, v: &str) -> bool {
    let words: Vec<_> = v.split_whitespace().collect();
    match key {
        "hanging-punctuation" => {
            v == "none"
                || unique(&words, &["first", "last", "force-end", "allow-end"])
                    && !(words.contains(&"force-end") && words.contains(&"allow-end"))
        }
        "text-autospace" => {
            matches!(v, "normal" | "auto" | "no-autospace")
                || unique(
                    &words,
                    &[
                        "ideograph-alpha",
                        "ideograph-numeric",
                        "punctuation",
                        "insert",
                        "replace",
                    ],
                ) && !(words.contains(&"insert") && words.contains(&"replace"))
        }
        "word-space-transform" => {
            v == "none"
                || unique(&words, &["space", "ideographic-space", "auto-phrase"])
                    && !(words.contains(&"space") && words.contains(&"ideographic-space"))
        }
        "hyphenate-character" => v == "auto" || quoted(v),
        "text-fit" => fit(&words),
        "text-wrap-mode" => matches!(v, "wrap" | "nowrap"),
        "text-wrap-style" => matches!(
            v,
            "auto" | "balance" | "stable" | "pretty" | "avoid-orphans"
        ),
        "text-wrap" => {
            let mut mode = false;
            let mut style = false;
            !words.is_empty()
                && words.iter().all(|word| match *word {
                    "wrap" | "nowrap" if !mode => {
                        mode = true;
                        true
                    }
                    "auto" | "balance" | "stable" | "pretty" | "avoid-orphans" if !style => {
                        style = true;
                        true
                    }
                    _ => false,
                })
        }
        _ => true,
    }
}
fn unique(words: &[&str], allowed: &[&str]) -> bool {
    !words.is_empty()
        && words
            .iter()
            .enumerate()
            .all(|(i, w)| allowed.contains(w) && !words[..i].contains(w))
}
fn quoted(v: &str) -> bool {
    v.len() >= 2 && v.starts_with(['"', '\'']) && v.ends_with(v.chars().next().unwrap_or(' '))
}
fn fit(words: &[&str]) -> bool {
    if words == ["none"] {
        return true;
    }
    let mut flags = [false; 4];
    !words.is_empty()
        && words.iter().all(|w| {
            let slot = match *w {
                "grow" => 0,
                "shrink" => 1,
                "consistent" | "per-line" | "per-line-all" => 2,
                n if n
                    .strip_suffix('%')
                    .and_then(|n| n.parse::<f32>().ok())
                    .is_some_and(|n| n.is_finite() && n >= 0.0) =>
                {
                    3
                }
                _ => return false,
            };
            if flags[slot] {
                return false;
            }
            flags[slot] = true;
            true
        })
}
