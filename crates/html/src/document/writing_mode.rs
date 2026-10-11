//! Writing mode for document; split out to keep the owning module within 250 lines.

use super::any_containment;
use crate::dom::Node;

pub(super) fn propagate_writing_mode(mut nodes: Vec<Node>) -> Vec<Node> {
    let [Node::Element(html)] = nodes.as_mut_slice() else {
        return nodes;
    };
    if html.tag != "html" {
        return nodes;
    }
    // Ограничение на корне гасит распространение: корень с ним — сам себе
    // область, и наружу его письмо не выходит.
    if any_containment(html) {
        // Тело своё письмо СОХРАНЯЕТ — гасится распространение, а не
        // вычисленное значение (css-writing-modes §3.1). Значит главным
        // потоком тело не стало, и полагающийся главному потоку прижим к
        // краю окна ему не положен. Рисователь обособления КОРНЯ уже не
        // видит, поэтому пометку ставим здесь
        // (contain-html-w-m-001..004).
        for n in html.children.iter_mut() {
            if let Node::Element(b) = n
                && b.tag == "body"
            {
                b.style.wm_contained = true;
            }
        }
        return nodes;
    }
    // `html::before`/`::after` — СОСЕДИ body в потоке страницы: растяжка
    // body на весь вьюпорт (min_h в render) выталкивала их за нижний край,
    // и текст псевдо пропадал (wm-propagation-body-044). Свой нулевой
    // минимум гасит только навязанный, заданного не трогает.
    let has_root_pseudo = html
        .children
        .iter()
        .any(|n| matches!(n, Node::Element(p) if p.tag == "::after" || p.tag == "::before"));
    if has_root_pseudo {
        for n in html.children.iter_mut() {
            if let Node::Element(b) = n
                && b.tag == "body"
                && b.style.height.is_none()
                && b.style.min_height.is_none()
            {
                b.style.min_height = Some(crate::style::values::value::Len::Px(0.0));
            }
        }
    }
    let Some(body) = html.children.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) else {
        return nodes;
    };
    // Ограничение на теле оставляет письмо ему: наверх идёт только
    // собственное письмо корня (contain-body-{w-m,t-o}-001..004).
    let body_contained = any_containment(body);
    let taken = if body_contained {
        (
            html.style.vertical,
            html.style.vertical_rl,
            html.style.rtl,
            html.style.sideways,
        )
    } else {
        (
            body.style.vertical.or(html.style.vertical),
            body.style.vertical_rl.or(html.style.vertical_rl),
            body.style.rtl.or(html.style.rtl),
            body.style.sideways.or(html.style.sideways),
        )
    };
    // Та же пометка, что и при обособлении корня: письмо тела осталось при
    // теле, область просмотра его не приняла (css-contain-2
    // §containment-types), и тело — обычный блок в потоке корня. Ставится
    // ПОСЛЕ вычисления `taken`, чтобы неизменяемый заём `body` уже кончился
    // (contain-body-w-m-001..004).
    if body_contained {
        for n in html.children.iter_mut() {
            if let Node::Element(b) = n
                && b.tag == "body"
            {
                b.style.wm_contained = true;
            }
        }
    }
    let own = (
        html.style.vertical,
        html.style.vertical_rl,
        html.style.rtl,
        html.style.sideways,
    );
    // Главное вертикальное письмо: строчная ось корня занимает всё окно
    // (§8.2) — И при письме, заданном на самом `<html>` (taken == own):
    // эталон wm-propagation-047 ставит sideways-lr на корень, и без 100vh
    // прижимать содержимое к низу не от чего.
    // До раннего выхода — только sideways-lr (нужен низ, wm-prop-047-ref);
    // обычные вертикальные корни с письмом на самом html так теряли
    // скроллер-структуру (available-size-022/023 замерено).
    // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2): якорь самим
    // стилем; корню с фоном-картинкой не ставится — гасил canvas-слой
    // (замерено на background-size-document-root-vrl-*).
    if taken.1 == Some(true) && html.style.align_self.is_none() && html.style.bg_image.is_none() {
        html.style.align_self = Some(crate::style::computed::Align::End);
    }
    // Вертикальный корень с ФОНОМ-КАРТИНКОЙ: без минимума высоты его
    // коробка при пустом теле нулевая, и краске негде лечь
    // (background-size-document-root-vrl-*). Обычным вертикальным корням
    // минимум не ставится — терялась скроллер-структура (av-size-022/023).
    if taken.0 == Some(true)
        && html.style.bg_image.is_some()
        && html.style.height.is_none()
        && html.style.min_height.is_none()
    {
        html.style.min_height = Some(crate::style::values::value::Len::Vh(1.0));
    }
    let side_lr = taken.3 == Some(true) && taken.1 != Some(true);
    if taken.0 == Some(true)
        && side_lr
        && html.style.height.is_none()
        && html.style.min_height.is_none()
    {
        html.style.min_height = Some(crate::style::values::value::Len::Vh(1.0));
    }
    if taken == own {
        return nodes;
    }
    // Вычисленные значения не меняются ни у кого (§8.2): распространяется
    // только used корневой коробки. Псевдо-дети html (`::before`/`::after`)
    // наследуют СОБСТВЕННОЕ письмо html — оно прикалывается им заранее
    // (wm-propagation-body-044: html vlr + body htb, текст псевдо вертикален).
    for child in html.children.iter_mut() {
        let Node::Element(e) = child else { continue };
        if !(e.tag == "::after" || e.tag == "::before") {
            continue;
        }
        e.style.vertical = e.style.vertical.or(own.0);
        e.style.vertical_rl = e.style.vertical_rl.or(own.1);
        e.style.rtl = e.style.rtl.or(own.2);
        e.style.sideways = e.style.sideways.or(own.3);
    }
    if taken.0 == Some(true) && html.style.height.is_none() && html.style.min_height.is_none() {
        html.style.min_height = Some(crate::style::values::value::Len::Vh(1.0));
    }
    (
        html.style.vertical,
        html.style.vertical_rl,
        html.style.rtl,
        html.style.sideways,
    ) = taken;
    nodes
}
