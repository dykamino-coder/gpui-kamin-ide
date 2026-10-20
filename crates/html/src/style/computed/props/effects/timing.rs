//! Computed::apply_effects, продолжение цепочки: animation*, transition*, resize, filter, contain. Ветви в исходном порядке; не совпавший ключ уходит в apply_effects_containment.

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
                self.transition = record_parsed!(
                    self,
                    v.split_whitespace().find_map(|t| {
                        t.strip_suffix("ms")
                            .and_then(|n| n.parse::<f32>().ok())
                            .map(|ms| ms / 1000.0)
                            .or_else(|| t.strip_suffix('s').and_then(|n| n.parse::<f32>().ok()))
                    })
                );
                if v == "none" {
                    self.accept_declaration();
                }
            }
            "resize" => {
                self.resize = record_parsed!(
                    self,
                    match v {
                        "both" => Some((true, true)),
                        "horizontal" => Some((true, false)),
                        "vertical" => Some((false, true)),
                        _ => None,
                    }
                );
                if v == "none" {
                    self.accept_declaration();
                }
            }
            "filter" => {
                self.parse_filter(v, vcx);
                self.accept_declaration();
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
