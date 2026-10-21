//! Отрисовка строк абзаца сплошным набором: кусок строки в видимом порядке
//! bidi, знаки обрыва и переноса, выделение мышью. Здесь память выделения
//! между кадрами (`Selection`: якорь и голова в байтах текста).

mod line;
mod marker;
mod selection;

pub(super) use crate::interactive::selection::Selection;
