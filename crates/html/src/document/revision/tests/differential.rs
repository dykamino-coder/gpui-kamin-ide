//! Правки текста знаками, которые CSS не схлопывает (U+00A0, U+3000, U+2003,
//! перевод страницы), письмом справа налево и цифрами: принятая без каскада
//! ревизия равна свежей сборке и деревом коробок, и состоянием документа.

use super::update;

/// Страница с местами правки `@N`: единственный текст коробки под
/// `filter: url()`, текст между run-in и блоком, абзац с `::first-line`,
/// строчный элемент, ячейка, `pre`, пункт списка, `dir=auto`, текст рядом с
/// блоком в анонимной обёртке.
const SITES: &str = "<!doctype html><html><head><style>.f{filter:url(#f)}\
    .r{display:run-in}p::first-line{color:#321}</style></head><body>\
    <div class=f>@0</div><span class=r>r</span>@1<div>b</div><p>@2<span>@3</span></p>\
    <table><tr><td>@4</td></tr></table><pre>@5</pre><ul><li>@6</li></ul>\
    <p dir=auto>@7</p><div><p>q</p>@8</div><p><rt>x</rt>@9<rt>y</rt></p></body></html>";

const PLACES: usize = 10;

fn page(texts: &[String]) -> String {
    let mut out = SITES.to_string();
    for (i, text) in texts.iter().enumerate() {
        out = out.replace(&format!("@{i}<"), &format!("{text}<"));
    }
    out
}

#[test]
fn text_that_is_not_collapsible_space_keeps_boxes_like_a_fresh_build() {
    // Узел из U+00A0 или U+3000 не пустой: коробка под `filter: url()` остаётся
    // с содержимым, а run-in не вбегает в следующий блок (CSS Text 3 §4.1.1).
    for space in ["\u{a0}", "\u{3000}"] {
        for (from, to) in [("1", space), (space, "1")] {
            let filtered =
                |t: &str| format!("<body><div style=\"filter:url(#f)\">{t}</div></body>");
            assert!(
                update(&filtered(from), &filtered(to), ""),
                "{from:?} -> {to:?}"
            );
            let run_in = |t: &str| {
                format!("<body><span style=\"display:run-in\">r</span>{t}<div>b</div></body>")
            };
            assert!(update(&run_in(from), &run_in(to), ""), "{from:?} -> {to:?}");
        }
    }
    // Перевод страницы между аннотациями вне руби — знак текста, а не пробел:
    // он обрывает серию анонимного руби-контейнера, как цифра.
    let ruby = |t: &str| format!("<body><p><rt>x</rt>{t}<rt>y</rt></p></body>");
    assert!(update(&ruby("1"), &ruby("\u{c}"), ""));
    assert!(update(&ruby("\u{c}"), &ruby("1"), ""));
}

#[test]
fn random_text_edits_match_a_fresh_build() {
    const ALPHABET: &[&str] = &[
        "a", "Z", "1", ".", " ", "\n", "\t", "\u{a0}", "\u{3000}", "\u{2003}", "\u{c}", "\u{5d0}",
        "\u{663}", "&nbsp;", "&#x3000;",
    ];
    // xorshift64: воспроизводимая последовательность без внешнего генератора.
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let word = |next: &mut dyn FnMut(usize) -> usize| -> String {
        (0..next(4))
            .map(|_| ALPHABET[next(ALPHABET.len())])
            .collect()
    };
    let mut adopted = 0;
    for _ in 0..160 {
        let texts: Vec<String> = (0..PLACES).map(|_| word(&mut next)).collect();
        let mut edited = texts.clone();
        for _ in 0..1 + next(2) {
            let i = next(PLACES);
            edited[i] = word(&mut next);
        }
        if edited == texts {
            continue;
        }
        if update(&page(&texts), &page(&edited), "") {
            adopted += 1;
        }
    }
    // Прогон проверяет обе ветви: часть правок принимается без каскада.
    assert!(adopted > 10, "{adopted}");
}

#[test]
fn equal_texts_are_told_apart_by_their_source() {
    // Номер исходного текста, а не содержимое, находит правленый узел.
    let page = |a: &str, b: &str| format!("<body><p>{a}<br>{b}</p><p>{a}</p></body>");
    assert!(update(&page("x", "x"), &page("x", "y"), ""));
    assert!(update(&page("x", "y"), &page("y", "y"), ""));
    // Обход склеивает тексты через комментарий: у склеенного нет номера.
    let merged = |a: &str| format!("<body><p>{a}<!-- c -->b</p></body>");
    assert!(!update(&merged("a"), &merged("c"), ""));
}
