//! Грамматика компонентов animation до построения AnimSpec.
use super::super::time_value;
use crate::animation::easing::Easing;
use crate::style::computed::split_outside_parens;
use crate::style::css::selector_tokens::ident;

pub(super) fn valid(key: &str, value: &str) -> bool {
    crate::style::css::split_args(value).iter().all(|item| {
        let words = split_outside_parens(item);
        if words.is_empty() {
            return false;
        }
        if key != "animation" {
            return words.len() == 1
                && match key {
                    "animation-name" => name(&words[0]),
                    "animation-duration" => time_value(&words[0]).is_some_and(|n| n >= 0.0),
                    "animation-delay" => time_value(&words[0]).is_some(),
                    "animation-iteration-count" => {
                        words[0] == "infinite"
                            || words[0]
                                .parse::<f64>()
                                .is_ok_and(|n| n.is_finite() && n >= 0.0)
                    }
                    "animation-direction" => matches!(
                        words[0].as_str(),
                        "normal" | "reverse" | "alternate" | "alternate-reverse"
                    ),
                    "animation-play-state" => matches!(words[0].as_str(), "running" | "paused"),
                    "animation-fill-mode" => matches!(
                        words[0].as_str(),
                        "none" | "forwards" | "backwards" | "both"
                    ),
                    _ => false,
                };
        }
        let mut used = [false; 6];
        let mut times = 0;
        for word in words {
            if let Some(n) = time_value(&word) {
                if times >= 2 || (times == 0 && n < 0.0) {
                    return false;
                }
                times += 1;
                continue;
            }
            let slot = if Easing::parse(&word).is_some() && !used[0] {
                0
            } else if matches!(
                word.as_str(),
                "normal" | "reverse" | "alternate" | "alternate-reverse"
            ) && !used[1]
            {
                1
            } else if matches!(word.as_str(), "running" | "paused") && !used[2] {
                2
            } else if matches!(word.as_str(), "none" | "forwards" | "backwards" | "both")
                && !used[3]
            {
                3
            } else if (word == "infinite"
                || word.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= 0.0))
                && !used[4]
            {
                4
            } else if name(&word) && !used[5] {
                5
            } else {
                return false;
            };
            used[slot] = true;
        }
        true
    })
}
fn name(v: &str) -> bool {
    (ident(v)
        && !matches!(
            v,
            "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
        ))
        || (v.starts_with(['\"', '\''])
            && v.ends_with(v.chars().next().unwrap_or(' '))
            && v.len() >= 2)
}
