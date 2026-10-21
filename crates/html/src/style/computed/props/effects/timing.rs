//! Computed::apply_effects, продолжение цепочки: animation*, transition*, resize, filter, contain.
//! Ветви в исходном порядке; не совпавший ключ уходит в apply_effects_containment.

use super::*;
use crate::document::state::ValueCx;
mod animation_props;

impl Computed {
    pub(super) fn apply_effects_timing(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            // --- Время --------------------------------------------------------
            "animation"
            | "animation-name"
            | "animation-duration"
            | "animation-iteration-count"
            | "animation-direction"
            | "animation-delay"
            | "animation-play-state" => {
                self.apply_animation(key, v);
            }
            "animation-timing-function" => {
                if let Some(easing) = crate::animation::easing::Easing::parse(v) {
                    self.accept_declaration();
                    self.animation_easing = Some(easing);
                }
            }
            "animation-fill-mode" => self.apply_animation(key, v),
            "transition" | "transition-duration" => {
                // Из записи перехода нужна только длительность: какие свойства
                // меняются, видно по разнице стилей.
                self.transition =
                    parsed_value!(self, transition_value(v, key == "transition-duration"));
            }
            "resize" => {
                self.resize = parsed_value!(
                    self,
                    match v {
                        "both" => Some(Some((true, true))),
                        "horizontal" => Some(Some((true, false))),
                        "vertical" => Some(Some((false, true))),
                        "none" => Some(None),
                        _ => None,
                    }
                );
                if v == "none" {
                    self.accept_declaration();
                }
            }
            "filter" => {
                self.parse_filter(v, vcx);
            }
            "contain" => {
                // Разбор по словам: подстрочный поиск ловил «size» в
                // «inline-size» и не видел paint внутри `content`
                // (css-contain-1 §3.1: strict = size layout paint style,
                // content = layout paint style).
                let words: Vec<_> = v.split_whitespace().collect();
                if words.is_empty()
                    || !words.iter().all(|w| {
                        matches!(
                            *w,
                            "none"
                                | "strict"
                                | "content"
                                | "size"
                                | "inline-size"
                                | "layout"
                                | "paint"
                                | "style"
                        )
                    })
                    || (words.len() > 1
                        && words
                            .iter()
                            .any(|w| matches!(*w, "none" | "strict" | "content")))
                {
                    return;
                }
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
                self.accept_declaration();
                self.contain_size = Some(bits.0);
                self.contain_inline_size = Some(inline_only);
                self.contain_layout = Some(bits.1);
                self.contain_paint = Some(bits.2);
                self.contain_style = Some(bits.3);
            }
            _ => self.apply_effects_containment(key, v, hit, vcx),
        }
    }
}

fn transition_value(v: &str, duration: bool) -> Option<Option<f32>> {
    if v.is_empty() {
        return None;
    }
    if !duration && v == "none" {
        return Some(None);
    }
    let mut first = None;
    for item in crate::style::css::split_args(v) {
        let (mut times, mut property, mut easing, mut behavior) = (0, false, false, false);
        let words = split_outside_parens(item);
        if words.is_empty() {
            return None;
        }
        for token in words {
            if let Some(n) = time_value(&token) {
                if times >= if duration { 1 } else { 2 } || (times == 0 && n < 0.0) {
                    return None;
                }
                if first.is_none() {
                    first = Some(n);
                }
                times += 1;
            } else if duration {
                return None;
            } else if crate::animation::easing::Easing::parse(&token).is_some() {
                if easing {
                    return None;
                }
                easing = true;
            } else if matches!(token.as_str(), "normal" | "allow-discrete") {
                if behavior {
                    return None;
                }
                behavior = true;
            } else if token != "none" && crate::style::css::selector_tokens::ident(&token) {
                if property {
                    return None;
                }
                property = true;
            } else {
                return None;
            }
        }
    }
    Some(Some(first.unwrap_or(0.0)))
}

pub(super) fn time_value(v: &str) -> Option<f32> {
    let n = if let Some(ms) = v.strip_suffix("ms") {
        ms.parse::<f32>().ok()? / 1000.0
    } else {
        v.strip_suffix('s')?.parse::<f32>().ok()?
    };
    n.is_finite().then_some(n)
}
