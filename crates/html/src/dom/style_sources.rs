//! Собирает ресурсные таблицы стилей из разобранного HTML/XML, исключая инертное содержимое
//! template.

use markup5ever_rcdom::{Handle, NodeData};

pub(crate) fn collect(markup: &str, xml: bool) -> Vec<String> {
    let dom = if xml {
        super::xml::xml_dom(markup)
    } else {
        super::input::html(markup)
    };
    #[expect(
        clippy::redundant_clone,
        reason = "keep the root alive: Node::drop drains descendant children"
    )]
    let mut pending = vec![dom.document.clone()];
    let mut sheets = Vec::new();
    let mut active_templates = std::collections::HashSet::new();
    while let Some(node) = pending.pop() {
        if let Some((template, _)) = super::shadow::declarative_shadow(&node) {
            active_templates.insert(std::rc::Rc::as_ptr(&template));
        }
        if let NodeData::Element {
            name,
            template_contents,
            ..
        } = &node.data
        {
            if &*name.local == "style" {
                let mut sheet = String::new();
                for child in node.children.borrow().iter() {
                    if let NodeData::Text { contents } = &child.data {
                        let text = contents.borrow();
                        let body = text
                            .trim()
                            .strip_prefix("<![CDATA[")
                            .and_then(|body| body.strip_suffix("]]>"))
                            .unwrap_or(&text);
                        sheet.push_str(body);
                    }
                }
                sheets.push(sheet);
            }
            if &*name.local == "template"
                && active_templates.contains(&std::rc::Rc::as_ptr(&node))
                && let Some(contents) = template_contents.borrow().as_ref()
            {
                pending.push(contents.clone());
            }
        }
        pending.extend(node.children.borrow().iter().rev().cloned());
    }
    sheets
}

/// Содержимое `<style>`-тегов документа, каждый — отдельной таблицей;
/// HTML сохраняет сырой текст, а XHTML раскрывает ссылки на знаки
/// на стадии XML-разбора, до передачи дерева этому сборщику.
pub(crate) fn collect_style_tags(handle: &Handle, out: &mut Vec<String>) {
    if let NodeData::Element { name, .. } = &handle.data
        && &*name.local == "style"
    {
        let mut sheet = String::new();
        for child in handle.children.borrow().iter() {
            if let NodeData::Text { contents } = &child.data {
                // Обёртка `<![CDATA[ … ]]>` встречается в эталонах XHTML: там
                // она прячет стиль от разбора XML. Разбору CSS она мусор, и
                // правила до первой закрывающей скобки пропадали вместе с ней.
                let text = contents.borrow();
                let trimmed = text.trim();
                let body = trimmed
                    .strip_prefix("<![CDATA[")
                    .and_then(|rest| rest.strip_suffix("]]>"))
                    .unwrap_or(&text);
                // Сохраняем текст детей: добавленный LF меняет строки CSS на EOF.
                sheet.push_str(body);
            }
        }
        out.push(sheet);
    }
    for child in handle.children.borrow().iter() {
        collect_style_tags(child, out);
    }
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
        assert_eq!(collect(html, false), ["document", "shadow"]);
    }

    #[test]
    fn xml_style_content_uses_the_xml_parser() {
        let xml = "<svg xmlns='http://www.w3.org/2000/svg'>\
            <style><![CDATA[p { content: '<&>'; }]]></style></svg>";
        assert_eq!(collect(xml, true), ["p { content: '<&>'; }"]);
    }
}
