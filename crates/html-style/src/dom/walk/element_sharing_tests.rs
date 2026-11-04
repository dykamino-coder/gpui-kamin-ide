//! Разделение стиля между элементами: перепроверка структурных, соседних и
//! `:has()`/`:empty` правил, позиционные значения, кузены и общие группы.

use crate::dom::{Element, Node, parse};
use crate::style::computed::Computed;

fn collect<'a>(nodes: &'a [Node], tag: &str, out: &mut Vec<&'a Element>) {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.tag == tag {
                out.push(e);
            }
            collect(&e.children, tag, out);
        }
    }
}

/// Наблюдаемые свойства: цвет и `z-index` (сам `Computed` несёт и
/// идентичность коробки, разную у каждого элемента).
fn styles(html: &str, tag: &str) -> Vec<String> {
    let parsed = parse(html, "");
    let mut found = vec![];
    collect(&parsed.nodes, tag, &mut found);
    let observed = |c: &Computed| format!("{:?} {:?}", c.inh.ui.color, c.reset_positioned.z_index);
    found.iter().map(|e| observed(e.style.cascaded())).collect()
}

#[test]
fn structural_rules_are_rechecked_for_each_sibling() {
    let s = styles(
        "<style>.r{color:blue}.r:nth-child(2n){color:red}</style>\
         <div class=r>a</div><div class=r>b</div><div class=r>c</div><div class=r>d</div>",
        "div",
    );
    assert_eq!(s.len(), 4);
    assert_ne!(s[0], s[1], "чётный брат берёт своё правило");
    assert_eq!(s[0], s[2]);
    assert_eq!(s[1], s[3]);
}

#[test]
fn ancestor_structural_rules_are_rechecked_for_cousins() {
    let s = styles(
        "<style>tr:nth-child(2n) td{color:red}</style><table>\
         <tr><td>1</td></tr><tr><td>2</td></tr><tr><td>3</td></tr><tr><td>4</td></tr></table>",
        "td",
    );
    assert_eq!(s.len(), 4);
    assert_ne!(s[0], s[1], "кузен под чётной строкой берёт своё правило");
    assert_eq!(s[0], s[2]);
    assert_eq!(s[1], s[3]);
}

#[test]
fn sibling_combinators_empty_and_has_are_rechecked() {
    let s = styles(
        "<style>.a + p{color:red}p:empty{color:green}p:has(b){color:blue}</style>\
         <div><p>x</p><p class=a>x</p><p>x</p><p></p><p><b>x</b></p><p>x</p></div>",
        "p",
    );
    assert_eq!(s.len(), 6);
    assert_ne!(s[0], s[2], "сосед после .a");
    assert_ne!(s[0], s[3], ":empty");
    assert_ne!(s[0], s[4], ":has()");
    assert_eq!(s[0], s[5]);
}

#[test]
fn positional_values_are_not_shared() {
    let s = styles(
        "<style>div{z-index:sibling-index()}</style><div>a</div><div>b</div>",
        "div",
    );
    assert_ne!(s[0], s[1], "sibling-index() — функция места");
    let s = styles(
        "<style>section{--i:sibling-index()}div{z-index:var(--i)}</style>\
         <section><div>a</div><div>b</div></section><section><div>c</div></section>",
        "div",
    );
    assert_ne!(s[0], s[1], "переменная родителя с позиционной функцией");
    assert_eq!(s[0], s[2]);
}

#[test]
fn equal_siblings_and_cousins_share_groups() {
    // `var()` выводит каскад из кэша совпавших блоков: общая группа может
    // прийти только от разделения стиля элементов.
    let html = "<style>p{--c:red}span{color:var(--c)}</style>\
                <p><span>a</span><span>b</span></p><p><span>c</span></p>";
    let parsed = parse(html, "");
    let mut spans = vec![];
    collect(&parsed.nodes, "span", &mut spans);
    let [a, b, c] = spans[..] else {
        panic!("три span")
    };
    let group = |e: &Element| e.style.cascaded().inh.clone();
    assert!(std::rc::Rc::ptr_eq(&group(a).0, &group(b).0), "братья");
    assert!(std::rc::Rc::ptr_eq(&group(a).0, &group(c).0), "кузены");
}

#[test]
fn different_attributes_do_not_share() {
    let s = styles(
        "<style>[data-x=a]{color:red}</style><div data-x=a>a</div><div data-x=b>b</div>",
        "div",
    );
    assert_ne!(s[0], s[1]);
}

#[test]
fn several_entries_with_one_key_are_told_apart_by_their_outcomes() {
    let s = styles(
        "<style>.r:nth-child(3n+1){color:red}.r:nth-child(3n+2){color:green}\
         .r:nth-child(3n+2):hover{color:blue}</style><div>\
         <p class=r>a</p><p class=r>b</p><p class=r>c</p>\
         <p class=r>d</p><p class=r>e</p><p class=r>f</p></div>",
        "p",
    );
    assert_eq!(s.len(), 6);
    assert_ne!(s[0], s[1]);
    assert_ne!(s[1], s[2]);
    assert_ne!(s[0], s[2]);
    assert_eq!(s[0], s[3]);
    assert_eq!(s[1], s[4]);
    assert_eq!(s[2], s[5]);
}

#[test]
fn hover_layers_of_positional_rules_are_rechecked() {
    let parsed = parse(
        "<style>.r:hover{color:red}div:nth-child(2n) > .r:hover{color:blue}</style>\
         <div><p class=r>a</p></div><div><p class=r>b</p></div><div><p class=r>c</p></div>",
        "",
    );
    let mut found = vec![];
    collect(&parsed.nodes, "p", &mut found);
    let hover = |e: &Element| {
        let layer = e.hover.as_ref().expect("слой наведения");
        format!("{:?}", layer.cascaded().inh.ui.color)
    };
    assert_eq!(found.len(), 3);
    assert_ne!(hover(found[0]), hover(found[1]), "под чётным div — синий");
    assert_eq!(hover(found[0]), hover(found[2]));
}
#[test]
fn has_in_an_ancestor_compound_is_rechecked_for_cousins() {
    // WPT css/selectors/has-style-sharing-004.
    let s = styles(
        "<style>:has(> .a) .b{color:green}.b{color:purple}</style>\
         <div><span class=b></span><span></span></div>\
         <div><span class=b></span><span class=a></span></div>",
        "span",
    );
    assert_eq!(s.len(), 4);
    assert_ne!(s[0], s[2], "у второго div есть ребёнок .a");
}

#[test]
fn has_marks_two_levels_up_are_rechecked() {
    // WPT css/selectors/has-style-sharing-006.
    let s = styles(
        "<style>:has(> .a) .b .c{color:green}.c{color:purple}</style>\
         <div><span class=b><span class=c></span></span><span></span></div>\
         <div><span class=b><span class=c></span></span><span class=a></span></div>",
        "span",
    );
    let c: Vec<_> = [&s[1], &s[4]].into_iter().collect();
    assert_eq!(s.len(), 6);
    assert_ne!(c[0], c[1], "у второго div есть ребёнок .a");
}
