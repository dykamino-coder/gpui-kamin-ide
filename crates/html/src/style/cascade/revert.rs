//! revert-layer (css-cascade-5): снятие откатанных объявлений с копий правил.

use crate::style::css::Rule;

type NormalKey = (u8, Vec<u32>, (u32, u32, u32), usize);

/// Ключ обычного каскада правила: происхождение, слой, специфичность, порядок.
fn normal_key(r: &Rule) -> NormalKey {
    (r.origin, r.layer.clone(), r.sel.specificity(), r.order)
}

fn is_rl(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("revert-layer")
}

/// Снять с копий правил объявления, откатанные `revert-layer`
/// (css-cascade-5 §revert-layer): «as if no rules were specified in the
/// current cascade layer — or between its normal and important levels».
/// Обычный откат в слое L снимает обычные объявления свойства в L; важный —
/// важные в L и в слоях после него (у важных они слабее) и обычные в L и
/// после него. `all: revert-layer` откатывает каждое свойство, чьё
/// объявление в том же слое стоит до него. `None` — откатывать нечего.
pub(super) fn revert_layers(matched: &[&Rule]) -> Option<Vec<Rule>> {
    if !matched
        .iter()
        .any(|r| r.decls.iter().any(|d| is_rl(&d.value)))
    {
        return None;
    }
    let mut rules: Vec<Rule> = matched.iter().map(|r| (*r).clone()).collect();
    let mut keys: Vec<String> = rules
        .iter()
        .flat_map(|r| r.decls.iter().map(|d| d.name.clone()))
        .filter(|k| !k.starts_with("--"))
        .collect();
    keys.sort();
    keys.dedup();
    // Снять объявления свойства `key` важности `imp` у правил, не прошедших
    // фильтр `keep`.
    let strip = |rules: &mut Vec<Rule>, key: &str, imp: bool, keep: &dyn Fn(&Rule) -> bool| {
        for r in rules.iter_mut().filter(|r| !keep(r)) {
            r.decls.retain(|d| d.name != key || d.important != imp);
        }
    };
    // Победитель свойства: (происхождение, слой, значение) по порядку каскада;
    // внутри правила — последнее объявление нужной важности.
    let winner = |rules: &Vec<Rule>, key: &str, imp: bool| -> Option<(u8, Vec<u32>, String)> {
        let mut best: Option<(&Rule, &str)> = None;
        for r in rules {
            let Some(d) = r
                .decls
                .iter()
                .rev()
                .find(|d| d.name == key && d.important == imp)
            else {
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
                best = Some((r, &d.value));
            }
        }
        best.map(|(r, v)| (r.origin, r.layer.clone(), v.to_string()))
    };
    // `all: revert-layer` (обычный): каждое свойство слоя, объявленное в
    // правиле НЕ позже правила с `all`, снимается в этом слое.
    let alls: Vec<(Vec<u32>, NormalKey)> = rules
        .iter()
        .filter(|r| {
            r.decls
                .iter()
                .rev()
                .find(|d| d.name == "all" && !d.important)
                .is_some_and(|d| is_rl(&d.value))
        })
        .map(|r| (r.layer.clone(), normal_key(r)))
        .collect();
    for (layer, at) in alls {
        for r in rules
            .iter_mut()
            .filter(|r| r.layer == layer && normal_key(r) <= at)
        {
            r.decls.retain(|d| {
                d.important
                    || d.name.starts_with("--")
                    || matches!(d.name.as_str(), "direction" | "unicode-bidi")
            });
        }
    }
    for key in keys {
        if key == "direction" || key == "unicode-bidi" || key == "all" {
            continue;
        }
        for _ in 0..16 {
            if let Some((origin, layer, v)) = winner(&rules, &key, true) {
                if is_rl(&v) {
                    let l = layer.clone();
                    strip(&mut rules, &key, true, &|r| {
                        r.origin != origin || r.layer < l
                    });
                    let l = layer.clone();
                    strip(&mut rules, &key, false, &|r| {
                        r.origin != origin || r.layer < l
                    });
                    continue;
                }
                break;
            }
            if let Some((origin, layer, v)) = winner(&rules, &key, false)
                && is_rl(&v)
            {
                let l = layer.clone();
                strip(&mut rules, &key, false, &|r| {
                    r.origin != origin || r.layer != l
                });
                continue;
            }
            break;
        }
    }
    Some(rules)
}
