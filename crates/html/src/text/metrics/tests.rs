//! Тесты метрик `ch`/`ex`: запасные полкегля без щупа, масштабирование замера по кеглю и
//! изоляция щупов, семейств и `normal` между документами одного потока.

use super::*;
use crate::document::state::DocState;

#[test]
fn platform_metrics_are_isolated_between_interleaved_documents() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    let font = gpui::font("Face");
    for (doc, name, advance) in [(&a, "First", 3.0), (&b, "Second", 7.0)] {
        let fonts = FontContext::default().with_loader(move |_| Some(name.to_owned()));
        fonts.install(&doc.text, doc.values());
        doc.values.set_installed_fonts(std::rc::Rc::new(
            [name.to_ascii_lowercase()].into_iter().collect(),
        ));
        install_space_probe(move |_, _| advance, &doc.text, doc.values());
    }
    for (doc, own, other, advance) in [
        (&a, "First", "Second", 3.0),
        (&b, "Second", "First", 7.0),
        (&a, "First", "Second", 3.0),
    ] {
        let vcx = doc.values();
        assert_eq!(load_font(vec![1], &doc.text).as_deref(), Some(own));
        assert!(font_installed(own, vcx));
        assert!(!font_installed(other, vcx));
        assert_eq!(space_advance(&font, 16.0, &doc.text), Some(advance));
    }
    let fresh = DocState::new(Default::default());
    assert!(!fonts_known(fresh.values()));
    assert_eq!(load_font(vec![1], &fresh.text), None);
    assert_eq!(space_advance(&font, 16.0, &fresh.text), None);
}

#[test]
fn falls_back_to_half_the_font_size() {
    let doc = DocState::new(Default::default());
    assert_eq!(
        ch_ex_px("нет такого шрифта", 20.0, &doc.text, doc.values()),
        (10.0, 10.0)
    );
}

#[test]
fn probe_result_scales_with_the_font_size() {
    let doc = DocState::new(Default::default());
    install_probe(
        |family, size| {
            if family == "Ahem" {
                (size, size * 0.8, size, size)
            } else {
                (size * 0.5, size * 0.5, size * 1.2, size)
            }
        },
        &doc.text,
        doc.values(),
    );
    assert_eq!(
        ch_ex_px("Ahem", 20.0, &doc.text, doc.values()),
        (20.0, 16.0)
    );
    assert_eq!(
        ch_ex_px("Segoe UI", 20.0, &doc.text, doc.values()),
        (10.0, 10.0)
    );
}

#[test]
fn interleaved_documents_keep_their_family_probes_and_parts() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    for (doc, family, fraction) in [(&a, "First", 0.25), (&b, "Second", 0.75)] {
        set_doc_family(family, doc.values());
        install_probe(
            move |name, size| {
                assert_eq!(name, family);
                (fraction * size, fraction * size, size, size)
            },
            &doc.text,
            doc.values(),
        );
        install_vprobe(
            move |_, size| (fraction * size, size / 4.0, size / 2.0),
            &doc.text,
            doc.values(),
        );
    }
    for (doc, fraction) in [(&a, 0.25), (&b, 0.75), (&a, 0.25)] {
        assert_eq!(
            ch_ex_px("", 20.0, &doc.text, doc.values()).0,
            fraction * 20.0
        );
        assert_eq!(
            vmetrics_px("Face", 20.0, &doc.text, doc.values()).0,
            fraction * 20.0
        );
    }
    normal_px::register(1.2, 0.81, 0.21, &a.text);
    normal_px::register(1.2, 0.6, 0.6, &b.text);
    assert_eq!(content_px(1.2, 10.0, &a.text), Some(12.0));
    assert_eq!(content_px(1.2, 10.0, &b.text), Some(12.0));
    assert_eq!(normal_px(1.2, 4.0, &a.text), 5.0);
    assert_eq!(normal_px(1.2, 4.0, &b.text), 4.0);
    assert!(content_px(1.5, 10.0, &a.text).is_none());
}
