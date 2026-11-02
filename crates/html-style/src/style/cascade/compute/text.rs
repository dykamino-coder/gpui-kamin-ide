//! Наследование текста, интерфейса и оформления.
mod inherit;
mod values;
use super::*;
pub(super) use inherit::inherit_text;
pub(in crate::style::cascade) use values::{decor_family, resolve_decor_len};
pub(super) use values::{inherit_decoration_values, inherit_ui};
