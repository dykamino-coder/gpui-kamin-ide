//! Поиск приемлемого якоря в текущем реестре и временный fallback прошлого кадра.

use super::super::resolve::last_named;
use super::super::settle::DefaultAnchor;
use super::super::{AnchorPlan, AnchorRec};

impl AnchorPlan {
    /// Запись якоря по имени (или якоря по умолчанию) — только приемлемого.
    pub(in crate::layout::positioned::anchor) fn rec(
        &self,
        name: Option<&str>,
    ) -> Option<AnchorRec> {
        self.rec_with(name, self.history)
    }

    pub(in crate::layout::positioned::anchor) fn rec_current(
        &self,
        name: Option<&str>,
    ) -> Option<AnchorRec> {
        self.rec_with(name, false)
    }

    fn rec_with(&self, name: Option<&str>, history: bool) -> Option<AnchorRec> {
        // Реестр ТЕКУЩЕГО кадра знает только якоря, уже прошедшие подготовку.
        // Якорь раньше по дереву, но в слое, который готовится ПОЗЖЕ коробки
        // (слой ICB у `fixed`, `containing_block::layers::icb_close` — последние дети
        // документа), в нём ещё не записан: `anchor-abspos-to-fixedpos-001` —
        // `fixed`-якорь и абсолют без краёв в потоке, коробка падала на
        // статическую позицию под якорь. Спека требует лишь «laid out
        // strictly before» по дереву (§target) — его и проверяет фильтр `seq`
        // у записи прошлого кадра (стенд ждёт устоявшихся кадров, как у
        // `anchor-size()`). Якорь ниже по дереву (`anchor-position-circular`)
        // фильтр отсекает.
        let named = |n: &str| {
            self.values
                .vcx()
                .doc
                .anchor_frame
                .named
                .borrow()
                .get(n)
                .copied()
                .or_else(|| {
                    history
                        .then(|| last_named(n, self.seq, self.values.vcx()))
                        .flatten()
                })
        };
        let r = match name {
            Some(n) => named(n),
            None => match &self.default_anchor {
                Some(DefaultAnchor::Named(n)) => named(n.as_str()),
                Some(DefaultAnchor::Implicit(id)) => self
                    .values
                    .vcx()
                    .doc
                    .anchor_frame
                    .implicit
                    .borrow()
                    .get(id)
                    .copied()
                    .or_else(|| {
                        if !history {
                            return None;
                        }
                        self.values
                            .vcx()
                            .doc
                            .anchor_frame
                            .last_implicit
                            .borrow()
                            .get(id)
                            .copied()
                    }),
                None => None,
            },
        }?;
        self.acceptable(&r).then_some(r)
    }

    /// §target, acceptable anchor element: «same original containing block»
    /// либо элемент, порождающий содержащий блок якоря, сам приемлем — то
    /// есть содержащий блок коробки лежит в цепочке содержащих блоков
    /// якоря (или сам является якорем: неявный якорь позиционированного
    /// хозяина). Начальный содержащий блок (окно, `fixed`) принимает всех;
    /// звено без пробы (строчный или атомарный содержащий блок) — цепочка
    /// неизвестна, якорь не отвергается (`no-anchor-anchor-center`).
    fn acceptable(&self, r: &AnchorRec) -> bool {
        if self.fixed || self.cb_node == 0 || r.id == self.cb_node {
            return true;
        }
        let mut n = r.cb;
        for _ in 0..64 {
            if n == self.cb_node {
                return true;
            }
            if n == 0 {
                return false;
            }
            match self
                .values
                .vcx()
                .doc
                .anchor_frame
                .cb_parent
                .borrow()
                .get(&n)
                .copied()
            {
                Some(p) => n = p,
                None => return true,
            }
        }
        true
    }
}
