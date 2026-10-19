//! External instance viewport, cascade isolation and recursive resource contracts.
use super::{doc, pixel};
use std::path::PathBuf;

struct Resource(PathBuf);
impl Resource {
    fn new(text: &str) -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("kamin-use-{}-{stamp}.svg", std::process::id()));
        std::fs::write(&path, text).unwrap();
        Self(path)
    }
    fn href(&self) -> String {
        url::Url::from_file_path(&self.0).unwrap().to_string()
    }
}
impl Drop for Resource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn external_symbol_has_own_cascade_and_instance_viewport() {
    let resource = Resource::new(
        r##"<svg xmlns="http://www.w3.org/2000/svg">
      <style>.source {fill:lime}</style>
      <symbol id="icon" viewBox="0 0 10 10"><rect class="source" width="5" height="10"/>
      <rect x="5" width="5" height="10"/></symbol></svg>"##,
    );
    let markup = doc(&format!(
        r##"<style>rect {{fill:red}}</style>
      <use href="{}#icon" fill="blue" width="100" height="100"/>"##,
        resource.href()
    ));
    assert_eq!(pixel(&markup, 200.0, 100.0, 25.0, 50.0), [0, 255, 0, 255]);
    assert_eq!(pixel(&markup, 200.0, 100.0, 75.0, 50.0), [255, 0, 0, 255]);
    assert_eq!(pixel(&markup, 200.0, 100.0, 125.0, 50.0)[3], 0);
}

#[test]
fn external_cycle_preserves_other_content_and_nested_relative_use() {
    let resource = Resource::new("");
    let name = resource.0.file_name().unwrap().to_str().unwrap();
    std::fs::write(
        &resource.0,
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg">
      <g id="cycle"><use href="{name}#cycle"/><rect width="20" height="20" fill="lime"/></g>
      <symbol id="icon"><use href="{name}#cycle"/></symbol></svg>"##
        ),
    )
    .unwrap();
    let markup = doc(&format!(
        r##"<use href="{}#icon"/><rect x="100" width="50" height="50" fill="red"/>"##,
        resource.href()
    ));
    assert_eq!(pixel(&markup, 200.0, 100.0, 10.0, 10.0), [0, 255, 0, 255]);
    assert_eq!(pixel(&markup, 200.0, 100.0, 120.0, 20.0), [0, 0, 255, 255]);
}
