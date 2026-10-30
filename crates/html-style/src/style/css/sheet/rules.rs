//! Обход правил таблицы стилей (sheet_rules): @-правила, вложенность, слои,
//! @import/@media/@supports/@scope, сбор Rule.

use super::*;
use crate::budget::admission::{Admission, Buffer, Error};

mod at_rule;
use at_rule::at_rule_inner;

pub(super) fn sheet_rules(
    css: &str,
    media: Media,
    top: bool,
    cx: &mut SheetCx,
) -> Result<Vec<Rule>, Error> {
    let admission = Admission::new(cx.doc.revision_budget());
    let mut out = vec![];
    let cleaned = strip_comments(css);
    struct Frame<'a> {
        rest: &'a str,
        order: usize,
        offset: usize,
        top: bool,
        saved_layer: Option<(String, Vec<u32>)>,
    }
    let mut stack = Buffer::default();
    stack.push(
        Frame {
            rest: &cleaned,
            order: 0,
            offset: 0,
            top,
            saved_layer: None,
        },
        &admission,
    )?;
    loop {
        admission.work(1)?;
        let Some(frame) = stack.last_mut() else { break };
        if frame.top {
            frame.rest = stylesheet_tokens::start(frame.rest);
        }
        let Some((piece, tail)) = next_piece(frame.rest) else {
            if let Some(saved) = stack.pop().expect("completed stylesheet").saved_layer {
                cx.layer_now = saved;
            }
            continue;
        };
        frame.rest = tail;
        // At-правило-ПРЕДЛОЖЕНИЕ блока не имеет и кончается точкой с запятой:
        // `@import`, `@charset`, `@namespace`, `@layer a, b;`. Ни одно из них
        // ничего не задаёт нашей отрисовке, поэтому запись просто пропускается
        // вместе со всей своей преамбулой.
        let (head, body) = match piece {
            // `@layer a, b.c;` — объявление порядка слоёв без правил
            // (css-cascade-5 §6.4.2): имена регистрируются по месту.
            Piece::Statement { head } => {
                let h = head.trim();
                // `get`, а не срез: шестой байт бывает внутри многобайтового
                // знака (`@chars…` в чужой кодировке, `at-charset-029`), и
                // срез ронял весь стенд паникой.
                if h.len() > 6 && h.get(..6).is_some_and(|p| p.eq_ignore_ascii_case("@layer")) {
                    for name in h[6..].split(',') {
                        let name = name.trim();
                        if !name.is_empty() {
                            let _ = cx.layer_enter_path(name);
                        }
                    }
                }
                continue;
            }
            Piece::Block { head, body } => (head.trim(), body),
        };
        // Незакрытый блок в КОНЦЕ таблицы закрывается неявно (CSS Syntax
        // §5.4.1): правило всё равно действует. Прежде такое правило
        // отбрасывалось целиком — а в наборе оно встречается прямо в тесте
        // (`break-spaces-009`: у `.test` нет закрывающей скобки, и коробка
        // теряла свою ширину вместе со всем остальным).
        // At-правила: тело у них устроено иначе, поэтому обычными правилами
        // их применять нельзя. `@media` и `@supports` разбираются как обёртка
        // над обычными правилами, `@keyframes` — отдельно (см.
        // `parse_keyframes`), остальные пропускаются.
        if head.starts_with('@') {
            // Имя at-правила регистронезависимо (§3.3): `@MeDIa` — то же
            // самое, что `@media` (`case-sensitive-001`).
            let name = head.to_ascii_lowercase();
            let inner = at_rule_inner(&name, head, body, &media, cx);
            // Блок `@layer имя { … }`: правила внутри — в своём слое.
            let layer_block = name.starts_with("@layer");
            let saved_layer = layer_block.then(|| cx.enter_layer(head[6..].trim()));
            if inner {
                let offset = frame.offset + frame.order;
                frame.order += 1000;
                stack.push(
                    Frame {
                        rest: body,
                        order: 0,
                        offset,
                        top: false,
                        saved_layer,
                    },
                    &admission,
                )?;
            } else if let Some(saved) = saved_layer {
                cx.layer_now = saved;
            }
            continue;
        }
        // Список селекторов режется ВНЕ скобок: запятая внутри
        // `:nth-child(2n of #a, #b)` — часть of-списка, а не граница
        // селектора, иначе `#b` становился самостоятельным правилом.
        let parts = split_top_level(head, ',');
        // Пустая часть списка селекторов делает недействительным ВЕСЬ
        // список (CSS 2.1 §4.1.7): `body,,div {}` не применяется ни к кому.
        if parts.iter().any(|one| one.trim().is_empty()) {
            continue;
        }
        // Недействительная часть списка тоже валит ВЕСЬ список (§4.1.7):
        // `p:first-line.two, p.two {}` не применяется ни к кому. Прежде
        // негодная часть просто выбрасывалась, и правило красило соседа
        // (`c25-pseudo-elmnt-000`).
        let parts = super::nesting::top_level_parts(&parts, &cx.selectors);
        let parse = |one: &String| Selector::parse_in(one, &cx.selectors);
        let sels: Option<Vec<Selector>> = parts.iter().map(parse).collect();
        let Some(sels) = sels else { continue };
        let start = out.len();
        if super::nesting::rule(&parts, body, &media, cx, &mut out, &mut frame.order)? {
            for rule in &mut out[start..] {
                rule.order += frame.offset;
            }
            continue;
        }
        let decls = parse_decls_in(body, cx.doc.values());
        if decls.is_empty() {
            continue;
        }
        for sel in sels {
            out.push(Rule {
                sel,
                decls: decls.clone(),
                order: frame.offset + frame.order,
                // Происхождение ставит тот, кто подключает таблицу (см. `dom`).
                origin: crate::style::css::Origin::UserAgent,
                layer: cx.layer_of_rules(),
                link: crate::style::css::LinkMatch::Any,
            });
            frame.order += 1;
        }
    }
    Ok(out)
}
