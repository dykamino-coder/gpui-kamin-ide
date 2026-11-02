//! Правки текста: без каскада принимаются только те, содержимое которых сборка
//! не читала (`dir=auto`/`<bdi>`, листы `<style>`), и только непустые по CSS.

use super::{PAGE, update};

#[test]
fn flow_text_edits_keep_the_cascade() {
    let edits = [
        PAGE.replace(">d<", ">e<"),
        PAGE.replace(">b<", ">bb x<"),
        PAGE.replace(">c<", ">c, d.<"),
        PAGE.replace(">a<", ">e<").replace(">d<", ">f<"),
    ];
    for new in &edits {
        assert!(update(PAGE, new, ""), "{new}");
        assert!(update(new, PAGE, ""), "{new}");
    }
    // Текст в анонимной обёртке рядом с блоком.
    let wrapped = PAGE.replace("<div><p id=x>", "<div>w<p id=x>");
    assert!(update(&wrapped, &wrapped.replace(">w<", ">v<"), ""));
}

#[test]
fn text_edits_the_cascade_can_see_rebuild_it() {
    let auto = PAGE.replace("<p>d", "<p dir=auto>d");
    let rebuilt = [
        // Пустой по CSS текст и текст листа `<style>`.
        (PAGE.to_string(), PAGE.replace(">d<", "><")),
        (PAGE.to_string(), PAGE.replace(">d<", "> <")),
        (PAGE.to_string(), PAGE.replace(">d<", ">\n\t<")),
        (
            PAGE.to_string(),
            PAGE.replace("p{color:#123}", "p{color:#124}"),
        ),
        // Текст, в котором `dir=auto` и `<bdi>` нашли первый сильный знак, и
        // текст перед ним без сильных знаков.
        (auto.clone(), auto.replace(">d<", ">e<")),
        (auto.clone(), auto.replace(">d<", ">\u{5d0}<")),
        (
            PAGE.replace("<p>d", "<p>1<bdi>d</bdi>"),
            PAGE.replace("<p>d", "<p>1<bdi>\u{627}</bdi>"),
        ),
        (
            PAGE.replace("<p>d", "<p dir=auto><span>1.</span>d"),
            PAGE.replace("<p>d", "<p dir=auto><span>\u{5d0}.</span>d"),
        ),
    ];
    for (old, new) in &rebuilt {
        assert!(!update(old, new, ""), "{new}");
    }
}

#[test]
fn text_edits_nothing_reads_keep_the_cascade() {
    // Письмо справа налево, цифры и управляющие bidi вне `dir=auto` сторону
    // элемента не меняют: её решает раскладка строк.
    for to in [
        "\u{5d0}\u{5d1}",
        "\u{627}1",
        "\u{663}",
        "a\u{202b}b\u{202c}",
        " d",
    ] {
        let new = PAGE.replace(">d<", &format!(">{to}<"));
        assert!(update(PAGE, &new, ""), "{to:?}");
        assert!(update(&new, PAGE, ""), "{to:?}");
    }
    let rtl = PAGE.replace("<p>d", "<p dir=rtl>d");
    assert!(update(&rtl, &rtl.replace(">d<", ">\u{5d0}<"), ""));
    // `dir=auto` читает текст лишь до первого сильного знака; текст после
    // него и внутри своего `dir` сторону предка не задаёт (а текст `<bdi>`
    // задаёт сторону самого `<bdi>`).
    let auto = |a: &str, b: &str, c: &str| {
        PAGE.replace(
            "<p>d",
            &format!("<p dir=auto>x <b>{a}</b><bdi>{b}</bdi><span dir=ltr>{c}</span>d"),
        )
    };
    assert!(update(&auto("y", "z", "w"), &auto("\u{5d0}", "z", "w"), ""));
    let skipped = |b: &str, c: &str| {
        PAGE.replace(
            "<p>d",
            &format!("<p dir=auto><bdi>{b}</bdi><span dir=ltr>{c}</span>d"),
        )
    };
    assert!(update(&skipped("z", "w"), &skipped("z", "\u{5d0}"), ""));
    assert!(!update(&skipped("z", "w"), &skipped("\u{5d0}", "w"), ""));
    // Первую букву режет раскладка, сборка её текст не читает.
    let lettered = PAGE.replace("p{color:#123}", "p::first-letter{color:red}");
    assert!(update(&lettered, &lettered.replace(">d<", ">e<"), ""));
}
