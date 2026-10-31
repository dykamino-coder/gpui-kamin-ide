//! Нейтральная лексика CSS Syntax: экраны, строки, url() и комментарии.
//! Не зависит от фаз конвейера; SVG-as-image и style используют одинаковые токены.

pub mod admission;
mod comments;
mod functions;
mod tokens;
mod unescape;
pub use comments::strip_comments;
pub use functions::functions;
pub use tokens::{
    at_url, escape_end, escaped_codepoint_end, scan_string, skip_string, skip_url, skip_url_body,
    try_skip_string, try_skip_url,
};
pub use unescape::unescape;
