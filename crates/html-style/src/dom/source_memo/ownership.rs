//! Output containment differs from source ancestry and selector matching paths.
use super::NodeKey;

#[derive(Clone, Default)]
pub struct OutputOwnership {
    pub parent: Option<NodeKey>,
    pub children: Vec<NodeKey>,
    /// The source child sequence read by this entry, including an empty sequence.
    pub sequence: Option<NodeKey>,
}
