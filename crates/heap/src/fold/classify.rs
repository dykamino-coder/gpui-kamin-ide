//! Stack classification shared by live and offline reports. Prefix tests are
//! anchored: allocator names inside generic arguments never hide a caller.
const ALLOCATOR: &[&str] = &[
    "kamin_alloc::profiled::",
    "kamin_profiler::allocation::",
    "engine_bench::allocations::",
    "engine_bench::_::__rust",
    "alloc::alloc::",
    "alloc::raw_vec::",
    "alloc::rc::",
    "alloc::sync::",
    "alloc::boxed::",
    "core::alloc::",
    "std::alloc::",
    "std::sys::alloc::",
    "__rust",
    "__rdl",
    "__rg",
    "_mi_",
    "mi_",
    "RtlCaptureStackBackTrace",
];
const PLUMBING: &[&str] = &[
    "alloc::",
    "core::",
    "std::",
    "hashbrown::",
    "smallvec::",
    "indexmap::",
    "arrayvec::",
    "?",
];
const SUBSYSTEMS: &[(&str, &str)] = &[
    ("kamin_html::layout::reuse", "retained"),
    ("kamin_html::document::retained", "retained"),
    ("kamin_html::text", "text"),
    ("gpui::text_system", "text"),
    ("cosmic_text", "text"),
    ("swash", "text"),
    ("harfrust", "text"),
    ("rustybuzz", "text"),
    ("ttf_parser", "text"),
    ("skrifa", "text"),
    ("read_fonts", "text"),
    ("dwrote", "text"),
    ("kamin_font_facts", "text"),
    ("unicode_", "text"),
    ("icu_", "text"),
    ("hyphenation", "text"),
    ("kamin_style::dom", "dom"),
    ("html5ever", "dom"),
    ("markup5ever", "dom"),
    ("xml5ever", "dom"),
    ("tendril", "dom"),
    ("xml::", "dom"),
    ("kamin_style::resources", "resources"),
    ("kamin_html::document::style", "style"),
    ("kamin_style", "style"),
    ("cssparser", "style"),
    ("selectors", "style"),
    ("kamin_html::paint", "paint"),
    ("gpui::scene", "display-list"),
    ("gpui::window::Window::paint", "display-list"),
    ("gpui::window::Window::insert_primitive", "display-list"),
    ("kamin_html::svg", "svg-images"),
    ("resvg", "svg-images"),
    ("usvg", "svg-images"),
    ("tiny_skia", "svg-images"),
    ("image::", "svg-images"),
    ("png::", "svg-images"),
    ("zune_", "svg-images"),
    ("kamin_html::layout", "layout"),
    ("taffy", "layout"),
    ("gpui::taffy", "layout"),
    ("kamin_html::render", "elements"),
    ("gpui::elements", "elements"),
    ("gpui::element", "elements"),
    ("gpui::arena", "elements"),
    ("gpui_windows", "gpu"),
    ("gpui_pre_windows", "gpu"),
    ("gpui::platform", "gpu"),
    ("reblessive", "recursion-stacks"),
    ("kamin_html::document", "document"),
    ("kamin_html::host", "bench-host"),
    ("engine_bench", "bench-host"),
    ("gpui", "gpui-other"),
    ("kamin_html", "html-other"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub name: String,
    pub subsystem: String,
    pub owned_type: Option<String>,
    pub stack: Vec<String>,
}

pub fn classify(names: Vec<String>) -> Site {
    let cut = names
        .iter()
        .take_while(|name| ALLOCATOR.iter().any(|prefix| name.starts_with(prefix)))
        .count();
    let owned_type = names[..cut]
        .iter()
        .filter_map(|name| owned(name))
        .next_back();
    let stack = names[cut..].to_vec();
    let index = stack
        .iter()
        .position(|name| !PLUMBING.iter().any(|prefix| name.starts_with(prefix)))
        .unwrap_or(0);
    let name = stack.get(index).cloned().unwrap_or_else(|| "?".into());
    let subsystem = stack[index..]
        .iter()
        .find_map(|name| {
            SUBSYSTEMS
                .iter()
                .find_map(|(prefix, owner)| name.starts_with(prefix).then_some(*owner))
        })
        .unwrap_or("other")
        .to_owned();
    Site {
        name,
        subsystem,
        owned_type,
        stack,
    }
}

fn owned(name: &str) -> Option<String> {
    if !["alloc::rc::Rc", "alloc::sync::Arc", "alloc::boxed::Box"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return None;
    }
    let (_, constructor) = name.rsplit_once("::new")?;
    let start = constructor.find('<')?;
    if !constructor[..start]
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    let value = constructor[start + 1..].strip_suffix('>')?;
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leading_allocator_only_owned_type_and_nearest_subsystem() {
        let site = classify(
            [
                "alloc::sync::Arc::new<doc::Node>",
                "alloc::vec::Vec::push",
                "kamin_html::layout::build<alloc::alloc::Global>",
                "kamin_html::document::new",
            ]
            .map(str::to_owned)
            .to_vec(),
        );
        assert_eq!(site.name, "kamin_html::layout::build<alloc::alloc::Global>");
        assert_eq!(site.subsystem, "layout");
        assert_eq!(site.owned_type.as_deref(), Some("doc::Node"));
        assert_eq!(site.stack.len(), 3);
        assert_eq!(classify(vec![]).name, "?");
    }
}
