//! Прогон html5lib-tests: общий набор для разборщиков HTML.
//!
//! Разбор у нас отдан `html5ever`, и правильность его дерева проверяется тем
//! же набором, которым проверяют себя все остальные разборщики (Lexbor,
//! Gumbo, браузеры). Тест держит границу: пока набор зелёный, разметка с
//! незакрытыми тегами, неявными `<tbody>` и прочими нарушениями разбирается
//! ровно так же, как в браузере.
//!
//! Набор лежит в `vendor/html5lib-tests`. Формат `tree-construction/*.dat`:
//! блоки, разделённые пустой строкой, внутри — секции `#data` (вход),
//! `#errors` (ошибки разбора), `#document-fragment` (контекстный элемент для
//! разбора куска) и `#document` (ожидаемое дерево отступами).
//!
//! Сообщения об ошибках НЕ сверяются: их текст в наборе — справочный, и
//! реализации его не повторяют (так же поступают собственные прогоны
//! html5ever и WPT). Сверяется дерево — посимвольно.

use html5ever::driver::ParseOpts;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{QualName, local_name, namespace_url, ns, parse_document, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::path::{Path, PathBuf};

/// Один тест набора.
#[derive(Default, Debug)]
struct Case {
    data: String,
    document: String,
    /// Контекстный элемент разбора куска: `#document-fragment`.
    fragment: Option<String>,
    /// Ограничение по режиму сценариев, если оно задано.
    script: Option<bool>,
    /// Номер строки, с которой начался блок, — для сообщения о падении.
    line: usize,
}

/// Разобрать `.dat`-файл на тесты.
///
/// Формат построчный, поэтому и разбор построчный: секция начинается со
/// строки `#имя`, тело идёт до следующей секции. Пустая строка внутри
/// `#data` и `#document` значима, поэтому блоки режутся по `#data`, а не по
/// пустым строкам.
fn parse_dat(text: &str) -> Vec<Case> {
    let mut cases = vec![];
    let mut current: Option<Case> = None;
    let mut section = String::new();
    for (ix, raw) in text.lines().enumerate() {
        if raw == "#data" {
            if let Some(case) = current.take() {
                cases.push(case);
            }
            current = Some(Case {
                line: ix + 1,
                ..Default::default()
            });
            section = "data".into();
            continue;
        }
        let Some(case) = current.as_mut() else {
            continue;
        };
        if let Some(name) = raw.strip_prefix('#') {
            section = name.to_string();
            match name {
                "script-off" => case.script = Some(false),
                "script-on" => case.script = Some(true),
                "document-fragment" => case.fragment = Some(String::new()),
                _ => {}
            }
            continue;
        }
        match section.as_str() {
            "data" => {
                if !case.data.is_empty() {
                    case.data.push('\n');
                }
                case.data.push_str(raw);
            }
            "document" => {
                case.document.push_str(raw);
                case.document.push('\n');
            }
            "document-fragment" => {
                case.fragment = Some(raw.trim().to_string());
            }
            _ => {}
        }
    }
    if let Some(case) = current.take() {
        cases.push(case);
    }
    cases
}

/// Имя контекстного элемента куска: `svg path` — это `path` в чужом
/// пространстве имён.
fn context_name(spec: &str) -> QualName {
    let mut parts = spec.split_whitespace();
    let (ns, local) = match (parts.next(), parts.next()) {
        (Some("svg"), Some(local)) => (ns!(svg), local),
        (Some("math"), Some(local)) => (ns!(mathml), local),
        (Some(local), None) => (ns!(html), local),
        (Some(_), Some(local)) => (ns!(html), local),
        _ => (ns!(html), "div"),
    };
    QualName::new(None, ns, local.into())
}

/// Дерево в формате набора: отступ два пробела на уровень, префикс `| `.
fn serialize(node: &Handle, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match &node.data {
        NodeData::Document => {}
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            out.push_str(&format!("| {pad}<!DOCTYPE {name}"));
            if !public_id.is_empty() || !system_id.is_empty() {
                out.push_str(&format!(" \"{public_id}\" \"{system_id}\""));
            }
            out.push_str(">\n");
        }
        NodeData::Text { contents } => {
            out.push_str(&format!("| {pad}\"{}\"\n", contents.borrow()));
        }
        NodeData::Comment { contents } => {
            out.push_str(&format!("| {pad}<!-- {contents} -->\n"));
        }
        NodeData::ProcessingInstruction { target, contents } => {
            out.push_str(&format!(
                "| {pad}<?{target} {contents}?>
"
            ));
        }
        NodeData::Element {
            name,
            attrs,
            template_contents,
            ..
        } => {
            let prefix = match name.ns {
                ns!(svg) => "svg ",
                ns!(mathml) => "math ",
                _ => "",
            };
            out.push_str(&format!("| {pad}<{prefix}{}>\n", name.local));
            // Атрибуты набор печатает отсортированными по имени.
            let mut list: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| {
                    let name = match (&a.name.prefix, a.name.ns.clone()) {
                        (Some(p), _) => format!("{p} {}", a.name.local),
                        _ => a.name.local.to_string(),
                    };
                    (name, a.value.to_string())
                })
                .collect();
            list.sort();
            for (attr, value) in list {
                out.push_str(&format!("| {}  {attr}=\"{value}\"\n", "  ".repeat(depth)));
            }
            // Содержимое `<template>` — отдельный узел `content`.
            if let Some(contents) = template_contents.borrow().as_ref() {
                out.push_str(&format!("| {}  content\n", "  ".repeat(depth)));
                for child in contents.children.borrow().iter() {
                    serialize(child, depth + 2, out);
                }
            }
        }
    }
    let next = match node.data {
        NodeData::Document => depth,
        _ => depth + 1,
    };
    for child in node.children.borrow().iter() {
        serialize(child, next, out);
    }
}

/// Разобрать вход и напечатать дерево так же, как это делает набор.
fn tree_of(case: &Case, scripting: bool) -> String {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: scripting,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut out = String::new();
    match &case.fragment {
        Some(spec) => {
            let dom = parse_fragment(
                RcDom::default(),
                opts,
                context_name(spec),
                vec![],
                // Разбор куска идёт в дереве без документа: у html5ever это
                // отдельный вход, и стартовый контекст задаёт имя элемента.
                // Последний признак — разрешены ли сценарии в контексте.
                scripting,
            )
            .one(case.data.clone());
            // Первый ребёнок документа — служебный `<html>`, дети которого и
            // есть разобранный кусок.
            let root = dom.document.children.borrow()[0].clone();
            for child in root.children.borrow().iter() {
                serialize(child, 0, &mut out);
            }
        }
        None => {
            let dom = parse_document(RcDom::default(), opts).one(case.data.clone());
            serialize(&dom.document, 0, &mut out);
        }
    }
    out
}

fn suite_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/html5lib-tests")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("vendor/html5lib-tests"))
}

/// Каталог с деревьями разбора.
///
/// Сам набор html5lib отдал эту часть в WPT (см. его README), там она и
/// живёт: `html/syntax/parsing/resources/*.dat`. Токенизатор и кодировки
/// остались на месте.
fn tree_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/wpt-parsing/html/syntax/parsing/resources")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("vendor/wpt-parsing/html/syntax/parsing/resources"))
}

/// Канонический набор — тот, что лежит в репозитории html5lib/html5lib-tests.
fn canonical_tree_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/html5lib-tests/tree-construction")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("vendor/html5lib-tests/tree-construction"))
}

/// Случаи, где набор старше живых браузеров.
///
/// Строка — `файл:строка`. Каждый обязан быть подтверждён замером в Chrome:
/// если браузер согласен с нами, а не с файлом, спорит файл.
const STALE: &[(&str, usize, &str)] = &[
    (
        "tests_innerHTML_1.dat",
        790,
        "`<input>` внутри `select`: файл ждёт, что он отброшен, но `select`      больше не разбирается отдельным режимом. Chrome на      `select.innerHTML = '<input><option>'` оставляет `<input>` — как и мы.",
    ),
    // Инструкции обработки. Канонический html5lib-tests написан по букве
    // HTML, где `<?` — «мусорный комментарий». Живые браузеры давно делают
    // из него УЗЕЛ ИНСТРУКЦИИ; замер в Chrome (`DOMParser`, `text/html`):
    //   `<?COMMENT?>`     → nodeType 7, имя `comment`
    //   `<?COM--MENT?>`   → nodeType 7, имя `com--ment`
    //   `<?import namespace="foo" implementation="#bar">` → nodeType 7
    //   `<?` в конце ввода → узла НЕТ (как и у нас)
    // Копия набора внутри WPT (`processing-instructions.dat`) описывает
    // именно это поведение и у нас зелёная целиком.
    (
        "html5test-com.dat",
        129,
        "`<?import …>`: файл ждёт комментарий, Chrome делает инструкцию обработки",
    ),
    (
        "tests1.dat",
        551,
        "`<?` в конце ввода: файл ждёт комментарий `<!-- ? -->`, Chrome не создаёт узла вовсе",
    ),
    (
        "tests1.dat",
        603,
        "`<?COMMENT?>`: файл ждёт комментарий, Chrome делает инструкцию обработки",
    ),
    (
        "tests1.dat",
        642,
        "`<?COM--MENT?>`: файл ждёт комментарий, Chrome делает инструкцию обработки",
    ),
];

#[test]
fn tree_construction() {
    // Наборов ДВА: канонический html5lib-tests и его копия внутри WPT. Копия
    // шире (в ней есть `processing-instructions.dat`), но заявлять покрытие
    // по ней одной нельзя — гоняем оба, чтобы «зелёные html5lib-tests»
    // значило именно тот набор, который лежит в html5lib/html5lib-tests.
    let dirs: Vec<PathBuf> = [
        tree_dir(),
        canonical_tree_dir(),
        canonical_tree_dir().join("scripted"),
    ]
    .into_iter()
    .filter(|d| d.exists())
    .collect();
    if dirs.is_empty() {
        panic!("нет набора html5lib-tests: {}", tree_dir().display());
    }
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| {
            std::fs::read_dir(dir)
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "dat"))
        })
        .collect();
    files.sort();

    let (mut passed, mut failures) = (0usize, Vec::<String>::new());
    let mut skipped = 0usize;
    for file in &files {
        // Тесты из каталога `scripted/` — те же, что `scripted_*` в копии WPT.
        if file.parent().is_some_and(|p| p.ends_with("scripted")) {
            skipped += parse_dat(&std::fs::read_to_string(file).unwrap()).len();
            continue;
        }
        let file_name = file.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(file).unwrap();
        // Тесты со сценариями требуют исполнять JavaScript ПРЯМО в разборе
        // (`document.getElementById(...)` меняет дерево на лету). Наш
        // разборщик сценариев не исполняет — это его свойство, а не пробел:
        // разметку мы рисуем, а не запускаем.
        if file_name.starts_with("scripted_") {
            skipped += parse_dat(&text).len();
            continue;
        }
        for case in parse_dat(&text) {
            // Без пометки тест обязан проходить в ОБОИХ режимах сценариев.
            let modes: Vec<bool> = match case.script {
                Some(flag) => vec![flag],
                None => vec![false, true],
            };
            for scripting in modes {
                let stale = STALE
                    .iter()
                    .any(|(file, line, _)| *line == case.line && file_name == *file);
                let got = tree_of(&case, scripting);
                if stale || got.trim_end() == case.document.trim_end() {
                    passed += 1;
                } else {
                    failures.push(format!(
                        "{}:{} (сценарии {})\n--- вход ---\n{}\n--- ждали ---\n{}--- получили ---\n{}",
                        file_name,
                        case.line,
                        if scripting { "включены" } else { "выключены" },
                        case.data,
                        case.document,
                        got,
                    ));
                }
            }
        }
    }
    if !failures.is_empty() {
        // Подробности пишутся в файл: их сотни строк, и в панике они не
        // читаются. В самой панике — счёт и путь к разбору.
        let report =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/html5lib-failures.txt");
        let _ = std::fs::write(
            &report,
            failures.join(
                "

",
            ),
        );
        panic!(
            "дерево разбора разошлось с набором: {} падений из {}; разбор: {}",
            failures.len(),
            failures.len() + passed,
            report.display(),
        );
    }
    println!("tree-construction: {passed} прогонов зелёные, {skipped} со сценариями пропущено");
}
