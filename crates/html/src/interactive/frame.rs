//! Сброс состояния кадра.
// owner: A

pub fn frame_sanitize(vcx: crate::document::state::ValueCx<'_>) {
    vcx.doc.vertical_measures.begin_frame();
    // Реестр якорей — расходник кадра того же рода: пишется на подготовке,
    // читается там же, к следующей сборке дерева обязан быть пуст.
    crate::layout::positioned::anchor::reset();
}
