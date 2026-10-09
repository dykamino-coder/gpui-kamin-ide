//! Page margin boxes use generated content inside their resolved border boxes.

use super::*;

pub(crate) fn builder(
    declarations: PageMarginDeclsFn,
    root: Computed,
    opts: RenderOpts,
    counters: page_counters::PageCounters,
) -> crate::flow::MarginFn {
    let counters = std::cell::RefCell::new(counters);
    std::rc::Rc::new(move |i, name, pages, geometry| {
        page_margin_boxes(
            &declarations(i, name),
            i,
            pages,
            geometry,
            &root,
            &opts,
            &mut counters.borrow_mut(),
        )
    })
}

/// Марджин-боксы листа `page` из `pages` (css-page-3 §margin-boxes): элемент
/// и мера каждой ПОРОЖДЁННОЙ коробки — `content` не `none`/`normal`
/// (§populating-margin-boxes). Раскладку делает `flow::PageStack` по
/// `page_margin`. Элемент — гибкая колонка во весь border box: так
/// `vertical-align` коробки работает «как у ячейки таблицы» (§page-properties),
/// а `text-align` наследует блок содержимого.
fn page_margin_boxes(
    decls: &PageMarginDecls,
    page: usize,
    pages: usize,
    g: &crate::flow::PageGeom,
    root: &Computed,
    opts: &RenderOpts,
    counters: &mut page_counters::PageCounters,
) -> Vec<crate::flow::MarginBox> {
    let (ctx, boxes) = decls;
    let mut ctx_own = Computed::default();
    for (k, v) in ctx {
        ctx_own.apply_one(k, v);
    }
    page_counters::resolve(&mut ctx_own, root);
    counters.begin(page, pages, &ctx_own);
    let ctx_style = inline::inherit(root, &ctx_own);
    let mut out = Vec::new();
    for (slot, list) in boxes {
        let Some(place) = crate::page_margin::place(slot) else {
            continue;
        };
        let last = |key: &str| {
            list.iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.trim().to_string())
        };
        let Some(content) = last("content") else {
            continue;
        };
        if content == "none" || content == "normal" {
            continue;
        }
        let Some(items) = crate::computed::parse_content(&content)
            .and_then(|items| crate::dom::resolve_content_attributes(&items, &[], false))
        else {
            continue;
        };
        let (ta, va) = crate::page_margin::defaults(slot);
        let va = last("vertical-align").unwrap_or_else(|| va.to_string());
        let mut own = Computed::default();
        own.apply_one("text-align", ta);
        for (k, v) in list {
            if k != "content" && k != "vertical-align" {
                own.apply_one(k, v);
            }
        }
        page_counters::resolve(&mut own, &ctx_own);
        let resolved = inline::inherit(&ctx_style, &own);
        let fs = match resolved.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let mut content_counters = counters.for_margin(&own);
        let children = content_nodes(&items, &resolved, &mut content_counters);
        let cb = crate::page_margin::containing_block(place, g.size, g.margin);
        // Длина по базе: `auto` — `None`; проценты — от содержащего блока по
        // СВОЕЙ оси (Blink `kContainingBlockSize`).
        let len = |l: &Option<Len>, base: f32| -> Option<f32> {
            match l {
                Some(Len::Px(v)) => Some(*v),
                Some(Len::Pct(k)) => Some(k * base),
                Some(Len::Em(k)) => Some(k * fs),
                _ => None,
            }
        };
        let zero = |l: &Option<Len>, base: f32| len(l, base).unwrap_or(0.0);
        let b = own.borders();
        let pad = [
            zero(&own.padding.top, cb.3),
            zero(&own.padding.right, cb.2),
            zero(&own.padding.bottom, cb.3),
            zero(&own.padding.left, cb.2),
        ];
        let edges = [
            zero(&b.top, cb.3) + pad[0],
            zero(&b.right, cb.2) + pad[1],
            zero(&b.bottom, cb.3) + pad[2],
            zero(&b.left, cb.2) + pad[3],
        ];
        let auto_m = |l: &Option<Len>| matches!(l, Some(Len::Auto));
        let margin = [
            (!auto_m(&own.margin.top)).then(|| zero(&own.margin.top, cb.3)),
            (!auto_m(&own.margin.right)).then(|| zero(&own.margin.right, cb.2)),
            (!auto_m(&own.margin.bottom)).then(|| zero(&own.margin.bottom, cb.3)),
            (!auto_m(&own.margin.left)).then(|| zero(&own.margin.left, cb.2)),
        ];
        let bb = own.border_box == Some(true);
        let w = len(&own.width, cb.2).map(|v| if bb { v } else { v + edges[1] + edges[3] });
        let h = len(&own.height, cb.3).map(|v| if bb { v } else { v + edges[0] + edges[2] });
        // `size` — border box в точках (итог раскладки) либо `None` для меры по
        // содержимому: доли от обёртки не годятся, `blocks` кладёт коробку в
        // свою обёртку, и `height: 100%` решалась от неё как `auto` —
        // `vertical-align` терял высоту (`alignment-001`: буквы у верха).
        let opts = opts.clone();
        let ctx_style = ctx_style.clone();
        let intrinsic = [own.width, own.height];
        let build = move |size: Option<(f32, f32)>| -> AnyElement {
            let mut st = own.clone();
            for (k, v) in [
                ("padding-top", format!("{}px", pad[0])),
                ("padding-right", format!("{}px", pad[1])),
                ("padding-bottom", format!("{}px", pad[2])),
                ("padding-left", format!("{}px", pad[3])),
            ] {
                st.apply_one(k, &v);
            }
            let (sw, sh) = match size {
                Some((w, h)) => (format!("{w}px"), format!("{h}px")),
                None => ("auto".to_string(), "auto".to_string()),
            };
            for (k, v) in [
                ("width", sw.as_str()),
                ("height", sh.as_str()),
                ("min-width", "0"),
                ("max-width", "none"),
                ("min-height", "0"),
                ("max-height", "none"),
                ("margin", "0"),
                ("box-sizing", "border-box"),
                ("display", "flex"),
                ("flex-direction", "column"),
                (
                    "justify-content",
                    match va.as_str() {
                        "top" => "flex-start",
                        "bottom" => "flex-end",
                        _ => "center",
                    },
                ),
            ] {
                st.apply_one(k, v);
            }
            let inner = Element {
                tag: "div".to_string(),
                inline: false,
                node_id: 0,
                style: Computed::default(),
                hover: None,
                first_letter: None,
                first_line: None,
                children: children.clone(),
                attrs: vec![],
                anim: Default::default(),
                list_item: None,
            };
            let outer = Element {
                tag: "div".to_string(),
                style: st,
                children: vec![Node::Element(inner.clone())],
                ..inner
            };
            let mut els = blocks(&[Node::Element(outer)], &ctx_style, &opts);
            if els.len() == 1 {
                els.pop().unwrap()
            } else {
                div().children(els).into_any_element()
            }
        };
        let probe = build(None);
        out.push(crate::flow::MarginBox {
            place,
            make: std::rc::Rc::new(move |w, h| build(Some((w, h)))),
            probe,
            w,
            h,
            intrinsic,
            margin,
        });
    }
    out
}

// CSS Paged Media 3 §populating-margin-boxes uses the ordinary generated
// content model (CSS Content 3 §2), including quotes and inline images.
fn content_nodes(
    items: &[crate::computed::ContentItem],
    style: &Computed,
    counters: &mut crate::counters::Counters,
) -> Vec<Node> {
    use crate::computed::ContentItem;
    let mut children = Vec::new();
    let mut run = Vec::new();
    let flush = |run: &mut Vec<ContentItem>,
                 children: &mut Vec<Node>,
                 counters: &mut crate::counters::Counters| {
        if !run.is_empty() {
            children.push(Node::Text(crate::dom::content_text(
                run,
                counters,
                &[],
                style.quotes.as_ref(),
                false,
            )));
            run.clear();
        }
    };
    for item in items {
        if let ContentItem::Image(src) = item {
            flush(&mut run, &mut children, counters);
            if let Some(src) = crate::dom::content_image_src(src) {
                let mut image = super::anon_element("img", vec![]);
                image.inline = true;
                image.attrs.push(("src".into(), src));
                children.push(Node::Element(image));
            }
        } else {
            run.push(item.clone());
        }
    }
    flush(&mut run, &mut children, counters);
    children
}
