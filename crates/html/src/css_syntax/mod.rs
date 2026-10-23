//! Нейтральная лексика CSS Syntax: экраны, строки, url() и комментарии.
//! Не зависит от фаз конвейера; SVG-as-image и style используют одинаковые токены.

mod comments;
mod tokens;
mod unescape;
pub(crate) use comments::strip_comments;
pub(crate) use tokens::{at_url, escape_end, skip_string, skip_url};
pub use unescape::unescape;
