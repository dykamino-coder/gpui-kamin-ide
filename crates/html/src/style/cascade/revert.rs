//! revert-layer (css-cascade-5): снятие откатанных объявлений с копий правил; распознавание !important.

/// Помечено ли объявление как важное.
/// Снять с копий правил объявления, откатанные `revert-layer`
/// (css-cascade-5 §revert-layer): «as if no rules were specified in the
/// current cascade layer — or between its normal and important levels».
/// Обычный откат в слое L снимает обычные объявления свойства в L; важный —
/// важные в L и в слоях после него (у важных они слабее) и обычные в L и
/// после него. `all: revert-layer` откатывает каждое свойство, чьё
/// объявление в том же слое стоит до него. `None` — откатывать нечего.
pub(super) fn revert_layers(
    matched: &[&crate::style::css::Rule],
) -> Option<Vec<crate::style::css::Rule>> {
    use crate::style::css::DECL_SEP;
    let is_rl = |part: &str| {
        strip_important(part)
            .trim()
            .eq_ignore_ascii_case("revert-layer")
    };
    if !matched
        .iter()
        .any(|r| r.decls.values().any(|v| v.split(DECL_SEP).any(is_rl)))
    {
        return None;
    }
    let mut rules: Vec<crate::style::css::Rule> = matched.iter().map(|r| (*r).clone()).collect();
    // Порядки каскада: обычный — по возрастанию, важный — слой по убыванию.
    let normal_key =
        |r: &crate::style::css::Rule| (r.origin, r.layer.clone(), r.sel.specificity(), r.order);
    let mut keys: Vec<String> = rules
        .iter()
        .flat_map(|r| r.decls.keys().cloned())
        .filter(|k| !k.starts_with("--") && k != crate::style::css::ORDER_KEY)
        .collect();
    keys.sort();
    keys.dedup();
    // Снять части свойства `key` важности `imp` у правил, прошедших фильтр.
    let strip = |rules: &mut Vec<crate::style::css::Rule>,
                 key: &str,
                 imp: bool,
                 keep: &dyn Fn(&crate::style::css::Rule) -> bool| {
        for r in rules.iter_mut().filter(|r| !keep(r)) {
            if let Some(v) = r.decls.get(key) {
                let rest: Vec<&str> = v
                    .split(DECL_SEP)
                    .filter(|p| is_important(p) != imp)
                    .collect();
                if rest.is_empty() {
                    r.decls.remove(key);
                } else {
                    let joined = rest.join(&DECL_SEP.to_string());
                    r.decls.insert(key.to_string(), joined);
                }
            }
        }
    };
    // Победитель свойства: (индекс правила, слой, значение) по порядку каскада.
    let winner = |rules: &Vec<crate::style::css::Rule>,
                  key: &str,
                  imp: bool|
     -> Option<(Vec<u32>, String)> {
        let mut best: Option<(&crate::style::css::Rule, String)> = None;
        for r in rules {
            let Some(v) = r.decls.get(key) else { continue };
            let Some(part) = v.split(DECL_SEP).rfind(|p| is_important(p) == imp) else {
                continue;
            };
            let better = match &best {
                None => true,
                Some((b, _)) if imp => {
                    (
                        std::cmp::Reverse(r.origin),
                        std::cmp::Reverse(&r.layer),
                        r.sel.specificity(),
                        r.order,
                    ) >= (
                        std::cmp::Reverse(b.origin),
                        std::cmp::Reverse(&b.layer),
                        b.sel.specificity(),
                        b.order,
                    )
                }
                Some((b, _)) => normal_key(r) >= normal_key(b),
            };
            if better {
                best = Some((r, part.to_string()));
            }
        }
        best.map(|(r, v)| (r.layer.clone(), v))
    };
    // `all: revert-layer` (обычный): каждое свойство слоя, объявленное в
    // правиле НЕ позже правила с `all`, снимается в этом слое.
    let alls: Vec<(Vec<u32>, (u8, Vec<u32>, (u32, u32, u32), usize))> = rules
        .iter()
        .filter(|r| {
            r.decls.get("all").is_some_and(|v| {
                v.split(DECL_SEP)
                    .rfind(|p| !is_important(p))
                    .is_some_and(is_rl)
            })
        })
        .map(|r| (r.layer.clone(), normal_key(r)))
        .collect();
    for (layer, at) in alls {
        for r in rules
            .iter_mut()
            .filter(|r| r.layer == layer && normal_key(r) <= at)
        {
            let props: Vec<String> = r
                .decls
                .keys()
                .filter(|k| {
                    !k.starts_with("--")
                        && *k != crate::style::css::ORDER_KEY
                        && *k != "direction"
                        && *k != "unicode-bidi"
                })
                .cloned()
                .collect();
            for k in props {
                if let Some(v) = r.decls.get(&k) {
                    let rest: Vec<&str> = v.split(DECL_SEP).filter(|p| is_important(p)).collect();
                    if rest.is_empty() {
                        r.decls.remove(&k);
                    } else {
                        let joined = rest.join(&DECL_SEP.to_string());
                        r.decls.insert(k, joined);
                    }
                }
            }
        }
    }
    for key in keys {
        if key == "direction" || key == "unicode-bidi" || key == "all" {
            continue;
        }
        for _ in 0..16 {
            if let Some((layer, v)) = winner(&rules, &key, true) {
                if is_rl(&v) {
                    let l = layer.clone();
                    strip(&mut rules, &key, true, &|r| r.layer < l);
                    let l = layer.clone();
                    strip(&mut rules, &key, false, &|r| r.layer < l);
                    continue;
                }
                break;
            }
            if let Some((layer, v)) = winner(&rules, &key, false)
                && is_rl(&v)
            {
                let l = layer.clone();
                strip(&mut rules, &key, false, &|r| r.layer != l);
                continue;
            }
            break;
        }
    }
    Some(rules)
}

pub(super) fn is_important(v: &str) -> bool {
    v.to_ascii_lowercase()
        .replace(' ', "")
        .ends_with("!important")
}

/// Значение без пометки важности; пробел перед `!` тоже допустим.
pub(super) fn strip_important(v: &str) -> &str {
    match v.to_ascii_lowercase().rfind('!') {
        Some(at) if v[at..].to_ascii_lowercase().replace(' ', "") == "!important" => v[..at].trim(),
        _ => v,
    }
}
