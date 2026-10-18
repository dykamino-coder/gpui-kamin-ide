//! Списки слоёв фона (css-backgrounds-3 §2.1) по свойствам: типизированный
//! ключ вместо строкового имени свойства. Запись каждого списка остаётся
//! сырой до `Computed::bg_layers` — слой разбирается тем же путём, что и
//! одиночное объявление, иначе два разбора разошлись бы.

use super::*;

/// Фоновое свойство, значение которого — список слоёв через запятую.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgListProp {
    BlendMode,
    Shorthand,
    Image,
    Size,
    Position,
    Repeat,
    Origin,
    Clip,
    Attachment,
}

impl BgListProp {
    pub const ALL: [BgListProp; 9] = [
        BgListProp::BlendMode,
        BgListProp::Shorthand,
        BgListProp::Image,
        BgListProp::Size,
        BgListProp::Position,
        BgListProp::Repeat,
        BgListProp::Origin,
        BgListProp::Clip,
        BgListProp::Attachment,
    ];

    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "background-blend-mode" => BgListProp::BlendMode,
            "background" => BgListProp::Shorthand,
            "background-image" => BgListProp::Image,
            "background-size" => BgListProp::Size,
            "background-position" => BgListProp::Position,
            "background-repeat" => BgListProp::Repeat,
            "background-origin" => BgListProp::Origin,
            "background-clip" => BgListProp::Clip,
            "background-attachment" => BgListProp::Attachment,
            _ => return None,
        })
    }

    /// Имя свойства для разбора одного слоя через `apply_one`.
    pub const fn key(self) -> &'static str {
        match self {
            BgListProp::BlendMode => "background-blend-mode",
            BgListProp::Shorthand => "background",
            BgListProp::Image => "background-image",
            BgListProp::Size => "background-size",
            BgListProp::Position => "background-position",
            BgListProp::Repeat => "background-repeat",
            BgListProp::Origin => "background-origin",
            BgListProp::Clip => "background-clip",
            BgListProp::Attachment => "background-attachment",
        }
    }
}

/// Сырые списки слоёв по свойствам. Поля `bg_*` стиля несут верхний слой;
/// список хранится, только когда в записи больше одного слоя.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BgLists {
    lists: Vec<(BgListProp, String)>,
}

impl BgLists {
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }

    pub fn clear(&mut self) {
        self.lists.clear();
    }

    pub fn get(&self, prop: BgListProp) -> Option<&str> {
        self.lists
            .iter()
            .find(|(k, _)| *k == prop)
            .map(|(_, v)| v.as_str())
    }

    /// Положить список свойства, заменив прежний.
    pub fn set(&mut self, prop: BgListProp, raw: String) {
        self.remove(prop);
        self.lists.push((prop, raw));
    }

    pub fn remove(&mut self, prop: BgListProp) {
        self.lists.retain(|(k, _)| *k != prop);
    }

    /// Оставить только списки, для которых `keep` истинно.
    pub fn retain(&mut self, mut keep: impl FnMut(BgListProp) -> bool) {
        self.lists.retain(|(k, _)| keep(*k));
    }

    pub fn iter(&self) -> impl Iterator<Item = (BgListProp, &str)> {
        self.lists.iter().map(|(k, v)| (*k, v.as_str()))
    }

    /// Слои списка свойства (`background_layers`), если список задан.
    pub fn layers(&self, prop: BgListProp) -> Option<Vec<&str>> {
        self.get(prop).map(background_layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for p in BgListProp::ALL {
            assert_eq!(BgListProp::from_key(p.key()), Some(p));
        }
        assert_eq!(BgListProp::from_key("background-color"), None);
    }

    #[test]
    fn set_replaces_and_layers_split() {
        let mut l = BgLists::default();
        l.set(BgListProp::Image, "url(a), url(b)".into());
        l.set(BgListProp::Image, "none, url(c)".into());
        assert_eq!(l.layers(BgListProp::Image), Some(vec!["none", "url(c)"]));
        l.retain(|p| p == BgListProp::BlendMode);
        assert!(l.is_empty());
    }
}
