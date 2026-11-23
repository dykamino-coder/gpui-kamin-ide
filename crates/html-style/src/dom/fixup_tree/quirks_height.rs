//! Quirks percent heights inherit a block height through auto-height ancestors.
use crate::budget::admission::{Admission, Error};
use crate::dom::normalization::{Transform, apply};
use crate::dom::{Children, Element, Node};
use crate::style::{computed::Display, values::value::Len};

pub(crate) fn quirks_percent_heights(
    admission: &Admission,
    nodes: Vec<Node>,
    base: Option<f32>,
) -> Result<Vec<Node>, Error> {
    apply(&QuirksHeight, nodes, base, admission)
}

pub(crate) struct QuirksHeight;
impl Transform for QuirksHeight {
    type Context = Option<f32>;

    fn before_list(
        &self,
        nodes: &mut Children,
        _: &Self::Context,
        admission: &Admission,
    ) -> Result<(), Error> {
        mark_fill(nodes, admission)
    }

    fn enter(
        &self,
        e: &mut Element,
        &base: &Self::Context,
        _: &Admission,
    ) -> Result<Self::Context, Error> {
        let st = e.style.cascaded_mut();
        let out_of_flow = crate::style::flow::absolutely_positioned(st.reset_positioned.position);
        let blockish = matches!(
            st.reset_box_layout.display,
            None | Some(Display::Block | Display::InlineBlock | Display::ListItem)
        );
        if let (Some(Len::Pct(k)), Some(b), false, true) = (
            st.reset_box_layout.height.clone(),
            base,
            out_of_flow,
            blockish,
        ) {
            st.reset_box_layout.height = Some(Len::Px(k * b));
        }
        // Table cells break the quirks chain even when their own height is fixed.
        let tabular = matches!(
            st.reset_box_layout.display,
            Some(
                Display::Table
                    | Display::InlineTable
                    | Display::TableCell
                    | Display::TableRow
                    | Display::TableRowGroup
            )
        );
        Ok(match st.reset_box_layout.height {
            _ if tabular => None,
            Some(Len::Px(h)) => Some(h),
            None | Some(Len::Auto) if blockish && !out_of_flow && e.tag != "html" => base,
            _ => None,
        })
    }
}

fn fill(e: &Element) -> bool {
    e.style
        .cascaded()
        .reset_box_layout
        .float
        .is_none_or(|f| f == 0)
}

fn mark_fill(nodes: &mut Children, admission: &Admission) -> Result<(), Error> {
    let [Node::Element(html)] = nodes.as_slice() else {
        return Ok(());
    };
    if html.tag != "html" {
        return Ok(());
    }
    admission.work(html.children.len())?;
    let body = html
        .children
        .iter()
        .position(|n| matches!(n, Node::Element(e) if e.tag == "body" || e.tag == "frameset"));
    let body_fill = body.and_then(|i| match &html.children[i] {
        Node::Element(e) if e.tag == "body" => Some((i, fill(e), e.marks.quirks_fill)),
        _ => None,
    });
    let html_fill = fill(html);
    if html.marks.quirks_fill == html_fill && body_fill.is_none_or(|(_, want, had)| want == had) {
        return Ok(());
    }
    let Node::Element(html) = &mut nodes.edit(admission, 0)?[0] else {
        unreachable!()
    };
    html.marks.quirks_fill = html_fill;
    if let Some((i, want, had)) = body_fill
        && want != had
    {
        let Node::Element(body) = &mut html.children.edit(admission, 0)?[i] else {
            unreachable!()
        };
        // Preserve the exclusion before float layout clears the float property.
        body.marks.quirks_fill = want;
    }
    Ok(())
}
