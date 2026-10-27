//! XHTML: самозакрывающиеся теги в HTML-разборе.

mod scan;
#[cfg(test)]
mod tests;

/// Для HTML-адаптера XHTML с xmlns на разобранном корне HTML-парсер
/// на НЕ-void теге признак самозакрытия
/// игнорирует: `<div class="a"/>` открывал элемент, и всё дальнейшее
/// вкладывалось в него (css-flexbox `.xhtml`, семьи с
/// `<div/>`-распорками). Такие теги разворачиваются в пару `<tag …></tag>`
/// до разбора; void-элементы и содержимое `svg`/`math` (там парсер
/// самозакрытие понимает) не трогаются.
pub(super) fn expand_xhtml_self_closing<'a>(
    html: &'a str,
    budget: &crate::budget::RevisionBudget,
) -> Result<
    (
        std::borrow::Cow<'a, str>,
        Option<crate::budget::LiveReservation>,
    ),
    crate::dom::outcome::FailureReason,
> {
    use crate::budget::BudgetKind;
    use crate::dom::outcome::FailureReason;
    // Six source-wide feature searches and at most two source lengths copied.
    // Tag/body searches below admit each inspected byte before touching it.
    let work = (html.len() as u64)
        .checked_mul(8)
        .ok_or(FailureReason::CapacityOverflow)?;
    budget
        .consume(BudgetKind::TraversalVisits, work)
        .map_err(FailureReason::Budget)?;
    // `<pre>`/`<listing>`/`<textarea>` в XHTML тоже требуют правки (см. ниже),
    // даже если самозакрытых тегов в документе нет.
    let lf_tags = ["<pre", "<listing", "<textarea"]
        .iter()
        .any(|t| html.contains(t));
    if !html.contains("/>") && !lf_tags && !html.contains("<!--") && !html.contains('&') {
        return Ok((std::borrow::Cow::Borrowed(html), None));
    }
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr", "basefont", "frame", "keygen",
    ];
    // Each rewritten tag adds no more bytes than its original spelling;
    // entity decoding contracts, and an inserted LF is shorter than its tag.
    // Two source lengths therefore cover every output branch without growth.
    let capacity = html
        .len()
        .checked_mul(2)
        .ok_or(FailureReason::CapacityOverflow)?;
    let lease = budget
        .document()
        .reserve(BudgetKind::LiveBytes, capacity as u64)
        .map_err(FailureReason::Budget)?;
    let mut out = String::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| FailureReason::Allocation)?;
    let scan = scan::Scan(budget);
    let mut rest = html;
    let mut foreign = 0usize;
    while let Some(at) = scan.byte(rest, b'<')? {
        scan.work(17)?;
        out.push_str(&rest[..at]);
        let tag = &rest[at..];
        // Раздел CDATA — дословный текст до `]]>` (XML 1.0 §2.7): `<!--`
        // внутри него — не комментарий, а символы таблицы стилей
        // (`sgml-comments-000`: CSS видит CDO и правило за ним).
        if tag.starts_with("<![CDATA[") {
            let end = scan
                .literal(tag, "]]>")?
                .map(|e| e + 3)
                .unwrap_or(tag.len());
            out.push_str(&tag[..end]);
            rest = &tag[end..];
            continue;
        }
        // Комментарий XML (XML 1.0 §2.5) — разметка, а не текст, и в том
        // числе внутри `<style>`: HTML-разбор держит его в сыром тексте
        // таблицы, и слова комментария становились мусорным селектором,
        // который глотал СЛЕДУЮЩЕЕ правило (эталоны
        // `flexbox-align-self-vert-*-ref.xhtml`: `.centerParent
        // { text-align: center }` за комментарием терялось, середина
        // уезжала к левому краю). Вне сырого текста комментарий ничего не
        // рисует — выбрасывается целиком.
        if tag.starts_with("<!--") {
            let end = scan
                .literal(tag, "-->")?
                .map(|e| e + 3)
                .unwrap_or(tag.len());
            rest = &tag[end..];
            continue;
        }
        if tag.starts_with("<!") || tag.starts_with("<?") {
            let end = scan.byte(tag, b'>')?.map(|e| e + 1).unwrap_or(tag.len());
            out.push_str(&tag[..end]);
            rest = &tag[end..];
            continue;
        }
        // Конец тега — с учётом кавычек в значениях атрибутов.
        let mut end = None;
        let mut quote: Option<u8> = None;
        for (i, b) in tag.bytes().enumerate().skip(1) {
            scan.work(1)?;
            match (quote, b) {
                (Some(q), _) if b == q => quote = None,
                (Some(_), _) => {}
                (None, b'"') | (None, b'\'') => quote = Some(b),
                (None, b'>') => {
                    end = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            out.push_str(tag);
            rest = "";
            break;
        };
        let inner = &tag[1..end];
        let name_end = scan.name_end(inner)?;
        scan.work(
            inner
                .len()
                .checked_mul(3)
                .ok_or(FailureReason::CapacityOverflow)?,
        )?;
        let name = &inner[..name_end];
        if let Some(open) = name.strip_prefix('/') {
            if scan.matches(open, &["svg", "math"])? {
                foreign = foreign.saturating_sub(1);
            }
            out.push_str(&tag[..=end]);
        } else if inner.trim_end().ends_with('/') {
            if foreign > 0 || scan.matches(name, VOID)? || name.is_empty() {
                out.push_str(&tag[..=end]);
            } else {
                let attrs = inner.trim_end().trim_end_matches('/');
                out.push('<');
                out.push_str(attrs);
                out.push_str("></");
                out.extend(name.chars().map(|c| c.to_ascii_lowercase()));
                out.push('>');
            }
        } else {
            if scan.matches(name, &["svg", "math"])? {
                foreign += 1;
            }
            out.push_str(&tag[..=end]);
            // Первый перевод строки после `<pre>` выбрасывает только HTML-разбор
            // (`ignore_lf`, `vendor/html5ever/src/tree_builder/mod.rs:536`); в XML
            // такого правила нет, и XHTML-документ держит его строкой. Лишний
            // `\n` отдаётся разборщику на съедение, исходный остаётся:
            // `c548-ln-ht-000` (`pre.control` — 5 строк, у нас было 4),
            // `white-space-pre-001` (эталон ждёт 7 строк).
            let after = &tag[end + 1..];
            // В XML у `<style>`/`<script>` нет «сырого текста»: ссылки на
            // символы в нём раскрываются (XML 1.0 §4.4.2, §4.6), и правило
            // `div#test &gt; span` значит `div#test > span`. HTML-разбор
            // оставил бы `&gt;` буквами, и селектор пропадал целиком
            // (`inline-table-zorder-003…005`). Раскрываем до разбора —
            // кроме разделов CDATA (дословный текст) и комментариев (их
            // выбрасывает общий проход выше).
            if foreign == 0 && scan.matches(name, &["style", "script"])? {
                scan.work(5)?;
                let close = if name.eq_ignore_ascii_case("style") {
                    "</style"
                } else {
                    "</script"
                };
                let body_end = scan.close(after, close)?.unwrap_or(after.len());
                let mut body = &after[..body_end];
                while !body.is_empty() {
                    scan.work(15)?;
                    if body.starts_with("<![CDATA[") {
                        let e = scan
                            .literal(body, "]]>")?
                            .map(|e| e + 3)
                            .unwrap_or(body.len());
                        out.push_str(&body[..e]);
                        body = &body[e..];
                    } else if body.starts_with("<!--") {
                        let e = scan
                            .literal(body, "-->")?
                            .map(|e| e + 3)
                            .unwrap_or(body.len());
                        body = &body[e..];
                    } else if body.starts_with('&') {
                        scan.work(36)?;
                        let semi = body.bytes().take(12).position(|b| b == b';');
                        let ch = semi.and_then(|e| match &body[1..e] {
                            "lt" => Some('<'),
                            "gt" => Some('>'),
                            "amp" => Some('&'),
                            "quot" => Some('"'),
                            "apos" => Some('\''),
                            r if r.starts_with("#x") || r.starts_with("#X") => {
                                u32::from_str_radix(&r[2..], 16)
                                    .ok()
                                    .and_then(char::from_u32)
                            }
                            r if r.starts_with('#') => {
                                r[1..].parse::<u32>().ok().and_then(char::from_u32)
                            }
                            _ => None,
                        });
                        match (ch, semi) {
                            (Some(c), Some(e)) => {
                                out.push(c);
                                body = &body[e + 1..];
                            }
                            _ => {
                                out.push('&');
                                body = &body[1..];
                            }
                        }
                    } else {
                        let e = scan
                            .either(body)?
                            .map(|e| if e == 0 { 1 } else { e })
                            .unwrap_or(body.len());
                        out.push_str(&body[..e]);
                        body = &body[e..];
                    }
                }
                rest = &after[body_end..];
                continue;
            }
            scan.work(3)?;
            if foreign == 0
                && scan.matches(name, &["pre", "listing", "textarea"])?
                && (after.starts_with('\n') || after.starts_with("\r\n"))
            {
                out.push('\n');
            }
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    Ok((std::borrow::Cow::Owned(out), Some(lease)))
}
