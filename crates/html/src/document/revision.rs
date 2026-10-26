//! Транзакционная смена committed revision и постоянный исход отклонённого update.

use super::{BuildReport, Document, DocumentFailure, RevisionStatus, base_url};

impl Document {
    /// Ledger сохраняется при update и разделяется всеми views документа.
    pub fn budget(&self) -> crate::budget::DocumentBudget {
        self.revision_budget().document().clone()
    }

    /// Work текущей committed revision; subjobs используют тот же scope.
    pub fn revision_budget(&self) -> crate::budget::RevisionBudget {
        self.state
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
            self.last_failure = None;
            return Ok(BuildReport {
                revision: Some(key),
                retained_revision: Some(key),
                status: RevisionStatus::Unchanged,
                degradation: self.traversal_limit(),
            });
        }
        let media = self.state.media();
        let fonts = self.state.metrics.font_context();
        let candidate = Self::build(
            html,
            theme_css,
            self.base_url.clone(),
            self.xml,
            media,
            &fonts,
            revision,
        )
        .map_err(|mut failure| {
            failure.report.retained_revision = Some(self.key);
            self.last_failure = Some(failure);
            failure
        })?;
        *self = candidate;
        Ok(self.report())
    }

    /// Последний исход; при отказе nodes/state/salt остаются прошлой revision.
    pub fn report(&self) -> BuildReport {
        self.last_failure.map_or(
            BuildReport {
                revision: Some(self.key),
                retained_revision: Some(self.key),
                status: if self.traversal_limit().is_some() {
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
        self.last_failure
    }

    /// Минимальный постоянный сигнал для host, включая виртуализованный вывод.
    pub fn status_message(&self) -> Option<String> {
        self.last_failure
            .map(|failure| failure.to_string())
            .or_else(|| self.traversal_limit().map(|limit| limit.to_string()))
    }

    /// Полное исходное дерево этой revision, независимо от глубины представления.
    pub fn source_dom(&self) -> markup5ever_rcdom::Handle {
        self.state
            .source_dom()
            .expect("Document retains its source DOM")
    }

    pub fn traversal_limit(&self) -> Option<super::TraversalLimit> {
        self.state
            .traversal_limit()
            .or_else(|| self.state.child_traversal_limit())
    }

    /// Терминальный исход вложенного документа, обнаруженный при построении кадра.
    pub fn embedded_failure(&self) -> Option<DocumentFailure> {
        self.state.embedded_failure()
    }
}
