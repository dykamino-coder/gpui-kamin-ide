//! Отрисовка позиционированных коробок на их месте в дереве (CSS 2.1 Appendix E): половина
//! краски, отделённая от половины раскладки у содержащего блока.

mod source_paint;
pub(crate) use source_paint::split as split_source_paint;

mod deferred;
pub(crate) use deferred::source as deferred_source;
