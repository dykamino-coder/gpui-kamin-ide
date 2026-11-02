//! Проекция appearance использует те же откаты и подстановки, что полный каскад.

use super::*;

impl Computed {
    pub fn appearance_from_map(
        map: &CascadeMap<'_>,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) -> Self {
        let mut out = vcx.doc.initial_style();
        let mut picks = map.picks("appearance");
        if let Some(first) = picks.next() {
            out.apply_winner("appearance", first, picks, vars, &ComputeCx::new(cx, vcx));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_projection_agrees_with_full_cascade() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        for raw in [
            "appearance:base",
            "appearance:base;appearance:none",
            "appearance:base;all:initial",
            "appearance:var(--a)",
            "appearance:revert",
            "appearance:initial",
            "appearance:attr(data-a)",
        ] {
            let block = crate::style::css::parse_decls(raw);
            let map = CascadeMap::single(&block, Origin::Author);
            let vars = VarMap::default();
            let projected = Computed::appearance_from_map(&map, &vars, CascadeCx::NONE, vcx);
            let mut full = Computed::default();
            full.apply_map(&map, &vars, CascadeCx::NONE, vcx);
            assert_eq!(
                projected.reset_context.appearance, full.reset_context.appearance,
                "{raw}"
            );
        }
    }
}
