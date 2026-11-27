//! Requests outlive a document revision (`keep`): a ready image keeps its
//! handle and state, one no revision used is freed at the next.

use super::*;
use crate::budget::RevisionBudget;
use crate::resources::images::tests_support::data_png_url;

fn ask(store: &mut ImageStore) -> ImageId {
    let (url, _) = store.resolve(&data_png_url());
    let id = store.request(ImageRequest {
        url,
        options: DecodeOptions::default(),
    });
    store.complete_pending();
    id
}

fn live(store: &ImageStore) -> usize {
    store.slots.iter().filter(|s| s.entry.is_some()).count()
}

#[test]
fn a_ready_image_keeps_its_handle_and_state_across_a_revision() {
    let mut store = ImageStore::default();
    let id = ask(&mut store);
    let read = store.image_revision(id).expect("live request");
    store.set_budget(RevisionBudget::standalone());
    assert_eq!(
        store.image_revision(id),
        Some(read),
        "a new revision changed the image"
    );
    assert_eq!(ask(&mut store), id, "the same request got a new handle");
}

#[test]
fn an_image_no_revision_uses_is_freed_at_the_next() {
    let mut store = ImageStore::default();
    let id = ask(&mut store);
    store.set_budget(RevisionBudget::standalone());
    assert_eq!(
        live(&store),
        1,
        "a request used in the previous revision was freed"
    );
    store.set_budget(RevisionBudget::standalone());
    assert_eq!(
        live(&store),
        0,
        "an unused request outlived a whole revision"
    );
    assert_eq!(store.image_revision(id), None);
}

#[test]
fn a_replayed_read_keeps_its_image() {
    let mut store = ImageStore::default();
    let at = store.log_position();
    let id = ask(&mut store);
    let reads = store.reads_since(at);
    assert_eq!(reads.len(), 1);
    for _ in 0..3 {
        store.set_budget(RevisionBudget::standalone());
        assert!(
            store.replay_reads(&reads),
            "a replay of an unchanged image was refused"
        );
        store.end_frame_log();
    }
    assert_eq!(store.image_revision(id), Some(reads[0].1));
}
