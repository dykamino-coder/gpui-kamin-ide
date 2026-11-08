//! Правка ревизии записями мутаций DOM (`Mutation`) — тот же путь применения,
//! что у `update` той же формы, без разбора и без сравнения деревьев.
//!
//! Пакет, который не может изменить каскад, публикует дерево коробок с копией
//! только путей к правкам (`plan`). Иначе записи пишутся в исходный DOM, и он
//! же собирается заново в свежем состоянии, как свежий документ той же
//! разметки; отказ сборки возвращает DOM и ревизию к прежним.

use super::mutation::{Batch, Mutation};
use super::{BuildReport, Document, DocumentFailure, RevisionStatus};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// Пакет записей не применён.
#[derive(Debug)]
pub enum ApplyError {
    /// Запись адресует узел не того вида (текст вместо элемента) или узел вне
    /// исходного DOM ревизии (отсоединённый, содержимое шаблона, другой документ).
    Foreign,
    /// Сборка ревизии отвергнута; DOM и ревизия остались прежними.
    Rejected(DocumentFailure),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Foreign => f.write_str("mutation record addresses a node outside the document"),
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
        let started = std::time::Instant::now();
        let root = self.source_dom();
        let batch = Batch::new(records).ok_or(ApplyError::Foreign)?;
        let attached = batch.texts.iter().map(|t| &t.node);
        let mut attached = attached.chain(batch.elements.iter().map(|e| &e.element));
        if !attached.all(|node| within(node, &root)) {
            return Err(ApplyError::Foreign);
        }
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
        let key = applied_key(self.key, records, theme_css);
        let revision = self.budget().revision();
        if let Some(nodes) = self.plan(&batch, theme_css, None, &revision) {
            batch.write();
            *self = self.adopted(
                nodes,
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
        self.rebuild(&batch, theme_css, key, revision)
            .map_err(ApplyError::Rejected)
    }

    /// Записать пакет в исходный DOM и собрать его заново в свежем состоянии.
    fn rebuild(
        &mut self,
        batch: &Batch,
        theme_css: &str,
        key: u64,
        revision: crate::budget::RevisionBudget,
    ) -> Result<BuildReport, DocumentFailure> {
        let input = self
            .state
            .values
            .traversal
            .take_source()
            .expect("Document retains its source DOM");
        batch.write();
        let refused = |this: &mut Self, mut failure: DocumentFailure, input| {
            batch.undo();
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
        match Self::try_source(
            source,
            theme_css,
            self.base_url.clone(),
            self.xml,
            crate::document::state::DocState::continuing(media, &self.state),
            &fonts,
            revision,
        ) {
            Ok(next) => {
                *self = next;
                Ok(self.report())
            }
            Err((failure, source)) => Err(refused(self, failure, source.input)),
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

/// Узел стоит в исходном DOM `root` (цепочка родителей доходит до него).
fn within(node: &Handle, root: &Handle) -> bool {
    let mut at = node.clone();
    loop {
        if Rc::ptr_eq(&at, root) {
            return true;
        }
        let weak = at.parent.take();
        let parent = weak
            .as_ref()
            .and_then(markup5ever_rcdom::WeakHandle::upgrade);
        at.parent.set(weak);
        match parent {
            Some(parent) => at = parent,
            None => return false,
        }
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
