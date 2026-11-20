//! Правка ревизии записями мутаций DOM (`Mutation`) — тот же путь применения,
//! что у `update` той же формы, без разбора и без сравнения деревьев.
//!
//! Пакет, который не может изменить каскад, публикует дерево коробок с копией
//! только путей к правкам (`plan`). Иначе записи пишутся в исходный DOM, и он
//! же собирается заново в свежем состоянии, как свежий документ той же
//! разметки; отказ сборки возвращает DOM и ревизию к прежним.

use super::mutation::{Batch, Mutation};
use super::{BuildReport, Document, DocumentFailure, RevisionStatus};
use std::rc::Rc;

/// Пакет записей не применён.
#[derive(Debug)]
pub enum ApplyError {
    /// Запись адресует узел не того вида (текст вместо элемента) или узел вне
    /// исходного DOM ревизии (отсоединённый, содержимое шаблона, другой документ).
    Foreign,
    /// The named node is not a current direct child of this parent.
    NotChild,
    /// The new edge would form a cycle or violate the document's child constraints.
    Hierarchy,
    /// Сборка ревизии отвергнута; DOM и ревизия остались прежними.
    Rejected(DocumentFailure),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Foreign => f.write_str("mutation record addresses a node outside the document"),
            Self::Hierarchy => f.write_str("invalid DOM hierarchy"),
            Self::NotChild => f.write_str("mutation record does not name a direct child"),
            Self::Rejected(failure) => std::fmt::Display::fmt(failure, f),
        }
    }
}

impl std::error::Error for ApplyError {}

impl Document {
    /// Применить записи мутаций к исходному DOM ревизии (`source_dom`).
    pub fn apply(
        &mut self,
        records: &[Mutation],
        theme_css: &str,
    ) -> Result<BuildReport, ApplyError> {
        #[cfg(feature = "engine-bench")]
        let started = crate::work::Clock::now();
        let root = self.source_dom();
        let revision = std::cell::OnceCell::new();
        let mut batch = Batch::in_document(records, &root, || {
            let budget = revision.get_or_init(|| self.budget().revision());
            self.state
                .values
                .traversal
                .source()
                .context()
                .continuing(budget.clone())
        })?;
        if batch.is_empty() && *self.theme_css == *theme_css {
            // Как `update` той же разметки: терминальный отказ ревизии остаётся.
            if let Some(mut failure) = self.revision_budget().failure() {
                failure.report.revision = Some(self.key);
                return Err(ApplyError::Rejected(failure));
            }
            self.last_failure = None;
            return Ok(BuildReport {
                revision: Some(self.key),
                retained_revision: Some(self.key),
                status: RevisionStatus::Unchanged,
                degradation: self.traversal_limit(),
            });
        }
        let revision = revision
            .into_inner()
            .unwrap_or_else(|| self.budget().revision());
        let key = applied_key(self.key, records, theme_css);
        // Read from the source DOM before it is taken and the batch written.
        // Child membership can change sibling selectors, inherited values and formatting.
        let invalidation = if batch.changes_children() {
            None
        } else {
            let invalidation = self.invalidation(&batch, theme_css);
            if let Some(plan) = self.plan(
                &mut batch,
                theme_css,
                None,
                invalidation.as_ref(),
                &revision,
            ) {
                batch.write();
                self.commit(
                    plan,
                    key,
                    &revision,
                    #[cfg(feature = "engine-bench")]
                    super::super::BuildTimings {
                        parse: std::time::Duration::ZERO,
                        style: started.elapsed(),
                    },
                );
                return Ok(self.report());
            }
            invalidation
        };
        crate::paint_path::add(&crate::paint_path::REFUSED_REBUILT, 1);
        self.rebuild(&mut batch, invalidation, theme_css, key, revision)
            .map_err(ApplyError::Rejected)
    }

    /// Записать пакет в исходный DOM и собрать его заново в свежем состоянии.
    pub(super) fn rebuild(
        &mut self,
        batch: &mut Batch,
        invalidation: Option<crate::dom::invalidation::Changed>,
        theme_css: &str,
        key: u64,
        revision: crate::budget::RevisionBudget,
    ) -> Result<BuildReport, DocumentFailure> {
        let mut input = self
            .state
            .values
            .traversal
            .take_source()
            .expect("Document retains its source DOM");
        // Узлы остаются прежними, но допуск источников и каскад принадлежат
        // новой ревизии. Отказ кандидата не должен отравлять прежний ledger.
        let next_context = input.context().continuing(revision.clone());
        let context = input.replace_context(next_context);
        let agent_css = input.agent_css.get();
        batch.write();
        let mut refused = |this: &mut Self,
                           mut failure: DocumentFailure,
                           mut input: crate::dom::input::Admitted| {
            batch.undo();
            input.replace_context(context.clone());
            input.agent_css.set(agent_css);
            this.state.values.traversal.retain(input);
            failure.report.retained_revision = Some(this.key);
            this.last_failure = Some(failure);
            failure
        };
        let source = match Self::readmit(
            input,
            theme_css,
            self.base_url.as_ref(),
            self.xml,
            key,
            &revision,
        ) {
            Ok(source) => source,
            Err(refusal) => {
                let (failure, input) = *refusal;
                return Err(refused(self, failure, input));
            }
        };
        let media = self.state.media();
        let fonts = self.state.text.metrics.font_context();
        let state = crate::document::state::DocState::continuing(media, &self.state);
        let mut invalidation = invalidation.map(crate::dom::invalidation::Invalidation::new);
        // The records go to the rebuilt document (`restyle`); their sources
        // are not needed for that.
        self.reuse.release_sources();
        match Self::try_source(
            source,
            theme_css,
            self.base_url.clone(),
            self.xml,
            state,
            &fonts,
            revision,
            invalidation.as_mut(),
        ) {
            Ok(mut next) => {
                // Same source DOM, same node ids: the block records may hold
                // for the restyled tree (`restyle`).
                let restyled = invalidation
                    .as_ref()
                    .and_then(crate::dom::invalidation::Invalidation::finish);
                self.hand_over_reuse(&mut next, restyled);
                *self = next;
                Ok(self.report())
            }
            Err((failure, source)) => {
                self.reuse.forget();
                Err(refused(self, failure, source.input))
            }
        }
    }

    /// Записи, переводящие исходный DOM ревизии в разметку `html` той же
    /// формы (атрибуты и тексты), адресованные узлам `source_dom`; `None` —
    /// форма другая (дети, комментарии) или разметка не разобрана.
    pub fn mutations(&self, html: &str) -> Option<Vec<Mutation>> {
        let parsed =
            crate::dom::input::Admitted::with_budget(html, self.xml, self.budget().revision())
                .ok()?;
        let shape = super::shape::compare(&self.source_dom(), &parsed.dom.document)?;
        Some(shape.records())
    }
}

/// Ключ ревизии после записей: прежний ключ, тема и каждая запись (адрес
/// узла и новое значение).
fn applied_key(previous: u64, records: &[Mutation], theme_css: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    previous.hash(&mut hash);
    theme_css.hash(&mut hash);
    for record in records {
        match record {
            Mutation::InsertBefore {
                parent,
                child,
                before,
            } => {
                (
                    3u8,
                    parent.handle().key(),
                    child.handle().key(),
                    before.as_ref().map(|n| n.key()),
                )
                    .hash(&mut hash);
            }
            Mutation::RemoveChild { parent, child } => {
                (2u8, parent.key(), child.key()).hash(&mut hash);
            }
            Mutation::CharacterData { node, data } => {
                (0u8, Rc::as_ptr(node) as usize, data).hash(&mut hash);
            }
            Mutation::Attribute {
                element,
                name,
                value,
            } => {
                let (ns, local) = (&*name.ns, &*name.local);
                (1u8, Rc::as_ptr(element) as usize, ns, local, value).hash(&mut hash);
            }
        }
    }
    hash.finish()
}
