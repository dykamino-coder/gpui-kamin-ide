//! Computed::apply_one: font*, font-variant/feature/synthesis/size-adjust/stretch/kerning.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};
mod metrics;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_font(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "font-size" => {
                // Отрицательный кегль и неразборная запись делают объявление
                // НЕВАЛИДНЫМ (§4.2, §15.7): прежнее значение остаётся, а не
                // стирается в `None` (`c526-font-sz-003`: `-0.5in`).
                let neg = |l: &Len| {
                    matches!(
                        l,
                        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                            if *v < 0.0
                    )
                };
                // Абсолютные и относительные СЛОВА кегля (CSS 2.1 §15.7,
                // css-fonts-4 §absolute-size). Раньше слово давало `None`, а
                // `None` в модели значит «не задано», то есть наследование.
                // Из-за этого `font-size: initial` (через `initial_value` —
                // `medium`) не сбрасывал кегль корня, и абзац
                // `percentage-rem-low` набирался четырьмя точками вместо
                // шестнадцати. Таблица — та же, что у Chrome при базовом 16.
                let lower = v.to_ascii_lowercase();
                let word = match lower.as_str() {
                    "xx-small" => Some(Len::Px(9.0)),
                    "x-small" => Some(Len::Px(10.0)),
                    "small" => Some(Len::Px(13.0)),
                    "medium" => Some(Len::Px(16.0)),
                    "large" => Some(Len::Px(18.0)),
                    "x-large" => Some(Len::Px(24.0)),
                    "xx-large" => Some(Len::Px(32.0)),
                    "xxx-large" => Some(Len::Px(48.0)),
                    // Относительные — доли РОДИТЕЛЬСКОГО кегля: их сводит к
                    // точкам `resolve_em`, как обычный `em`.
                    "smaller" => Some(Len::Em(5.0 / 6.0)),
                    "larger" => Some(Len::Em(1.2)),
                    _ => None,
                };
                if let Some(l) = word {
                    self.font_size = Some(l);
                    // `larger`/`smaller` шагают по ТАБЛИЦЕ (§15.7: «if the
                    // parent element has font size 'medium', then 'larger'
                    // will make … 'large'»), а не множат: шесть шагов вверх от
                    // `xx-small` (9 → 26.9) не сходились с шестью вниз от
                    // `xx-large` (32 → 10.7) (`font-size-121`). Кегль родителя
                    // известен в `inline::inherit`; `Len::Em` — запас вне таблицы.
                    self.font_size_step = match lower.as_str() {
                        "larger" => 1,
                        "smaller" => -1,
                        _ => 0,
                    };
                    return;
                }
                self.font_size = match Len::parse(v) {
                    Some(l) if neg(&l) => self.font_size,
                    other => {
                        self.font_size_step = 0;
                        other
                    }
                };
            }
            "font-weight" => font_weight::apply(self, v),
            "font-style" => {
                self.italic = Some(v == "italic" || v == "oblique");
                // `oblique <angle>` — тоже наклон, а не курсив (css-fonts-4
                // §font-style-prop): подбор лица различает их.
                self.oblique = Some(v.starts_with("oblique"));
            }
            "font-family" => font_family::apply(self, v),

            // --- Текст -------------------------------------------------------
            // `font: [начертание] [вес] размер[/интерлиньяж] семейство`.
            "font" => font_shorthand::apply(self, v),
            "font-synthesis"
            | "font-synthesis-weight"
            | "font-synthesis-style"
            | "font-synthesis-small-caps" => {
                // css-fonts-4 §6.5: `auto` разрешает подмену, `none`
                // запрещает; у сокращения перечислены разрешённые части.
                let allow = |what: &str| match key {
                    "font-synthesis" => v.contains(what),
                    _ => v.trim() != "none",
                };
                match key {
                    "font-synthesis-weight" => self.font_synth.0 = Some(allow("weight")),
                    "font-synthesis-style" => self.font_synth.1 = Some(allow("style")),
                    "font-synthesis-small-caps" => self.font_synth.2 = Some(allow("small-caps")),
                    _ => {
                        self.font_synth = (
                            Some(allow("weight")),
                            Some(allow("style")),
                            Some(allow("small-caps")),
                        )
                    }
                }
                // Подмена ВЕСА и НАКЛОНА выражается своими тегами возможностей:
                // подбор грани в gpui читает их и отвергает поддельную грань
                // (`nsyw`/`nsys` — свои, не OpenType). Малые прописные мы не
                // синтезируем вовсе, поэтому у них тега нет.
                for (tag, on) in [("nsyw", self.font_synth.0), ("nsys", self.font_synth.1)] {
                    self.font_features.retain(|(t, _)| t != tag);
                    if on == Some(false) {
                        self.font_features.push((tag.to_string(), 1));
                    }
                }
            }
            "font-kerning" => {
                self.set_font_kerning(v);
            }
            "font-feature-settings" => {
                // Низкоуровневые теги через запятую: `"tnum" 1, "liga" off`.
                if v == "inherit" {
                    self.font_settings = None;
                    return;
                }
                // Негодный список роняет объявление целиком (§4.2).
                if let Some(list) = feature_list(v) {
                    self.font_settings = Some(list);
                }
            }
            "font-variant"
            | "font-variant-caps"
            | "font-variant-numeric"
            | "font-variant-ligatures"
            | "font-variant-east-asian"
            | "font-variant-position"
            | "font-variant-alternates" => {
                if matches!(key, "font-variant" | "font-variant-alternates") {
                    if v == "inherit" {
                        self.font_alternates = None;
                        return;
                    }
                    let Some(alternates) =
                        crate::text::fonts::alternates::parse(v, key == "font-variant")
                    else {
                        return;
                    };
                    self.font_alternates = Some(alternates);
                }
                // Значения разделяются ПРОБЕЛОМ (css-fonts-4 §6); каждое
                // свойство сперва чистит СВОЮ подгруппу тегов — повтор и
                // `normal` переопределяют, а не копятся при наследовании.
                const CAPS: &[&str] = &["smcp", "c2sc", "pcap", "c2pc", "unic", "titl"];
                const NUMERIC: &[&str] = &[
                    "lnum", "onum", "pnum", "tnum", "frac", "afrc", "ordn", "zero",
                ];
                const LIGA: &[&str] = &["liga", "clig", "dlig", "hlig", "calt"];
                const EAST: &[&str] = &[
                    "jp78", "jp83", "jp90", "jp04", "smpl", "trad", "fwid", "pwid", "ruby",
                ];
                const POS: &[&str] = &["subs", "sups"];
                const ALT: &[&str] = &["hist", "salt", "swsh", "ornm", "nalt"];
                let alt_tag = |t: &str| {
                    (t.len() == 4 && (t.starts_with("ss") || t.starts_with("cv")))
                        && t[2..].bytes().all(|b| b.is_ascii_digit())
                };
                let groups: &[&[&str]] = match key {
                    "font-variant-caps" => &[CAPS],
                    "font-variant-numeric" => &[NUMERIC],
                    "font-variant-ligatures" => &[LIGA],
                    "font-variant-east-asian" => &[EAST],
                    "font-variant-position" => &[POS],
                    "font-variant-alternates" => &[ALT],
                    _ => &[CAPS, NUMERIC, LIGA, EAST, POS, ALT],
                };
                self.font_features.retain(|(t, _)| {
                    !groups.iter().any(|g| g.contains(&t.as_str()))
                        && !(matches!(key, "font-variant" | "font-variant-alternates")
                            && alt_tag(t))
                });
                for token in v.split_whitespace() {
                    let push: &[(&str, u32)] = match token {
                        "small-caps" => &[("smcp", 1)],
                        "all-small-caps" => &[("smcp", 1), ("c2sc", 1)],
                        "petite-caps" => &[("pcap", 1)],
                        "all-petite-caps" => &[("pcap", 1), ("c2pc", 1)],
                        "unicase" => &[("unic", 1)],
                        "titling-caps" => &[("titl", 1)],
                        "lining-nums" => &[("lnum", 1)],
                        "oldstyle-nums" => &[("onum", 1)],
                        "proportional-nums" => &[("pnum", 1)],
                        "tabular-nums" => &[("tnum", 1)],
                        "diagonal-fractions" => &[("frac", 1)],
                        "stacked-fractions" => &[("afrc", 1)],
                        "ordinal" => &[("ordn", 1)],
                        "slashed-zero" => &[("zero", 1)],
                        "common-ligatures" => &[("liga", 1), ("clig", 1)],
                        "no-common-ligatures" => &[("liga", 0), ("clig", 0)],
                        "discretionary-ligatures" => &[("dlig", 1)],
                        "no-discretionary-ligatures" => &[("dlig", 0)],
                        "historical-ligatures" => &[("hlig", 1)],
                        "no-historical-ligatures" => &[("hlig", 0)],
                        "contextual" => &[("calt", 1)],
                        "no-contextual" => &[("calt", 0)],
                        "jis78" => &[("jp78", 1)],
                        "jis83" => &[("jp83", 1)],
                        "jis90" => &[("jp90", 1)],
                        "jis04" => &[("jp04", 1)],
                        "simplified" => &[("smpl", 1)],
                        "traditional" => &[("trad", 1)],
                        "full-width" => &[("fwid", 1)],
                        "proportional-width" => &[("pwid", 1)],
                        "ruby" => &[("ruby", 1)],
                        "sub" => &[("subs", 1)],
                        "super" => &[("sups", 1)],
                        "historical-forms" => &[],
                        // `none` выключает лигатуры по умолчанию.
                        "none" => &[("liga", 0), ("clig", 0), ("calt", 0)],
                        "normal" => &[],
                        _ => &[],
                    };
                    for (t, on) in push {
                        self.font_features.push(((*t).into(), *on));
                    }
                }
            }
            _ => self.apply_font_metrics(key, val, v, hit),
        }
    }
}
