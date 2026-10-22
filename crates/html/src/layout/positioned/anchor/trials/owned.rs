//! Принятые выражения владеют телами: ручки арены кадра не переживают paint.

use super::snapshot::Accepted;
use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::{
    AnchorFn, Len, Sum, anchor_get, anchor_store, calc_get, calc_store,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Owned {
    properties: Accepted,
    lengths: Vec<Option<Length>>,
}

impl Owned {
    pub(super) fn of(c: &Computed, vcx: ValueCx<'_>) -> Self {
        let mut properties = Accepted::of(c);
        let mut lengths = Vec::new();
        properties.lengths(|value| {
            lengths.push(value.map(|v| Length::of(v, vcx)));
            *value = None;
        });
        Self {
            properties,
            lengths,
        }
    }

    pub(super) fn apply(&self, c: &mut Computed, vcx: ValueCx<'_>) {
        let mut properties = self.properties.clone();
        let mut lengths = self.lengths.iter();
        properties.lengths(|value| {
            *value = lengths.next().and_then(|v| v.as_ref().map(|v| v.load(vcx)));
        });
        properties.apply(c);
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Length {
    Plain(Len),
    Calc(Sum),
    Anchor(Box<Anchor>),
}

impl Length {
    fn of(value: Len, vcx: ValueCx<'_>) -> Self {
        match value {
            Len::Calc(id) => Self::Calc(calc_get(id, vcx)),
            Len::Anchor(id) => match anchor_get(id, vcx) {
                Some(f) => Self::Anchor(Box::new(Anchor::of(&f, vcx))),
                None => Self::Plain(Len::Auto),
            },
            value => Self::Plain(value),
        }
    }
    fn load(&self, vcx: ValueCx<'_>) -> Len {
        match self {
            Self::Plain(value) => *value,
            Self::Calc(sum) => Len::Calc(calc_store(*sum, vcx)),
            Self::Anchor(f) => Len::Anchor(anchor_store(f.load(vcx), vcx)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Anchor {
    function: AnchorFn,
    fallback: Option<Length>,
    alts: Vec<Anchor>,
}

impl Anchor {
    fn of(f: &AnchorFn, vcx: ValueCx<'_>) -> Self {
        let mut function = f.clone();
        function.fallback = None;
        function.alts.clear();
        Self {
            function,
            fallback: f.fallback.map(|v| Length::of(v, vcx)),
            alts: f.alts.iter().map(|f| Self::of(f, vcx)).collect(),
        }
    }
    fn load(&self, vcx: ValueCx<'_>) -> AnchorFn {
        let mut f = self.function.clone();
        f.fallback = self.fallback.as_ref().map(|v| v.load(vcx));
        f.alts = self.alts.iter().map(|f| f.load(vcx)).collect();
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;

    #[test]
    fn accepted_expressions_survive_their_arena_by_value() {
        let doc = DocState::new(Default::default());
        let mut c = Computed::default();
        c.width = Len::parse("anchor-size(--a width, 3px)", doc.frame_values());
        if let Some(Len::Anchor(id)) = c.width {
            let mut f = (*anchor_get(id, doc.frame_values()).expect("parsed anchor-size")).clone();
            f.fallback = Len::parse_mixed("calc(3px + 20%)", doc.frame_values());
            c.width = Some(Len::Anchor(anchor_store(f, doc.frame_values())));
        }
        c.inset.top = Len::parse(
            "anchor(--a 25%, anchor(--b bottom, 9px))",
            doc.frame_values(),
        );
        c.margin.left = Len::parse_mixed("calc(7px + 10%)", doc.frame_values());
        let saved = Owned::of(&c, doc.frame_values());
        drop(doc);
        let doc = DocState::new(Default::default());
        let mut restored = Computed::default();
        saved.apply(&mut restored, doc.frame_values());
        assert_eq!(Owned::of(&restored, doc.frame_values()), saved);
        assert!(matches!(restored.width, Some(Len::Anchor(_))));
        assert!(matches!(restored.inset.top, Some(Len::Anchor(_))));
        assert!(matches!(restored.margin.left, Some(Len::Calc(_))));
    }
}
