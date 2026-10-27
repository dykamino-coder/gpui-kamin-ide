//! Допуск всех CSS-источников до base/style/font/profile и прямого DOM entrypoint.

use super::{
    input::Admitted,
    source_walk::{SourceWalk, Templates},
};
use crate::{
    budget::admission::{Admission, Error},
    css_syntax::admission::admit,
};
use markup5ever_rcdom::NodeData;

pub(crate) fn admit_sources(input: &Admitted, theme: &str) -> Result<(), Error> {
    let admission = Admission::new(input.context().budget().clone());
    admit(super::user_agent_css(), &admission)?;
    admit(super::ua_style::QUIRKS_CSS, &admission)?;
    let sheets = super::style_sources::collect(input, theme)?;
    for css in sheets.iter() {
        admit(css, &admission)?;
    }
    let mut walk = SourceWalk::new(&input.dom.document, Templates::Active, admission.clone())?;
    while let Some(node) = walk.try_next()? {
        if let NodeData::Element { attrs, .. } = &node.data {
            for attr in attrs.borrow().iter() {
                admission.work(1)?;
                if &*attr.name.local == "style" || &*attr.name.local == "content" {
                    admit(&attr.value, &admission)?;
                }
            }
        }
    }
    Ok(())
}
