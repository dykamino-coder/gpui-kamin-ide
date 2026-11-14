//! `Computed::has_viewport_lengths` только отмечает: идёт по ссылкам и не зовёт аллокатор.

use kamin_style::style::computed::{BoxStyle, Computed};
use kamin_style::values::{DocumentValues, ValueCx};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

// Счётчик только тестового потока: вызовы аллокатора, пока он включён.
std::thread_local! {
    static CALLS: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = CALLS.try_with(|calls| calls.set(calls.get().map(|count| count + 1)));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Стиль после наследования: шрифтовые длины уже вычислены, единицы окна — нет.
fn styled(declarations: &[(&str, &str)], vcx: ValueCx<'_>) -> Computed {
    let mut own = Computed::default();
    for (property, value) in declarations {
        own.apply_one(property, value, vcx);
    }
    let inherited = kamin_style::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    Computed::clone(&inherited)
}

#[test]
fn the_probe_never_calls_the_allocator() {
    let doc = DocumentValues::detached(kamin_style::style::css::Media::default());
    let vcx = doc.values();
    let gradients =
        "linear-gradient(90deg, #def, #fff), radial-gradient(circle 5px at 1px 2%, red, blue)";
    for (declarations, expected) in [
        (
            &[("background-image", "linear-gradient(90deg, #def, #fff)")][..],
            false,
        ),
        (
            &[("background-image", "linear-gradient(red 10px, blue 50%)")],
            false,
        ),
        (
            &[("background-image", "linear-gradient(red 1lh, blue 50%)")],
            false,
        ),
        (&[("background", gradients)], false),
        (
            &[(
                "background",
                "url(a.svg) 5px 10% / 20px auto, conic-gradient(red, blue)",
            )],
            false,
        ),
        (
            &[("background-image", "filter(url(a.png), blur(2px))")],
            false,
        ),
        (
            &[("mask-image", "linear-gradient(red, blue), url(a.svg)")],
            false,
        ),
        (&[("content", "url(image.svg)")], false),
        (&[("border-image", "linear-gradient(red, blue) 30")], false),
        (
            &[
                ("text-decoration", "underline"),
                ("text-decoration-inset", "2px 5px"),
            ],
            false,
        ),
        (
            &[("border-radius", "10px / 20px"), ("gap", "4px 1em")],
            false,
        ),
        (
            &[("background-image", "linear-gradient(red 10vw, blue)")],
            true,
        ),
        (
            &[(
                "background-image",
                "radial-gradient(circle 5vw at 1px 2%, red, blue)",
            )],
            true,
        ),
        (
            &[(
                "background",
                "linear-gradient(red, blue), url(a.svg) 5vw 10%",
            )],
            true,
        ),
        (
            &[(
                "background-image",
                "filter(url(a.png), blur(calc(1px + 2vw)))",
            )],
            true,
        ),
        (
            &[("mask-image", "url(a.svg), linear-gradient(red 10vw, blue)")],
            true,
        ),
        (&[("text-decoration-inset", "1vw")], true),
        (&[("width", "50vw")], true),
    ] {
        let style = styled(declarations, vcx);
        CALLS.set(Some(0));
        let has = style.has_viewport_lengths(vcx);
        let calls = CALLS.replace(None);
        assert_eq!((has, calls), (expected, Some(0)), "{declarations:?}");
    }
}
