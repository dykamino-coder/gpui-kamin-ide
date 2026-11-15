//! Полная сборка ревизии: разбор и допуск источников, затем каскад в свежем
//! состоянии. Разобранный вход (`Source`) общий для полной сборки и ревизии
//! без перерасчёта стиля (`adopt`): разметка разбирается и допускается один раз.

use super::super::{Document, DocumentFailure, base_url, parse_document, state};
use std::rc::Rc;

/// Разобранная и допущенная разметка ревизии до каскада.
pub(super) struct Source {
    pub(super) input: crate::dom::input::Admitted,
    /// Действующая база документа (`<base href>` поверх адреса источника).
    pub(super) base: Option<url::Url>,
    pub(super) key: u64,
    #[cfg(feature = "engine-bench")]
    pub(super) parse: std::time::Duration,
}

impl Document {
    /// Разобрать разметку и допустить её источники в бюджете ревизии.
    pub(super) fn admit(
        html: &str,
        theme_css: &str,
        base_url: Option<&url::Url>,
        xml: bool,
        revision: &crate::budget::RevisionBudget,
    ) -> Result<Source, DocumentFailure> {
        Self::admit_in(html, theme_css, base_url, xml, revision, None)
    }

    pub(super) fn admit_in(
        html: &str,
        theme_css: &str,
        base_url: Option<&url::Url>,
        xml: bool,
        revision: &crate::budget::RevisionBudget,
        first_key: Option<u64>,
    ) -> Result<Source, DocumentFailure> {
        let key = base_url::key(html, theme_css, base_url);
        #[cfg(feature = "engine-bench")]
        let parse_started = crate::work::Clock::now();
        let input = {
            crate::work::phase!(_phase, Parse);
            let context = crate::dom::input::Context::new(revision.clone());
            let context = match first_key {
                Some(first_key) => context.in_document(first_key),
                None => context,
            };
            crate::dom::input::Admitted::with_context(html, xml, context).map_err(
                |mut failure| {
                    failure.report.revision = Some(key);
                    failure
                },
            )?
        };
        #[cfg(feature = "engine-bench")]
        let parse = parse_started.elapsed();
        let base = if xml {
            base_url.cloned()
        } else {
            base_url::document_base(
                &input.dom.document,
                base_url,
                &crate::budget::admission::Admission::new(revision.clone()),
            )?
        };
        crate::dom::early_admission::admit_sources(&input, theme_css)?;
        Ok(Source {
            input,
            base,
            key,
            #[cfg(feature = "engine-bench")]
            parse,
        })
    }

    pub(in crate::document) fn build(
        html: &str,
        theme_css: &str,
        base_url: Option<url::Url>,
        xml: bool,
        state: Rc<state::DocState>,
        fonts: &crate::text::metrics::FontContext,
        revision: crate::budget::RevisionBudget,
    ) -> Result<Self, DocumentFailure> {
        let source = Self::admit(html, theme_css, base_url.as_ref(), xml, &revision)?;
        Self::from_source(source, theme_css, base_url, xml, state, fonts, revision)
    }

    /// Каскад разобранной ревизии в состоянии `state`.
    pub(super) fn from_source(
        source: Source,
        theme_css: &str,
        base_url: Option<url::Url>,
        xml: bool,
        state: Rc<state::DocState>,
        fonts: &crate::text::metrics::FontContext,
        revision: crate::budget::RevisionBudget,
    ) -> Result<Self, DocumentFailure> {
        Self::try_source(
            source, theme_css, base_url, xml, state, fonts, revision, None,
        )
        .map_err(|(failure, _)| failure)
    }

    /// Каскад `source`; при отказе разобранный вход возвращается вызывающему
    /// (сборка из исходного DOM прежней ревизии кладёт его обратно).
    /// `invalidation`: a rebuild for attribute mutations collects the
    /// elements they restyle (`dom::convert_restyling`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_source(
        source: Source,
        theme_css: &str,
        base_url: Option<url::Url>,
        xml: bool,
        state: Rc<state::DocState>,
        fonts: &crate::text::metrics::FontContext,
        revision: crate::budget::RevisionBudget,
        invalidation: Option<&mut crate::dom::invalidation::Invalidation>,
    ) -> Result<Self, (DocumentFailure, Box<Source>)> {
        let key = source.key;
        #[cfg(feature = "engine-bench")]
        let mut timings = super::super::BuildTimings {
            parse: source.parse,
            ..Default::default()
        };
        let mut kept = crate::dom::restyle_probe::RestyleCache::default();
        let (built, media_reads) = crate::style::css::reads::record(|| {
            parse_document(
                &source.input,
                theme_css,
                source.base.as_ref(),
                state,
                fonts,
                revision,
                invalidation,
                &mut kept,
                #[cfg(feature = "engine-bench")]
                &mut timings,
            )
        });
        let (state, nodes, root) = match built {
            Ok(built) => built,
            Err(mut failure) => {
                failure.report.revision = Some(key);
                return Err((failure, Box::new(source)));
            }
        };
        // Исчерпание aggregate ревизии в ранних проходах (ресурсы, подстановки) терминально:
        // кандидат не публикуется, даже если операции вернули штатный fallback.
        if let Some(mut failure) = state.values.revision_budget().failure() {
            failure.report.revision = Some(key);
            return Err((failure, Box::new(source)));
        }
        state.values.traversal.retain(source.input);
        if state.boxes.borrow().is_empty() {
            crate::document::boxes::BoxTree::edit(&state.boxes).publish_document(
                &nodes,
                None,
                &state.values,
            );
        }
        // Документ отпускает выведенные группы стиля в Drop.
        state.values.hold_document();
        Ok(Document {
            state,
            nodes: Rc::new(nodes),
            retained: Default::default(),
            reuse: Default::default(),
            restyle: super::Kept::built(kept),
            root,
            key,
            theme_css: theme_css.into(),
            base_url,
            xml,
            last_failure: None,
            media_reads: Rc::new(media_reads),
            #[cfg(feature = "engine-bench")]
            timings,
        })
    }

    /// Разобранный вход следующей ревизии из исходного DOM этой же ревизии,
    /// в который уже записаны записи мутаций: база документа и ранний допуск
    /// источников заново (записи могли изменить `<base href>` и адреса
    /// ресурсов). При отказе вход возвращается вызывающему.
    pub(super) fn readmit(
        input: crate::dom::input::Admitted,
        theme_css: &str,
        base_url: Option<&url::Url>,
        xml: bool,
        key: u64,
        revision: &crate::budget::RevisionBudget,
    ) -> Result<Source, Box<(DocumentFailure, crate::dom::input::Admitted)>> {
        let base = if xml {
            Ok(base_url.cloned())
        } else {
            base_url::document_base(
                &input.dom.document,
                base_url,
                &crate::budget::admission::Admission::new(revision.clone()),
            )
            .map_err(DocumentFailure::from)
        };
        let admitted = base.and_then(|base| {
            crate::dom::early_admission::admit_sources(&input, theme_css)?;
            Ok(base)
        });
        match admitted {
            Ok(base) => Ok(Source {
                input,
                base,
                key,
                #[cfg(feature = "engine-bench")]
                parse: std::time::Duration::ZERO,
            }),
            Err(mut failure) => {
                failure.report.revision = Some(key);
                Err(Box::new((failure, input)))
            }
        }
    }
}
