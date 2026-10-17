//! `iframe`.
// owner: A

use crate::dom::Element;
use crate::layout::block::reorder::orthogonal_vertical_children;
use crate::layout::replaced::limits::responsive_embedded_sizing;
use crate::render::{RenderOpts, blocks, styled_div};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

/// Картинка: `src` с `data:`-URI или путь. Внешние URL не грузим — документ
/// рисуется в чате, где сеть запрещена по тем же причинам, что и в вебвью.
/// Приклеить базовую папку к относительным `url(...)` вложенного документа.
fn resolve_embedded_urls(html: &str, dir: &std::path::Path) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find("url(") {
        out.push_str(&rest[..at + 4]);
        rest = &rest[at + 4..];
        let Some(end) = rest.find(')') else { break };
        let raw = &rest[..end];
        let inner = raw.trim().trim_matches('"').trim_matches('\'');
        if inner.starts_with("data:") || inner.contains("://") || inner.starts_with('/') {
            out.push_str(raw);
        } else {
            let abs = format!("file:///{}", dir.join(inner).display()).replace('\\', "/");
            out.push('"');
            out.push_str(&abs);
            out.push('"');
        }
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

thread_local! {
    /// Глубина вложенных документов — от циклических iframe.
    pub(crate) static IFRAME_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `<object>`, чей `data` — ДОКУМЕНТ, а не картинка (HTML §4.8.7: сначала
/// атрибут `type`, иначе по расширению адреса). Гейт нарочно узкий: `.svg`,
/// `image/*` и растры остаются на пути картинки (css-images
/// `object-fit-*-svg-*o` — 32 зелёных пары, `object-fit-*-png-*o` — 12).
/// Запрос и якорь адреса отрезаются до проверки расширения.
pub(crate) fn object_is_document(e: &Element) -> bool {
    if let Some(t) = e.attr("type") {
        let t = t.trim().to_ascii_lowercase();
        if t.starts_with("text/html") || t.starts_with("application/xhtml+xml") {
            return true;
        }
        if t.starts_with("image/") {
            return false;
        }
    }
    let url = e.attr("data").unwrap_or_default();
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    [".html", ".htm", ".xht", ".xhtml"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

pub(crate) fn iframe(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    if IFRAME_DEPTH.with(|d| d.get()) >= 3 {
        return None;
    }
    // HTML: the presence of srcdoc (even empty) takes precedence over src.
    let html = if let Some(html) = e.attr("srcdoc") {
        html.to_owned()
    } else {
        let src = e.attr("src")?;
        let path = src
            .strip_prefix("file:///")
            .or_else(|| src.strip_prefix("file://"))?;
        let html = std::fs::read_to_string(path).ok()?;
        match std::path::Path::new(path).parent() {
            Some(dir) => resolve_embedded_urls(&html, dir),
            None => html,
        }
    };
    // Resolve against the containing block, before sizing the child viewport.
    let mut resolved = super::image::pct_height_to_px(e, inherited);
    resolved.style.resolve_viewport(opts.viewport);
    let in_flow = !matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute | crate::style::computed::Position::Fixed)
    );
    if in_flow
        && let Some(Len::Pct(k)) = resolved.style.width
        && let Some(width) = crate::layout::block::containing::AVAIL_W.with(|v| v.get())
    {
        resolved.style.width = Some(Len::Px(k * width));
    }
    if in_flow
        && let Some(Len::Pct(k)) = resolved.style.height
        && let Some(height) = crate::layout::block::available_height::HEIGHT.with(|v| v.get())
    {
        resolved.style.height = Some(Len::Px(k * height));
    }
    let e = &resolved;
    let attr_len = |k: &str| e.attr(k).and_then(|v| v.parse::<f32>().ok());
    // Размер: CSS сильнее атрибутов; умолчание — 300×150 (CSS 2.2 §замещаемые).
    // Рамка и отбивка лежат СНАРУЖИ размера содержимого (CSS 2.1 §10.3.2,
    // `box-sizing: content-box`), а раскладка меряет `w`/`h` как border-box:
    // без поправки `border: 1px` съедал 300×150 изнутри
    // (`responsive-iframe-unsized-ref`: 302×152).
    let st = &e.style;
    let px_sum = |sides: &[Option<Len>]| -> f32 {
        sides
            .iter()
            .filter_map(|s| match s {
                Some(Len::Px(v)) => Some(*v),
                _ => None,
            })
            .sum()
    };
    let bw = st.borders();
    let (ex, ey) = if crate::style::apply::intrinsic_size::native_content_box(st) {
        (0.0, 0.0)
    } else {
        (
            px_sum(&[st.padding.left, st.padding.right, bw.left, bw.right]),
            px_sum(&[st.padding.top, st.padding.bottom, bw.top, bw.bottom]),
        )
    };
    let border_box = st.border_box == Some(true);
    let (w, outer_w) = match st.width {
        Some(Len::Px(v)) if border_box => ((v - ex).max(0.0), v),
        Some(Len::Px(v)) => (v, v + ex),
        _ => {
            let v = attr_len("width").unwrap_or(300.0);
            (v, v + ex)
        }
    };
    let (h, outer_h) = match st.height {
        Some(Len::Px(v)) if border_box => ((v - ey).max(0.0), v),
        Some(Len::Px(v)) => (v, v + ey),
        _ => {
            let v = attr_len("height").unwrap_or(150.0);
            (v, v + ey)
        }
    };
    // Рамка меряет свои `@media` своей коробкой (`doc::parse_embedded`).
    // Режим quirks у вложенного документа свой: разбор его перепишет, а
    // внешний возвращается после сборки рамки.
    let outer_quirks = crate::style::select::quirks();
    let responsive = st.frame_sizing_height
        && matches!(st.height, None | Some(Len::Auto))
        && e.attr("height").is_none()
        && responsive_embedded_sizing(&html);
    let (nodes, salt) = crate::document::parse_embedded_sized(
        &html,
        crate::BROWSER_CSS,
        (w, h),
        responsive,
        !e.attr("scrolling")
            .is_some_and(|v| v.eq_ignore_ascii_case("no")),
        Some(crate::document::EmbeddingScheme {
            preferred_dark: st.embedding_dark_scheme,
            owner_dark: st.dark_color_scheme,
        }),
    );
    // Верхний уровень вложенного документа проходит те же ортогональные
    // поправки, что и дети контейнера.
    let nodes = orthogonal_vertical_children(nodes, &Computed::default());

    let mut sub = opts.clone();
    sub.viewport = (w, h);
    sub.doc_salt = salt;
    IFRAME_DEPTH.with(|d| d.set(d.get() + 1));
    // Свой слой ICB на вложенный документ: его абсолюты без позиционированного
    // предка держатся ЕГО начального содержащего блока — области просмотра
    // рамки, а не внешней страницы (HTML §4.8.5 «nested browsing context»;
    // `abs-pos-non-replaced-icb-*`: коробка с `right: 80%` улетала в левый
    // верхний угол внешнего документа).
    let icb_layer = crate::layout::positioned::layer_scope::LayerScope::icb();
    let mut kids = blocks(&nodes, &sub.root_style(), &sub);
    kids.extend(icb_layer.finish());
    IFRAME_DEPTH.with(|d| d.set(d.get() - 1));
    crate::style::select::QUIRKS.with(|q| q.set(outer_quirks));
    Some(
        {
            let mut frame = styled_div(e).w(px(outer_w));
            // CSS 2.2 §10.8.1 / CSS Inline 3 §A.3: a replaced iframe has no
            // baseline from its embedded document; synthesize from its own box.
            frame.style().baseline_unavailable = Some(true);
            // css-sizing-4 §frame-sizing: высота по содержимому, когда
            // вложенный документ согласился и автор высоту не задал.
            if responsive {
                frame
            } else {
                frame.h(px(outer_h))
            }
        }
        .overflow_hidden()
        .relative()
        .flex_shrink_0()
        .children(kids)
        .into_any_element(),
    )
}
