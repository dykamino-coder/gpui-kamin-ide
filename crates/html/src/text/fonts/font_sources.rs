//! Ordered font-source activation, retaining successful and failed URL cache entries.

use crate::budget::admission::{Admission, Error};
use crate::values::ValueCx;

pub(super) fn load(
    block: &str,
    base: Option<&url::Url>,
    fonts: &crate::text::text_fonts::TextFonts,
    vcx: ValueCx<'_>,
    admission: &Admission,
) -> Result<Option<(String, String)>, Error> {
    super::face_rules::try_source(block, |path| {
        let src = crate::resources::url_loader::resolve(&path, base, admission)?;
        let cached = vcx.doc.font_faces.loaded.borrow().get(&src).cloned();
        let real = match cached {
            Some(hit) => hit,
            None => {
                let loaded = super::read_font(&src, vcx.doc).and_then(|bytes| {
                    super::math::remember(&src, &bytes, fonts);
                    let romn = super::sfnt_romn_baseline(&bytes);
                    vcx.doc
                        .font_faces
                        .romn_by_src
                        .borrow_mut()
                        .insert(src.clone(), romn);
                    crate::text::metrics::load_font(bytes, fonts)
                });
                vcx.doc
                    .font_faces
                    .loaded
                    .borrow_mut()
                    .insert(src.clone(), loaded.clone());
                loaded
            }
        };
        Ok(real.map(|real| (src, real)))
    })
}
