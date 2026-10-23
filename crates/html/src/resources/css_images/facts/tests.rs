//! Снимки фактов не запускают загрузку и сохраняют состояния прежнего прохода.

use super::*;
use crate::resources::images::store::{ImageServices, ResourcePolicy};
use crate::resources::images::tests_support::{TempDir, data_png_url, png_bytes, png_sized};
use url::Url;

fn store(dir: &TempDir) -> ImageStore {
    ImageStore::new(
        ImageServices::process(),
        Some(Url::from_file_path(dir.path().join("page.html")).unwrap()),
        ResourcePolicy::default(),
    )
}

#[test]
fn pending_is_read_only_and_ready_does_not_change_held_snapshot() {
    let mut store = ImageStore::default();
    let old = Rc::new(BackgroundImages::default());
    let raw = data_png_url();
    let image = ImageSource::Url(raw.clone());
    let revision = store.snapshot().revision();
    assert_eq!(
        old.image_available(&image, true),
        ImageAvailability::Pending
    );
    assert_eq!(old.uniform_image_color(&image), None);
    assert_eq!(store.snapshot().revision(), revision);
    assert!(store.take_jobs().is_empty());
    let new = BackgroundImages::prepare(&old, &mut store, [raw]);
    assert_eq!(new.image_available(&image, true), ImageAvailability::Ready);
    assert_eq!(
        new.uniform_image_color(&image),
        Some(Color {
            r: 5.0 / 255.0,
            g: 6.0 / 255.0,
            b: 7.0 / 255.0,
            a: 1.0,
        })
    );
    assert_eq!(
        old.image_available(&image, true),
        ImageAvailability::Pending
    );
    let same = BackgroundImages::prepare(&new, &mut store, []);
    assert!(Rc::ptr_eq(&new, &same));
    assert!(store.take_jobs().is_empty());
}

#[test]
fn failed_file_can_appear_and_changed_pixels_publish_another_snapshot() {
    let mut dir = TempDir::new("background-facts-late");
    let mut store = store(&dir);
    let image = ImageSource::Url("late.png".into());
    let failed = BackgroundImages::prepare(&Rc::default(), &mut store, ["late.png".into()]);
    assert_eq!(
        failed.image_available(&image, true),
        ImageAvailability::Failed
    );
    dir.write("late.png", &png_bytes([255, 0, 0, 255]));
    assert_eq!(store.revalidate(), 2);
    let red = BackgroundImages::prepare(&failed, &mut store, []);
    assert_eq!(red.uniform_image_color(&image).unwrap().r, 1.0);
    assert_eq!(
        failed.image_available(&image, true),
        ImageAvailability::Failed
    );
    dir.write("late.png", &png_sized(2, 1, [0, 0, 255, 255]));
    assert_eq!(store.revalidate(), 2);
    let blue = BackgroundImages::prepare(&red, &mut store, []);
    assert_eq!(blue.uniform_image_color(&image).unwrap().b, 1.0);
    assert_eq!(red.uniform_image_color(&image).unwrap().r, 1.0);
}

#[test]
fn base_changes_cannot_reuse_url_facts() {
    let (mut a, mut b) = (TempDir::new("facts-base-a"), TempDir::new("facts-base-b"));
    a.write("same.png", &png_bytes([255, 0, 0, 255]));
    b.write("same.png", &png_bytes([0, 255, 0, 255]));
    let mut store = store(&a);
    let image = ImageSource::Url("same.png".into());
    let red = BackgroundImages::prepare(&Rc::default(), &mut store, ["same.png".into()]);
    store.set_base(Some(
        Url::from_file_path(b.path().join("page.html")).unwrap(),
    ));
    let green = BackgroundImages::prepare(&red, &mut store, []);
    assert_eq!(green.uniform_image_color(&image).unwrap().g, 1.0);
    assert_eq!(red.uniform_image_color(&image).unwrap().r, 1.0);
    let mut denied_store = ImageStore::new(
        ImageServices::process(),
        Some(Url::from_file_path(b.path().join("page.html")).unwrap()),
        ResourcePolicy::data_only(),
    );
    let denied = BackgroundImages::prepare(&Rc::default(), &mut denied_store, ["same.png".into()]);
    assert_eq!(
        denied.image_available(&image, true),
        ImageAvailability::Failed
    );
    assert_eq!(
        green.image_available(&image, true),
        ImageAvailability::Ready
    );
}

#[test]
fn constant_gradient_facts_follow_resolved_current_color_and_hue_arc() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = crate::style::computed::Computed::default();
    own.apply_one("color", "green", vcx);
    own.apply_one(
        "background-image",
        "linear-gradient(currentColor, currentColor)",
        vcx,
    );
    let mut image = ImageSource::parse("linear-gradient(currentColor, currentColor)", vcx).unwrap();
    image.resolve_colors(own.color.unwrap());
    let facts = Rc::new(BackgroundImages::default());
    assert_eq!(
        facts.uniform_image_color(&image),
        Color::parse("green", vcx)
    );
    let image = ImageSource::parse("linear-gradient(in hsl longer hue, red, red)", vcx).unwrap();
    assert_eq!(facts.uniform_image_color(&image), None);
}
