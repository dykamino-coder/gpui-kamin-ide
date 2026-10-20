//! CSS font-face names, descriptors and loaded sources belonging to one document.

use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub(crate) struct FontFaces {
    pub(super) aliases: RefCell<HashMap<String, String>>,
    pub(super) features: RefCell<HashMap<String, Vec<(String, u32)>>>,
    pub(super) faces: RefCell<HashMap<String, Vec<super::matching::Face>>>,
    pub(super) has_space: RefCell<HashMap<String, bool>>,
    pub(super) size_adjust: RefCell<HashMap<String, Vec<(u8, f32)>>>,
    pub(super) loaded: RefCell<HashMap<String, Option<String>>>,
    pub(super) romn_by_src: RefCell<HashMap<String, Option<f32>>>,
    pub(super) romn: RefCell<HashMap<String, f32>>,
}
