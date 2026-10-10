//! Запросы фона к Computed: цвет под background-clip: text и разложение списка слоёв фона.

use super::*;

impl Computed {
    /// Слои фона СВЕРХУ ВНИЗ, когда их больше одного: каждый — копия стиля с
    /// одним слоем (картинка, размер, положение, повтор, область) и без
    /// цвета фона — цвет лежит под всеми слоями и красится коробкой
    /// (css-backgrounds-3 §3.1, §2.1: значения списков, которых меньше
    /// слоёв, повторяются по кругу). Градиент слоя уходит в растровую плитку
    /// (`bg_image` с сырой записью), чтобы все слои шли одним путём и в
    /// своём порядке. `None` — слой один.
    /// `background-clip` of the background COLOR: css-backgrounds-3 §3.2,
    /// «the background color is clipped according to the background-clip
    /// value associated with the bottom-most background image layer». The
    /// number of layers comes from `background-image` (§2.1); a shorter
    /// `background-clip` list repeats, a longer one is truncated
    /// (`background-color-clip`: two `none` layers, clip list
    /// `border-box, content-box, border-box` → `content-box`).
    pub(crate) fn color_clip(&self) -> Option<BgClip> {
        let Some((_, clips)) = self.bg_lists.iter().find(|(k, _)| k == "background-clip") else {
            return self.bg_clip;
        };
        let Some((_, images)) = self.bg_lists.iter().find(|(k, _)| k == "background-image") else {
            return self.bg_clip;
        };
        let n = background_layers(images).len();
        let clips = background_layers(clips);
        if n < 2 || clips.is_empty() {
            return self.bg_clip;
        }
        let mut probe = Computed::default();
        probe.apply_one("background-clip", clips[(n - 1) % clips.len()]);
        probe.bg_clip
    }

    pub(crate) fn bg_layers(&self) -> Option<Vec<Computed>> {
        let short = self
            .bg_lists
            .iter()
            .find(|(k, _)| k == "background")
            .map(|(_, v)| v.clone());
        let image = self
            .bg_lists
            .iter()
            .find(|(k, _)| k == "background-image")
            .map(|(_, v)| v.clone());
        let images: Vec<String> = match (&image, &short) {
            (Some(v), _) | (None, Some(v)) => background_layers(v)
                .into_iter()
                .map(str::to_string)
                .collect(),
            _ => return None,
        };
        if images.len() < 2 {
            return None;
        }
        // Длины слоёв в единицах шрифта (`1ch 0 / 4ch 1ch`): слой разбирается
        // заново из сырой записи уже ПОСЛЕ `resolve_em`, и `ch` в положении и
        // размере оставался нерешённым — слой выходил нулевым
        // (`hanging-whitespace-001..004`). Решаем по своему кеглю.
        let font_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::text::metrics::mono_family_for(self.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        let (ch, ex) = crate::text::metrics::ch_ex_px(&family, font_px);
        let px_of = |v: &str| -> String {
            if has_font_units(v) {
                font_lengths_to_px(v, font_px, 16.0, ex, ch)
            } else {
                v.to_string()
            }
        };
        let mut out = vec![];
        for (i, _) in images.iter().enumerate() {
            let mut c = self.clone();
            c.bg_lists.clear();
            c.background = None;
            if let Some(v) = &short {
                c.bg_image = None;
                c.gradient = None;
                c.gradient_raw = None;
                c.bg_size = BgSize::Auto;
                c.bg_pos = BgPos::default();
                c.bg_repeat = None;
                c.bg_origin = None;
                let layers = background_layers(v);
                c.apply_one("background", &px_of(layers[i % layers.len()]));
                c.background = None;
            }
            for (k, v) in self.bg_lists.iter().filter(|(k, _)| k != "background") {
                let layers = background_layers(v);
                c.apply_one(k, &px_of(layers[i % layers.len()]));
            }
            c.bg_lists.clear();
            // Градиент слоя — плиткой: источником идёт сама функция
            // градиента из записи слоя (в сокращении рядом с ней размер,
            // положение и повтор).
            if c.bg_image.is_none()
                && c.gradient.is_some()
                && let Some(r) = images.get(i)
                && let Some(at) = r.find("gradient(")
            {
                let start = r[..at]
                    .rfind(|ch: char| ch.is_whitespace() || ch == ',')
                    .map_or(0, |p| p + 1);
                let mut depth = 0i32;
                let mut end = r.len();
                for (j, ch) in r[at..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = at + j + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                c.bg_image = Some(r[start..end].to_string());
            }
            c.gradient = None;
            c.gradient_raw = None;
            out.push(c);
        }
        Some(out)
    }
}
