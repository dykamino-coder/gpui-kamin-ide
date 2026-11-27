//! Debug formatting of the parsed SVG document.
use super::{Document, SvgNode};

impl std::fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        if !self.root().has_children() {
            return write!(f, "Document []");
        }

        macro_rules! writeln_indented {
            ($depth:expr, $f:expr, $fmt:expr) => {
                for _ in 0..$depth { write!($f, "    ")?; }
                writeln!($f, $fmt)?;
            };
            ($depth:expr, $f:expr, $fmt:expr, $($arg:tt)*) => {
                for _ in 0..$depth { write!($f, "    ")?; }
                writeln!($f, $fmt, $($arg)*)?;
            };
        }

        fn print_children(
            parent: SvgNode,
            depth: usize,
            f: &mut std::fmt::Formatter,
        ) -> Result<(), std::fmt::Error> {
            for child in parent.children() {
                if child.is_element() {
                    writeln_indented!(depth, f, "Element {{");
                    writeln_indented!(depth, f, "    tag_name: {:?}", child.tag_name());

                    if !child.attributes().is_empty() {
                        writeln_indented!(depth + 1, f, "attributes: [");
                        for attr in child.attributes() {
                            writeln_indented!(depth + 2, f, "{:?}", attr);
                        }
                        writeln_indented!(depth + 1, f, "]");
                    }

                    if child.has_children() {
                        writeln_indented!(depth, f, "    children: [");
                        print_children(child, depth + 2, f)?;
                        writeln_indented!(depth, f, "    ]");
                    }

                    writeln_indented!(depth, f, "}}");
                } else {
                    writeln_indented!(depth, f, "{:?}", child);
                }
            }

            Ok(())
        }

        writeln!(f, "Document [")?;
        print_children(self.root(), 1, f)?;
        writeln!(f, "]")?;

        Ok(())
    }
}
