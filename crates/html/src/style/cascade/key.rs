//! Ключ каскада одного объявления (css-cascade-5 §6.2): происхождение с
//! важностью, контекст (shadow-including tree order), прикреплённость к
//! элементу (`style=""`), слой, специфичность, близость `@scope`, порядок
//! правила и место объявления в блоке. Ключи сравниваются целиком: больший —
//! сильнее. У важных объявлений происхождение, контекст и слой
//! переворачиваются, как велит §6.2 (важное агента сильнее важного автора,
//! ранний слой сильнее позднего).

use crate::style::css::Origin;

/// Сравнимый ключ каскада. Порядок полей — порядок критериев §6.2.
/// Умолчание — слабейший ключ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CascadeKey {
    /// Происхождение; у важных — `origin ^ 0xF` (переходы не переворачиваются).
    origin_imp: u8,
    /// Shadow-including tree order контекста; у важных перевёрнут.
    tree_order: u16,
    /// Объявление из `style=""` (§6.2 «Element-Attached Styles»): сильнее
    /// правил той же важности, не переворачивается.
    inline: bool,
    /// Ранг слоя среди слоёв каскада элемента; у важных перевёрнут.
    layer: u16,
    specificity: (u32, u32, u32),
    /// Близость корня `@scope`: больше — ближе; 0 — вне `@scope`.
    proximity: u32,
    rule_order: u32,
    decl_index: u32,
}

/// Уровень «слоя» ключа для `revert-layer` (Blink
/// `CascadePriority::ForLayerComparison`): происхождение, контекст,
/// прикреплённость и слой без учёта важности.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LayerLevel {
    origin: u8,
    tree_order: u16,
    inline: bool,
    layer: u16,
}

/// Статические части ключа одного источника (правила или `style=""`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySource {
    pub origin: Origin,
    pub tree_order: u16,
    pub inline: bool,
    pub layer: u16,
    pub specificity: (u32, u32, u32),
    pub proximity: u32,
    pub rule_order: u32,
}

impl CascadeKey {
    pub fn new(src: &KeySource, important: bool, decl_index: u32) -> Self {
        let origin = src.origin as u8;
        // Переходы сильнее всего и важности не имеют (§6.2, Blink
        // `CascadeOrigin::kTransition`).
        let flip = important && src.origin != Origin::Transition;
        CascadeKey {
            origin_imp: if flip { origin ^ 0xF } else { origin },
            tree_order: if flip {
                src.tree_order ^ 0xFFFF
            } else {
                src.tree_order
            },
            inline: src.inline,
            layer: if flip { src.layer ^ 0xFFFF } else { src.layer },
            specificity: src.specificity,
            proximity: src.proximity,
            rule_order: src.rule_order,
            decl_index,
        }
    }

    pub fn important(&self) -> bool {
        self.origin_imp < Origin::Transition as u8 && self.origin_imp & 0x8 != 0
    }

    pub fn origin(&self) -> Origin {
        let raw = if self.important() {
            self.origin_imp ^ 0xF
        } else {
            self.origin_imp
        };
        match raw {
            1 => Origin::UserAgent,
            2 => Origin::User,
            3 => Origin::PresHint,
            5 => Origin::Try,
            6 => Origin::Animation,
            16 => Origin::Transition,
            _ => Origin::Author,
        }
    }

    pub fn layer_level(&self) -> LayerLevel {
        let imp = self.important();
        LayerLevel {
            origin: self.origin() as u8,
            tree_order: if imp {
                self.tree_order ^ 0xFFFF
            } else {
                self.tree_order
            },
            inline: self.inline,
            layer: if imp { self.layer ^ 0xFFFF } else { self.layer },
        }
    }

    /// Ключ источника без места объявления в блоке.
    pub fn block_key(&self) -> CascadeKey {
        CascadeKey {
            decl_index: 0,
            ..*self
        }
    }

    pub fn decl_index(&self) -> u32 {
        self.decl_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(origin: Origin, layer: u16) -> KeySource {
        KeySource {
            origin,
            tree_order: 0,
            inline: false,
            layer,
            specificity: (0, 0, 0),
            proximity: 0,
            rule_order: 0,
        }
    }

    #[test]
    fn origin_and_importance_order() {
        let k = |o, imp| CascadeKey::new(&src(o, 0), imp, 0);
        let order = [
            k(Origin::UserAgent, false),
            k(Origin::User, false),
            k(Origin::PresHint, false),
            k(Origin::Author, false),
            k(Origin::Try, false),
            k(Origin::Animation, false),
            k(Origin::Author, true),
            k(Origin::User, true),
            k(Origin::UserAgent, true),
            k(Origin::Transition, false),
        ];
        for pair in order.windows(2) {
            assert!(pair[0] < pair[1], "{pair:?}");
        }
        assert!(k(Origin::Author, true).important());
        assert!(!k(Origin::Animation, false).important());
        assert_eq!(k(Origin::User, true).origin(), Origin::User);
    }

    #[test]
    fn important_inline_is_below_important_user_agent() {
        let inline = KeySource {
            inline: true,
            ..src(Origin::Author, 0)
        };
        let ua = CascadeKey::new(&src(Origin::UserAgent, 0), true, 0);
        let author_inline = CascadeKey::new(&inline, true, 0);
        let author_rule = CascadeKey::new(&src(Origin::Author, 5), true, 0);
        assert!(author_inline < ua);
        assert!(author_rule < author_inline);
        // Normal inline beats every normal author rule, any layer.
        assert!(
            CascadeKey::new(&src(Origin::Author, 9), false, 0) < CascadeKey::new(&inline, false, 0)
        );
    }

    #[test]
    fn layers_flip_for_important() {
        let early = src(Origin::Author, 1);
        let late = src(Origin::Author, 2);
        assert!(CascadeKey::new(&early, false, 0) < CascadeKey::new(&late, false, 0));
        assert!(CascadeKey::new(&early, true, 0) > CascadeKey::new(&late, true, 0));
        // The layer level ignores importance.
        assert_eq!(
            CascadeKey::new(&early, true, 0).layer_level(),
            CascadeKey::new(&early, false, 3).layer_level()
        );
        assert!(
            CascadeKey::new(&early, true, 0).layer_level()
                < CascadeKey::new(&late, false, 0).layer_level()
        );
    }

    #[test]
    fn specificity_then_order_then_position() {
        let a = KeySource {
            specificity: (0, 1, 0),
            rule_order: 1,
            ..src(Origin::Author, 0)
        };
        let b = KeySource {
            specificity: (0, 0, 1),
            rule_order: 9,
            ..src(Origin::Author, 0)
        };
        assert!(CascadeKey::new(&b, false, 0) < CascadeKey::new(&a, false, 0));
        let k0 = CascadeKey::new(&a, false, 0);
        let k1 = CascadeKey::new(&a, false, 1);
        assert!(k0 < k1 && k0.block_key() == k1.block_key());
        assert!(k0.block_key() != CascadeKey::new(&b, false, 0).block_key());
    }
}
