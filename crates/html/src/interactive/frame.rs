//! Сброс состояния документа, живущего один кадр: счётчики вхождений вертикальных замеров
//! (`ParagraphState::begin_frame`) и реестры якорей (`anchor::reset`: реестры кадра
//! очищаются вместе с содержащими блоками). Вызывается в начале сборки
//! дерева (`render`, постраничная раскладка), чтобы кадр не наследовал счётчики и реестры.

pub fn frame_sanitize(cx: &crate::layout::cx::LayoutCx) {
    // Ключи вхождений локальны этой сборке, в том числе render_block.
    cx.paragraph().vertical.begin_frame();
    // Document начинает текстовый кадр до передачи параметров в render.
    // Повторная смена поколения потеряла бы данные предыдущего кадра.
    if !cx.contracts.paragraph_frame_started {
        cx.paragraph().paragraphs.begin_frame();
    }
    // Реестр якорей — расходник кадра того же рода: пишется на подготовке,
    // читается там же, к следующей сборке дерева обязан быть пуст.
    crate::layout::positioned::anchor::reset(&cx.layout().anchors);
}
