//! Квоты заданий хранилища: граница очереди, отмена поколения и закрытого хранилища,
//! поздний и вытесненный итог возвращают все reservations; aggregate ревизии терминален.

use super::*;
use crate::budget::{BudgetError, BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
use crate::cache::MIB;
use crate::resources::images::cache::DecodedImageCache;
use crate::resources::images::fixtures::{data_url, png};
use crate::resources::images::types::ImageLimit;
use crate::resources::loader::SyncLoader;
use std::sync::Arc;

struct Doc {
    store: ImageStore,
    document: DocumentBudget,
    governor: ResourceGovernor,
}

fn doc(kind: BudgetKind, limit: u64) -> Doc {
    let governor = ResourceGovernor::default();
    let limits = DocumentLimits::default().lower(kind, limit).unwrap();
    let document = DocumentBudget::new(limits, governor.clone()).unwrap();
    let services = ImageServices {
        loader: Arc::new(SyncLoader),
        cache: Arc::new(DecodedImageCache::new(MIB)),
    };
    let mut store = ImageStore::new(services, None, ResourcePolicy::default());
    store.set_budget(document.revision());
    Doc {
        store,
        document,
        governor,
    }
}

fn ask(store: &mut ImageStore, w: u32) -> ImageId {
    let (url, _) = store.resolve(&data_url("image/png", &png(w, 1)));
    store.request(ImageRequest {
        url,
        options: DecodeOptions::default(),
    })
}

fn live(doc: &Doc) -> [u64; 3] {
    [BudgetKind::PendingTasks, BudgetKind::DecodedPixels, BudgetKind::LiveBytes]
        .map(|kind| doc.document.used(kind) + doc.governor.used(kind))
}

fn budget_kind(state: Option<&ImageState>) -> Option<BudgetKind> {
    match state? {
        ImageState::Failed(ImageError::LimitExceeded(ImageLimit::Budget(
            BudgetError::Exceeded(e),
        ))) => Some(e.kind),
        _ => None,
    }
}

#[test]
fn queued_jobs_reserve_pending_tasks_at_the_boundary() {
    for (limit, queued) in [(1, 1), (2, 2), (3, 2)] {
        let mut d = doc(BudgetKind::PendingTasks, limit);
        let (a, b) = (ask(&mut d.store, 1), ask(&mut d.store, 2));
        assert_eq!(d.store.queue.len(), queued);
        assert_eq!(d.document.used(BudgetKind::PendingTasks), queued as u64);
        let refused = budget_kind(d.store.snapshot().state(b));
        assert_eq!(refused, (queued == 1).then_some(BudgetKind::PendingTasks));
        d.store.complete_pending();
        assert_eq!(d.document.used(BudgetKind::PendingTasks), 0);
        assert!(matches!(d.store.snapshot().state(a), Some(ImageState::Ready { .. })));
    }
}

#[test]
fn cancelled_late_and_superseded_results_release_every_reservation() {
    let mut d = doc(BudgetKind::Documents, 1);
    let id = ask(&mut d.store, 3);
    let services = d.store.services().clone();
    // Отмена до выполнения: задание не читает и не декодирует.
    let jobs = d.store.take_jobs();
    d.store.begin_generation();
    let done = jobs[0].run(&services);
    assert!(matches!(done.result, Err(ImageError::Cancelled)));
    assert_eq!(d.store.budget().used(BudgetKind::ResourceRequests), 0);
    assert!(d.store.apply_completed(done).is_none());
    drop(jobs);
    assert_eq!(live(&d), [0, 0, 0]);
    assert!(d.store.snapshot().state(id).is_none());
    // Поздний итог: декодирован и допущен, но поколение уже сменилось.
    let id = ask(&mut d.store, 3);
    let jobs = d.store.take_jobs();
    let done = jobs[0].run(&services);
    drop(jobs);
    assert_eq!(d.document.used(BudgetKind::DecodedPixels), 3);
    d.store.begin_generation();
    assert!(d.store.apply_completed(done).is_none());
    assert_eq!(d.document.used(BudgetKind::DecodedPixels), 0);
    assert!(d.store.snapshot().state(id).is_none());
    // Вытесненная попытка того же поколения: её допуск возвращается, новый — принят.
    let id = ask(&mut d.store, 3);
    let old = d.store.take_jobs();
    d.store.slots[id.slot as usize].entry.as_mut().unwrap().content = Some(Revision::Absent);
    assert_eq!(d.store.revalidate(), 1);
    let stale = old[0].run(&services);
    drop(old);
    assert!(d.store.apply_completed(stale).is_none());
    d.store.complete_pending();
    assert_eq!(d.document.used(BudgetKind::DecodedPixels), 3);
    assert_eq!(d.document.used(BudgetKind::PendingTasks), 0);
    d.store.begin_generation();
    assert_eq!(d.document.used(BudgetKind::DecodedPixels), 0);
}

#[test]
fn closing_the_store_cancels_its_outstanding_jobs() {
    let mut d = doc(BudgetKind::Documents, 1);
    ask(&mut d.store, 2);
    let services = d.store.services().clone();
    let jobs = d.store.take_jobs();
    drop(d.store);
    assert!(matches!(jobs[0].run(&services).result, Err(ImageError::Cancelled)));
    assert_eq!(d.document.used(BudgetKind::PendingTasks), 1, "the job is still held");
    drop(jobs);
    assert_eq!(d.document.used(BudgetKind::PendingTasks), 0);
    assert_eq!(d.governor.used(BudgetKind::PendingTasks), 0);
}

#[test]
fn exhausted_resource_aggregate_fails_the_image_and_the_revision() {
    // Один PNG 1×1 в data: — несколько десятков байт; вторая картинка не помещается.
    let size = png(1, 1).len() as u64;
    let mut d = doc(BudgetKind::ResourceBytes, size + png(2, 1).len() as u64 - 1);
    let (a, b) = (ask(&mut d.store, 1), ask(&mut d.store, 2));
    d.store.complete_pending();
    assert!(matches!(d.store.snapshot().state(a), Some(ImageState::Ready { .. })));
    assert_eq!(budget_kind(d.store.snapshot().state(b)), Some(BudgetKind::ResourceBytes));
    let terminal = d.store.budget().exhausted().unwrap();
    assert!(matches!(terminal, BudgetError::Exceeded(e) if e.kind == BudgetKind::ResourceBytes));
    assert_eq!(d.store.budget().used(BudgetKind::ResourceBytes), size);
}
