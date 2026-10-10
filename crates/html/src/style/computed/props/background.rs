//! Computed::apply_one: background*, box-shadow, object-*, image-orientation.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod gradient_layer;
mod layer_props;
mod shorthand;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_background(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // Сокращение несёт всё сразу: `background: #fff url(a.png) no-repeat`
            // — и цвет, и картинку, и режим повтора. Раньше побеждало что-то
            // одно, и картинка терялась при заданном цвете.
            "background" | "background-color" => self.apply_background_shorthand(key, val, v, hit),
            "box-shadow" if has_font_units(v) => self.shadow_raw = Some(v.to_string()),
            "box-shadow" => {
                self.shadow_raw = None;
                if v == "inherit" {
                    self.shadow_inherit = true;
                    return;
                }
                // `inset` в записи означает тень ВНУТРИ фигуры: раньше такая
                // запись просто не рисовалась.
                // Негодная запись ОТБРАСЫВАЕТСЯ целиком, прежняя тень живёт
                // (§7.1 `none | <shadow>#`; `box-shadow-invalid-001`).
                if !box_shadow_valid(v) {
                    return;
                }
                let (inset, outer): (Vec<&str>, Vec<&str>) = crate::style::css::split_args(v)
                    .into_iter()
                    .partition(|one| one.contains("inset"));
                self.shadows = parse_shadows(&outer.join(","));
                self.inset_shadows = parse_shadows(&inset.join(",").replace("inset", " "));
            }
            "object-fit" => self.object_fit = Some(v.to_string()),
            // `image-orientation` (css-images-3 §5.4): `from-image | none |
            // [<angle> || flip]`. Угол со `flip` спека сама помечает
            // необязательным и устаревшим («optional to implement and
            // deprecated»), и в корпусе его не просит ни один рефтест —
            // разбираем два ключевых слова.
            "image-orientation" => self.image_orient_none = Some(v.trim() == "none"),
            "background-image" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_IMAGE;
                    return;
                }
                // `none` ГАСИТ картинку (§14.2.1): ветки под него не было
                // вовсе, и заданный ранее адрес переживал отмену.
                if v.trim().eq_ignore_ascii_case("none") {
                    self.bg_image = None;
                    self.gradient = None;
                    self.gradient_raw = None;
                } else if v.starts_with("linear-gradient(") || v.starts_with("radial-gradient(") {
                    // Негодная запись роняет ОБЪЯВЛЕНИЕ (§4.2), прежняя
                    // картинка живёт: `linear-gradient(green, green)` и следом
                    // четыре негодных угла обязаны оставить зелёный.
                    if let Some(g) = parse_gradient(v) {
                        self.gradient = Some(g);
                        self.gradient_em = has_font_units(v).then(|| v.to_string());
                        // Сырая запись нужна фону РЯДА таблицы: он рисуется
                        // слоем картинки, и градиент туда идёт источником.
                        self.gradient_raw = Some(v.to_string());
                    }
                } else if let Some(rest) = v.strip_prefix("filter(") {
                    // `filter(<image>, <filter-list>)` (filter-effects-1 §12):
                    // фильтр применяется К КАРТИНКЕ, не к элементу — цвета
                    // градиента пересчитываются на месте.
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let parts = crate::style::css::split_args(inner);
                    if let Some(img) = parts.first().map(|p| p.trim())
                        && (img.starts_with("linear-gradient(")
                            || img.starts_with("radial-gradient("))
                        && let Some(mut g) = parse_gradient(img)
                    {
                        let mut tmp = Self::default();
                        tmp.apply_one("filter", &parts[1..].join(" "));
                        if let Some(f) = tmp.filter {
                            g.from = f.apply(g.from);
                            g.to = f.apply(g.to);
                            for stop in g.stops.iter_mut() {
                                stop.0 = f.apply(stop.0);
                            }
                        }
                        self.gradient = Some(g);
                    } else if let Some(img) = parts.first().map(|p| p.trim())
                        && gradient_as_raster(img)
                    {
                        // Конический и повторяющиеся идут растровой плиткой
                        // (`gradient_as_raster`): фильтр доносится до цветов
                        // стопов прямо в записи — растеризатор получает уже
                        // пересчитанные цвета (filter-effects-1 §12
                        // `filter()`: фильтр применяется к КАРТИНКЕ).
                        let mut tmp = Self::default();
                        tmp.apply_one("filter", &parts[1..].join(" "));
                        self.bg_image = Some(match tmp.filter {
                            Some(f) => filter_gradient_text(img, &f),
                            None => img.to_string(),
                        });
                    }
                } else if gradient_as_raster(v) {
                    // Конический и ПОВТОРЯЮЩИЕСЯ GPU-путь не выражает — они
                    // идут растровой плиткой (css-images-3 §3.6,
                    // css-images-4 §2.3; растеризатор уже есть, а
                    // `background::source` эти записи опознаёт с самого
                    // начала — до `Computed` они просто не доезжали).
                    self.bg_image = Some(v.to_string());
                } else if let Some(rest) = v.strip_prefix("image(") {
                    // `image(<url>? , <color>?)` (css-images-4 §2.4): цвет —
                    // запасной слой; сплошная заливка выражается градиентом
                    // из одного цвета.
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let mut url = None;
                    for part in crate::style::css::split_args(inner) {
                        let part = part.trim();
                        if let Some(u) = parse_url(part) {
                            url = Some(u);
                        }
                    }
                    match url {
                        Some(u) => self.bg_image = Some(u),
                        None if parse_image_color(v).is_some() => {
                            // A color image has no natural dimensions (CSS Images 4
                            // §2.3). Keep it in the image layer, including currentColor
                            // until the element's text color has been resolved.
                            self.bg_image = Some(v.to_string());
                            self.gradient = None;
                            self.gradient_raw = None;
                        }
                        _ => {}
                    }
                } else if let Some(rest) = v
                    .strip_prefix("image-set(")
                    .or_else(|| v.strip_prefix("-webkit-image-set("))
                {
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    match image_set_pick(inner) {
                        // Функция ПРЕДСТАВЛЯЕТ выбранный `<image>` (§2.5,
                        // шаг 4): он разбирается тем же кодом, что без
                        // обёртки, — адрес, градиент, конический, повтор.
                        // Кандидат приходит СЫРОЙ записью (`url(...)`): голый
                        // путь ни одна ветка не принимает, и прошлый заход
                        // (09.09, +9/−17) терял на этом все адреса.
                        Some(Some(chosen)) => self.apply_one("background-image", &chosen),
                        // Годная запись без пригодных кандидатов — «invalid
                        // image» (шаг 3): слой ГАСНЕТ, прежний не выживает.
                        Some(None) => {
                            self.bg_image = None;
                            self.gradient = None;
                            self.gradient_raw = None;
                        }
                        // Негодная запись (отрицательное разрешение числом —
                        // css-values-4 §6.3): объявление не применяется (§4.2).
                        None => {}
                    }
                } else if v.trim_end().ends_with(')')
                    && let Some(url) = parse_url(v)
                {
                    // Хвост после `url(...)` делает объявление недействительным
                    // (§4.2): `background-image: url(x) repeat` не картинка.
                    self.bg_image = Some(url);
                }
            }

            _ => self.apply_background_layers(key, val, v, hit),
        }
    }
}
