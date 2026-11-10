//! Транзакционная смена committed revision и постоянный исход отклонённого update.

mod adopt;
mod apply;
mod build;
mod commit;
mod mentions;
mod mutation;
mod patch;
mod restyle;
mod shape;
#[cfg(test)]
mod tests;
mod text;

pub use apply::ApplyError;
pub use mutation::Mutation;

use super::{BuildReport, Document, DocumentFailure, RevisionStatus, base_url};

impl Document {
    /// Ledger сохраняется при update и разделяется всеми views документа.
    pub fn budget(&self) -> crate::budget::DocumentBudget {
        self.revision_budget().document().clone()
    }

    /// Work текущей committed revision; subjobs используют тот же scope.
    pub fn revision_budget(&self) -> crate::budget::RevisionBudget {
        self.state
            .values
            .budget
            .borrow()
            .as_ref()
            .expect("Document owns an admitted revision")
            .clone()
    }

    /// Разобрать заново, только если разметка действительно изменилась.
    ///
    /// Для стриминга (текст дописывается по кусочку) это и есть главный
    /// рубеж: пока пришедший кусок не изменил разметку, дерево остаётся тем
    /// же, и кадр стоит ровно столько же, сколько нативный.
    pub fn update(&mut self, html: &str, theme_css: &str) -> Result<BuildReport, DocumentFailure> {
        let revision = self.budget().revision();
        revision.source(html, theme_css).map_err(|error| {
            let mut failure = DocumentFailure::new(super::FailureReason::Budget(error));
            failure.report.retained_revision = Some(self.key);
            self.last_failure = Some(failure);
            failure
        })?;
        let key = base_url::key(html, theme_css, self.base_url.as_ref());
        if key == self.key {
            if let Some(mut failure) = self.revision_budget().failure() {
                failure.report.revision = Some(self.key);
                return Err(failure);
            }
            self.last_failure = None;
            return Ok(BuildReport {
                revision: Some(key),
                retained_revision: Some(key),
                status: RevisionStatus::Unchanged,
                degradation: self.traversal_limit(),
            });
        }
        let record = |mut failure: DocumentFailure, this: &mut Self| {
            failure.report.retained_revision = Some(this.key);
            this.last_failure = Some(failure);
            failure
        };
        let source = match Self::admit(html, theme_css, self.base_url.as_ref(), self.xml, &revision)
        {
            Ok(source) => source,
            Err(failure) => return Err(record(failure, self)),
        };
        // Та же форма: отличие — записи мутаций прежнего DOM, путь применения
        // общий с `apply`; иначе новая разметка собирается целиком.
        let mut same_shape = false;
        let mut invalidation = None;
        if let Some(old) = self.state.values.source_dom()
            && let Some(shape) = shape::compare(&old, &source.input.dom.document)
            && let Some(batch) = mutation::Batch::new(&shape.records())
        {
            #[cfg(feature = "engine-bench")]
            let started = std::time::Instant::now();
            same_shape = true;
            invalidation = self.invalidation(&batch, theme_css);
            if let Some(plan) = self.plan(&batch, theme_css, Some(&shape.sheets), &revision) {
                batch.write();
                self.commit(
                    plan,
                    source.key,
                    &revision,
                    #[cfg(feature = "engine-bench")]
                    super::BuildTimings {
                        parse: source.parse,
                        style: started.elapsed(),
                    },
                );
                return Ok(self.report());
            }
        }
        let media = self.state.media();
        let fonts = self.state.text.metrics.font_context();
        let state = super::state::DocState::continuing(media, &self.state);
        let mut invalidation = invalidation.map(crate::dom::invalidation::Invalidation::new);
        let candidate = match Self::try_source(
            source,
            theme_css,
            self.base_url.clone(),
            self.xml,
            state,
            &fonts,
            revision,
            invalidation.as_mut(),
        ) {
            Ok(candidate) => candidate,
            Err((failure, _)) => return Err(record(failure, self)),
        };
        // The same source shape gives the same node ids (traversal counters):
        // the block records may hold for the restyled tree (`restyle`).
        let mut candidate = candidate;
        if same_shape {
            let restyled = invalidation.and_then(crate::dom::invalidation::Invalidation::finish);
            self.hand_over_reuse(&mut candidate, restyled.as_deref());
        }
        *self = candidate;
        Ok(self.report())
    }

    /// Последний исход; при отказе nodes/state/salt остаются прошлой revision.
    pub fn report(&self) -> BuildReport {
        self.last_failure().map_or(
            BuildReport {
                revision: Some(self.key),
                retained_revision: Some(self.key),
                // Отказ вложенного документа — штатный fallback своей области.
                status: if self.traversal_limit().is_some() || self.embedded_failure().is_some() {
                    RevisionStatus::Degraded
                } else {
                    RevisionStatus::Complete
                },
                degradation: self.traversal_limit(),
            },
            |failure| failure.report,
        )
    }

    /// Постоянный статус для хоста до следующего успешного обновления.
    pub fn last_failure(&self) -> Option<DocumentFailure> {
        self.last_failure.or_else(|| {
            self.revision_budget().failure().map(|mut failure| {
                failure.report.revision = Some(self.key);
                failure
            })
        })
    }

    /// Полное исходное дерево этой revision, независимо от глубины представления.
    pub fn source_dom(&self) -> markup5ever_rcdom::Handle {
        self.state
            .values
            .source_dom()
            .expect("Document retains its source DOM")
    }

    pub fn traversal_limit(&self) -> Option<super::TraversalLimit> {
        self.state
            .values
            .traversal_limit()
            .or_else(|| self.state.values.child_traversal_limit())
    }

    /// Терминальный исход вложенного документа, обнаруженный при построении кадра.
    pub fn embedded_failure(&self) -> Option<DocumentFailure> {
        self.state.values.embedded_failure()
    }
}
