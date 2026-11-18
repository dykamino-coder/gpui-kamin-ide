//! The style of the spacer run that holds an atomic inline's place in a line.

use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

/// The paragraph's style without word or letter spacing. Paragraphs of one
/// computed style share it through the box tree (`document::boxes::used`)
/// instead of a copy each.
pub(super) fn atom_spacer(inherited: &BoxStyle, opts: &RenderOpts) -> BoxStyle {
    let role = crate::document::boxes::used::UsedRole::AtomSpacer;
    let base = inherited.style_rc().clone();
    let mut mark = inherited.clone();
    let found = opts.cx.doc().boxes.borrow().used.find(&base, role);
    if let Some(spacer) = found {
        mark.set_style_rc(spacer);
        return mark;
    }
    mark.inh.text_flow.word_space_char = None;
    mark.inh.text_flow.letter_spacing = Some(Len::Px(0.0));
    let used = mark.style_rc().clone();
    opts.cx.doc().boxes.borrow().used.insert(&base, role, used);
    mark
}
