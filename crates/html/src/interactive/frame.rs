//! Сброс состояния кадра.
// owner: A

use crate::text::vertical::VT_FRAME;

pub fn frame_sanitize(vcx: crate::document::state::ValueCx<'_>) {
    // Рамка повёрнутого абзаца не должна пережить страницу: сброс при смене
    // документа приходит позже начала следующего рендера, и щупы
    // горизонтальных абсолютов считались повёрнутыми (`css-sizing/aspect-
    // ratio/abspos-014..021`: 0.00 в одиночку и 2.36 в пачке).
    VT_FRAME.with(|c| c.set(None));
    vcx.doc.vertical_measures.begin_frame();
    // Реестр якорей — расходник кадра того же рода: пишется на подготовке,
    // читается там же, к следующей сборке дерева обязан быть пуст.
    crate::layout::positioned::anchor::reset();
}
