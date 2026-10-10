//! Выбор статичного лоадера: reduced motion не создаёт даже AnimationElement.

pub(super) fn select<T, U>(
    reduced: bool,
    element: T,
    static_render: impl FnOnce(T) -> U,
    animate: impl FnOnce(T) -> U,
) -> U {
    if reduced {
        static_render(element)
    } else {
        animate(element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reduced_mode_never_constructs_decorative_animation() {
        assert_eq!(
            select(true, "Loading…", |e| e, |_| panic!("animation installed")),
            "Loading…"
        );
    }
    #[test]
    fn normal_mode_preserves_animated_builder() {
        assert_eq!(select(false, 1, |_| panic!("static path"), |e| e + 1), 2);
    }
}
