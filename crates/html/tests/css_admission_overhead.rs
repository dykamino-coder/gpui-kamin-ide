//! Измерение аллокаций допущенных CSS/URL операций на реальной сборке Document.

#![allow(
    clippy::print_stdout,
    reason = "measurement process entry point emits tabular allocation evidence"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "measurement process entry point reads its optional corpus manifest from the environment"
)]

use kamin_html::{Document, budget::BudgetKind};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Measured;
// Только измеритель тестового потока; cumulative bytes включают временные копии.
std::thread_local! {
    static ALLOCATED: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for Measured {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let _ = ALLOCATED.try_with(|cell| {
                if let Some(bytes) = cell.get() {
                    cell.set(Some(bytes + layout.size()));
                }
            });
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: Measured = Measured;

fn measure(name: &str, html: &str, css: &str, path: Option<&std::path::Path>) {
    let mut builder = Document::builder(html, css);
    if let Some(path) = path {
        builder.path(path);
    }
    ALLOCATED.set(Some(0));
    let result = builder.build();
    let bytes = ALLOCATED.replace(None).unwrap();
    let document = match result {
        Ok(document) => document,
        Err(error) => {
            println!("REFUSED\t{name}\t{bytes}\t{error}");
            return;
        }
    };
    let revision = document.revision_budget();
    let kinds = [
        BudgetKind::CssBytes,
        BudgetKind::CssTokens,
        BudgetKind::CssRules,
        BudgetKind::CssSelectors,
        BudgetKind::CssComponents,
        BudgetKind::CssDeclarations,
        BudgetKind::CssDepth,
        BudgetKind::UrlBytes,
    ];
    print!("MEASURE\t{name}\t{bytes}");
    for kind in kinds {
        print!("\t{}", revision.used(kind));
    }
    println!();
}

#[test]
fn actual_document_allocations_are_measured_against_admitted_operations() {
    if let Ok(manifest) = std::env::var("KAMINIDE_CSS_CENSUS_MANIFEST") {
        for entry in std::fs::read_to_string(manifest).unwrap().lines() {
            let path = std::path::Path::new(entry);
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let source = String::from_utf8_lossy(&bytes);
            if path.extension().is_some_and(|extension| extension == "css") {
                measure(entry, "<p/>", &source, None);
            } else {
                measure(entry, &source, "", Some(path));
            }
        }
    } else {
        measure("fixed", "<p/>", "", None);
        for count in [1, 32, 1024] {
            let css = (0..count)
                .map(|i| format!(".item{i}, #id{i} {{color:red;margin:0}}"))
                .collect::<String>();
            measure(&format!("rules-{count}"), "<p/>", &css, None);
        }
        let unicode = "я".repeat(256);
        let uri = format!("https://пример.рф/{unicode}");
        ALLOCATED.set(Some(0));
        let document = Document::builder("<p/>", "")
            .base_uri(&uri)
            .build()
            .unwrap();
        let bytes = ALLOCATED.replace(None).unwrap();
        println!(
            "URL\t{bytes}\t{}",
            document.revision_budget().used(BudgetKind::UrlBytes)
        );
    }
}
