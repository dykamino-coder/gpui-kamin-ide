//! Счётчики и содержимое ::marker перед ::before и детьми.
use crate::document::state::ValueCx;
use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::select::Ancestor;
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    style: &mut Computed,
    marker_layer: Option<Box<Computed>>,
    is_list_item: bool,
    me: &Ancestor,
    path: &[Ancestor],
    attrs: &[(String, String)],
    counters: &mut crate::style::generated::counters::Counters,
    vcx: ValueCx<'_>,
) {
    // Содержимое маркера — по первому верному условию css-lists-3
    // §content-property: `content` на `::marker` не `normal` →
    // «exactly as for ::before»; `none` → коробки нет; иначе
    // прежний путь `list-style-*`. Blink делает ту же отсечку
    // первой строкой `ListMarker::MarkerText`:
    // `if (!marker.StyleRef().ContentBehavesAsNormal()) return
    // kNotText;` (`list_marker.cc:159`).
    //
    // Своей коробки у маркера в дереве нет — его рисует
    // `render::list` по `marker_text`/`no_marker`, поэтому
    // содержимое сворачивается в эти поля, а прочие свойства слоя
    // (цвет, шрифт, разрядка) едут в `marker_layer`.
    //
    // CSS Lists 3 §marker-pseudo: the marker is the item's FIRST child,
    // before ::before; counter-* declared on ::marker (with DOM
    // inheritance of counter directives) apply right here, before the
    // marker's own content is evaluated (`marker-counter`).
    if let Some(mut m) = marker_layer.filter(|_| is_list_item) {
        content::inherit_generated(&mut m, Some(&me));
        counters.enter_marker();
        language::pseudo(counters, &m, &me, path);
        apply_counter_decls(&m, counters, "", &[], &mut false, &|_, _| 0);
        if let Some(items) = host_content(&m, &me, vcx) {
            let quotes = m.quotes.as_ref();
            style.marker_text = Some(content_text(
                &items,
                counters,
                &attrs,
                quotes,
                me.html_attrs,
                crate::style::generated::counter_style::Context::new(&m, me.counter_context),
                vcx,
            ));
            style.no_marker = Some(false);
        } else if m.content_none == Some(true) {
            style.no_marker = Some(true);
        }
        counters.leave();
        style.marker_layer = Some(m);
    }
}
