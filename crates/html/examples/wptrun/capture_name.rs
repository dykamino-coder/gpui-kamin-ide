//! Keep distinct pairs' evidence when basenames recur across WPT folders.

pub(super) fn stem(pair_index: usize, test: &str) -> String {
    let path = std::path::Path::new(test);
    let basename = path.file_stem().unwrap_or_default().to_string_lossy();
    format!("{:06}-{basename}", pair_index + 1)
}
