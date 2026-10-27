//! Собирает ресурсные таблицы стилей из разобранного HTML/XML, исключая инертное содержимое
//! template.

use markup5ever_rcdom::{Handle, NodeData};

use super::source_walk::{SourceWalk, Templates};
use crate::budget::admission::{Admission, Buffer, Error, Text};

/// Ресурсные источники: тема и активные style; ownership резервов остаётся у строк.
pub(crate) fn collect(input: &super::input::Admitted, theme: &str) -> Result<Buffer<Text>, Error> {
    let admission = Admission::new(input.context().budget().clone());
    let mut sheets = Buffer::default();
    sheets.push(Text::from_str(theme, &admission)?, &admission)?;
    collect_into(
        &input.dom.document,
        Templates::Active,
        &admission,
        &mut sheets,
    )?;
    Ok(sheets)
}

/// Один и тот же сборщик обслуживает document и shadow sheet без склейки EOF.
pub(crate) fn try_collect_style_tags(
    root: &Handle,
    admission: &Admission,
) -> Result<Buffer<Text>, Error> {
    let mut sheets = Buffer::default();
    collect_into(root, Templates::None, admission, &mut sheets)?;
    Ok(sheets)
}

fn collect_into(
    root: &Handle,
    templates: Templates,
    admission: &Admission,
    out: &mut Buffer<Text>,
) -> Result<(), Error> {
    let mut walk = SourceWalk::new(root, templates, admission.clone())?;
    while let Some(node) = walk.try_next()? {
        if let NodeData::Element { name, .. } = &node.data {
            admission.work(name.local.len())?;
            if &*name.local != "style" {
                continue;
            }
            let mut sheet = Text::default();
            for child in node.children.borrow().iter() {
                admission.work(1)?;
                if let NodeData::Text { contents } = &child.data {
                    let text = contents.borrow();
                    admission.work(text.len())?;
                    let body = text
                        .trim()
                        .strip_prefix("<![CDATA[")
                        .and_then(|body| body.strip_suffix("]]>"))
                        .unwrap_or(&text);
                    sheet.push_str(body, admission)?;
                }
            }
            out.push(sheet, admission)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_style_elements_and_active_shadow_templates_supply_css() {
        let html = "<!-- <style>comment</style> --><script>\"<style>script</style>\"</script>\
            <p>@font-face {not: css}</p><style>document</style>\
            <template><style>inert</style></template>\
            <div><template shadowrootmode='OPEN'><style>shadow</style></template>\
            <template shadowrootmode='open'><style>second-inert</style></template></div>";
        assert_eq!(
            collect(
                &super::super::input::Admitted::parse(html, false).unwrap(),
                ""
            )
            .unwrap()
            .iter()
            .skip(1)
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
            ["document", "shadow"]
        );
    }

    #[test]
    fn xml_style_content_uses_the_xml_parser() {
        let xml = "<svg xmlns='http://www.w3.org/2000/svg'>\
            <style><![CDATA[p { content: '<&>'; }]]></style></svg>";
        assert_eq!(
            collect(
                &super::super::input::Admitted::parse(xml, true).unwrap(),
                ""
            )
            .unwrap()
            .iter()
            .skip(1)
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
            ["p { content: '<&>'; }"]
        );
    }

    fn policy(
        kind: crate::budget::BudgetKind,
        cap: u64,
    ) -> (Admission, crate::budget::RevisionBudget) {
        use crate::budget::{DocumentBudget, DocumentLimits, ResourceGovernor};
        let revision = DocumentBudget::new(
            DocumentLimits::default().lower(kind, cap).unwrap(),
            ResourceGovernor::default(),
        )
        .unwrap()
        .revision();
        (Admission::new(revision.clone()), revision)
    }

    #[test]
    fn source_work_boundaries_and_live_failure_release_partial_sheets() {
        use crate::budget::BudgetKind::{LiveBytes, TraversalVisits};
        let input = super::super::input::Admitted::parse(
            "<style>KEEP</style><div><template shadowrootmode=open><style>LOST</style></template></div>",
            false,
        )
        .unwrap();
        let root = &input.dom.document;
        let run = |admission: &Admission| {
            let mut out = Buffer::default();
            collect_into(root, Templates::Active, admission, &mut out)?;
            Ok::<_, Error>(out)
        };
        let (admission, revision) = policy(TraversalVisits, 1_000_000);
        let sheets = run(&admission).unwrap();
        assert_eq!(
            sheets.iter().map(AsRef::as_ref).collect::<Vec<&str>>(),
            ["KEEP", "LOST"]
        );
        let needed = revision.used(TraversalVisits);
        assert!(revision.document().used(LiveBytes) > 0);
        drop(sheets);
        assert_eq!(revision.document().used(LiveBytes), 0);
        for cap in [needed - 1, needed, needed + 1] {
            let (admission, revision) = policy(TraversalVisits, cap);
            let result = run(&admission);
            assert_eq!(result.is_ok(), cap >= needed);
            if let Err(error) = &result {
                assert!(matches!(error, Error::Budget(_)));
            }
            drop(result);
            assert_eq!(revision.document().used(LiveBytes), 0);
        }
        let mut low = 0;
        let mut high = 65_536;
        while low < high {
            let mid = (low + high) / 2;
            let (admission, revision) = policy(LiveBytes, mid);
            let result = run(&admission);
            let success = result.is_ok();
            drop(result);
            assert_eq!(revision.document().used(LiveBytes), 0);
            if success {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        for cap in [low - 1, low, low + 1] {
            let (admission, revision) = policy(LiveBytes, cap);
            let result = run(&admission);
            assert_eq!(result.is_ok(), cap >= low);
            drop(result);
            assert_eq!(revision.document().used(LiveBytes), 0);
            assert_eq!(revision.document().governor().used(LiveBytes), 0);
        }
    }

    #[test]
    fn source_walk_failure_is_terminal_and_documents_are_independent() {
        use crate::budget::BudgetKind::{LiveBytes, TraversalVisits};
        let input =
            super::super::input::Admitted::parse("<style>KEEP</style><style>LOST</style>", false)
                .unwrap();
        let (admission, revision) = policy(TraversalVisits, 1);
        let mut walk = SourceWalk::new(&input.dom.document, Templates::Active, admission).unwrap();
        assert!(walk.try_next().unwrap().is_some());
        assert!(matches!(walk.try_next(), Err(Error::Budget(_))));
        assert_eq!(revision.document().used(LiveBytes), 0);
        let used = revision.used(TraversalVisits);
        assert!(walk.try_next().unwrap().is_none());
        assert_eq!(revision.used(TraversalVisits), used);
        let (first, first_revision) = policy(LiveBytes, 65_536);
        let (second, second_revision) = policy(LiveBytes, 65_536);
        let a = try_collect_style_tags(&input.dom.document, &first).unwrap();
        let b = try_collect_style_tags(&input.dom.document, &second).unwrap();
        drop(a);
        assert_eq!(first_revision.document().used(LiveBytes), 0);
        assert!(second_revision.document().used(LiveBytes) > 0);
        drop(b);
        assert_eq!(second_revision.document().used(LiveBytes), 0);
    }
}
