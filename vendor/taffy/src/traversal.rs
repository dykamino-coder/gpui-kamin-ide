//! Последовательные ленивые операции для продолжений раскладки.
pub(crate) async fn unwrap_or_else<T>(v: Option<T>, f: impl AsyncFnOnce() -> T) -> T {
    match v {
        Some(v) => v,
        None => f().await,
    }
}
pub(crate) async fn or_else<T>(v: Option<T>, f: impl AsyncFnOnce() -> Option<T>) -> Option<T> {
    match v {
        Some(v) => Some(v),
        None => f().await,
    }
}
pub(crate) async fn map<T, U>(v: Option<T>, f: impl AsyncFnOnce(T) -> U) -> Option<U> {
    match v {
        Some(v) => Some(f(v).await),
        None => None,
    }
}
pub(crate) async fn then<T>(v: bool, f: impl AsyncFnOnce() -> T) -> Option<T> {
    if v {
        Some(f().await)
    } else {
        None
    }
}
pub(crate) async fn fold<I: Iterator, A>(
    iter: I,
    mut value: A,
    mut f: impl AsyncFnMut(A, I::Item) -> A,
) -> A {
    for item in iter {
        value = f(value, item).await;
    }
    value
}
pub(crate) async fn for_each<I: Iterator>(iter: I, mut f: impl AsyncFnMut(I::Item)) {
    for item in iter {
        f(item).await;
    }
}

pub(crate) async fn max_map<I: Iterator>(
    iter: I,
    mut f: impl AsyncFnMut(I::Item) -> f32,
) -> Option<f32> {
    let mut max = None;
    for item in iter {
        let value = f(item).await;
        if max.is_none_or(|old: f32| old.total_cmp(&value).is_le()) {
            max = Some(value);
        }
    }
    max
}
