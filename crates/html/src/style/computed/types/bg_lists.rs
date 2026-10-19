//! Типизированные списки фоновых компонентов и циклическое повторение слоёв.

use crate::document::state::ValueCx;

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
    #[cfg(test)]
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

/// Списки вычисленных фоновых компонентов в порядке объявлений.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BgLists {
    lists: Vec<(BgListProp, Vec<super::bg_layer::BgLayer>)>,
}

impl BgLists {
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }
    pub fn clear(&mut self) {
        self.lists.clear();
    }
    pub(crate) fn get(&self, prop: BgListProp) -> Option<&[super::bg_layer::BgLayer]> {
        self.lists
            .iter()
            .find(|(k, _)| *k == prop)
            .map(|(_, v)| v.as_slice())
    }
    /// Разбор списка выполняется в каскаде тем же диспетчером, что и одного слоя.
    pub fn set(&mut self, prop: BgListProp, raw: String, vcx: ValueCx<'_>) {
        let layers = super::background_layers(&raw)
            .into_iter()
            .map(|v| super::bg_layer::BgLayer::parse(prop, v, None, vcx))
            .collect();
        self.set_layers(prop, layers);
    }
    pub(crate) fn set_layers(&mut self, prop: BgListProp, layers: Vec<super::bg_layer::BgLayer>) {
        self.remove(prop);
        self.lists.push((prop, layers));
    }
    pub fn remove(&mut self, prop: BgListProp) {
        self.lists.retain(|(k, _)| *k != prop);
    }
    pub fn retain(&mut self, mut keep: impl FnMut(BgListProp) -> bool) {
        self.lists.retain(|(k, _)| keep(*k));
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = (BgListProp, &[super::bg_layer::BgLayer])> {
        self.lists.iter().map(|(k, v)| (*k, v.as_slice()))
    }
    pub(crate) fn position_expressions(
        &mut self,
    ) -> impl Iterator<Item = &mut super::super::background_position::PositionExpression> {
        self.lists.iter_mut().flat_map(|(_, layers)| {
            layers
                .iter_mut()
                .flat_map(|layer| layer.position_math.iter_mut().flatten())
        })
    }
    pub(crate) fn resolve_font(&mut self, font: f32, ex: f32, ch: f32, vcx: ValueCx<'_>) {
        for (prop, layers) in &mut self.lists {
            for layer in layers {
                if let Some(raw) = layer.pending.take() {
                    *layer =
                        super::bg_layer::BgLayer::parse(*prop, &raw, Some((font, ex, ch)), vcx);
                }
            }
        }
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
    fn replacement_keeps_only_new_typed_layers() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut l = BgLists::default();
        l.set(BgListProp::Clip, "border-box, padding-box".into(), vcx);
        l.set(BgListProp::Clip, "text, content-box".into(), vcx);
        let mut c = super::super::Computed::default();
        let layers = l.get(BgListProp::Clip).unwrap();
        assert_eq!(layers.len(), 2);
        layers[1].apply(BgListProp::Clip, &mut c);
        assert_eq!(c.bg_clip, Some(super::super::BgClip::ContentBox));
        l.retain(|p| p == BgListProp::BlendMode);
        assert!(l.is_empty());
    }
}
