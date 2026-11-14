//! Итог баланса и роста строк; новый кадр строит собственные элементы и рекордеры.

use super::Recipe;
use crate::dom::BoxStyles;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::Window;

mod eligibility;
mod state;
pub(super) use eligibility::simple_tree;
pub(super) use state::Cache;
use state::{Child, Extent, Key, Plan};

impl Recipe {
    pub(super) fn restore_numeric(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &Window,
    ) -> bool {
        let Some(cache) = &self.numeric else {
            return false;
        };
        cache.validate(window);
        let Some(key) = Key::new(stack, width) else {
            return false;
        };
        let plans = cache.plans.borrow();
        let Some(plan) = plans.iter().find(|p| p.key == key) else {
            return false;
        };
        if plan.kids.len() != stack.children.len()
            || !plan.kids.iter().zip(&stack.children).all(|(saved, child)| {
                saved.source == child.source && saved.growth.is_some() == child.regrow.is_some()
            })
        {
            return false;
        }
        for (saved, child) in plan.kids.iter().zip(&mut stack.children) {
            super::super::column_regrow::restore_geometry(child, &saved.geometry);
            if let (Some(growth), Some(regrow)) = (&saved.growth, &child.regrow) {
                regrow.watched.flow.restore_growth(growth);
                super::super::column_regrow::rebuild(child);
            }
        }
        stack.forced_h = plan.height;
        crate::work::add!(MulticolNumericReuses, 1);
        true
    }

    pub(super) fn remember_numeric(&self, stack: &ColumnStack, width: f32, window: &Window) {
        let Some(cache) = &self.numeric else { return };
        let Some(key) = Key::new(stack, width) else {
            return;
        };
        if stack.balance_failure.is_some() {
            return;
        }
        let mut kids = Vec::with_capacity(stack.children.len());
        for child in &stack.children {
            let geometry = child.kid();
            if !geometry.decorations.is_empty()
                || geometry.float_min != 0.
                || child.paginate.is_some()
                || child.clone_dec.is_some()
                || child.nested_cols
                || child.repeat.is_some()
                || child.line_trim.is_some()
            {
                return;
            }
            let growth = if let Some(r) = &child.regrow {
                if !crate::dom::same_tree_as(
                    &r.source,
                    &BoxStyles::of(&r.source),
                    &r.source.children,
                    &r.dom,
                ) {
                    return;
                }
                Some(r.watched.flow.snapshot_growth())
            } else {
                None
            };
            kids.push(Child {
                source: child.source,
                geometry,
                growth,
            });
        }
        cache.validate(window);
        let mut plans = cache.plans.borrow_mut();
        plans.retain(|p| p.key != key);
        if plans.len() == 4 {
            plans.remove(0);
        }
        plans.push(Plan {
            key,
            height: stack.forced_h,
            kids,
        });
    }

    pub(super) fn cached_extent(
        &self,
        stack: &ColumnStack,
        width: f32,
        window: &Window,
    ) -> Option<Extent> {
        let cache = self.numeric.as_ref()?;
        cache.validate(window);
        let key = Key::new(stack, width)?;
        cache
            .extents
            .borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, e)| {
                crate::work::add!(MulticolExtentReuses, 1);
                *e
            })
    }

    pub(super) fn remember_extent(
        &self,
        stack: &ColumnStack,
        width: f32,
        extent: Extent,
        window: &Window,
    ) {
        let Some(cache) = &self.numeric else { return };
        let Some(key) = Key::new(stack, width) else {
            return;
        };
        if stack.balance_failure.is_some() {
            return;
        }
        cache.validate(window);
        let mut extents = cache.extents.borrow_mut();
        extents.retain(|(k, _)| *k != key);
        if extents.len() == 4 {
            extents.remove(0);
        }
        extents.push((key, extent));
    }
}
