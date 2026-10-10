//! Cluster edges for breaking; split out to keep the owning module within 250 lines.

/// Можно ли разорвать текст ровно на этом месте — граница ли это грозди.
///
/// Смотрятся ОБЕ стороны: знак справа не должен быть продолжением
/// (огласовка, модификатор, знак-тег), а знак слева не должен быть
/// соединителем — после нулевого соединителя гроздь продолжается следующим
/// знаком (`line-breaking-014`: радужный флаг рвался по соединителю).
pub(crate) fn cluster_edge(text: &str, at: usize) -> bool {
    if at >= text.len() {
        return true;
    }
    // Огласовка ПОСЛЕ пробела ни к чему не приросла: по UAX-14 (правило LB9)
    // знак-продолжение после разделителя считается обычной буквой, и рвать
    // перед ним можно.
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    if !cluster_start(&text[at..]) {
        return false;
    }
    !matches!(text[..at].chars().next_back(), Some('\u{200d}'))
}

/// То же, но с учётом `line-break: anywhere`.
///
/// `anywhere` перекрывает класс ZWJ по css-text-4, то есть рвать РЯДОМ с
/// соединителем можно. Саму гроздь он не разбирает: огласовка, знак вариации,
/// модификатор тона и знак-тег остаются при своём знаке, иначе эмодзи-цепочка
/// рассыпается по строкам (`line-breaking-014`).
pub(super) fn cluster_edge_at(text: &str, at: usize, anywhere: bool) -> bool {
    if !anywhere {
        return cluster_edge(text, at);
    }
    if at >= text.len() {
        return true;
    }
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    let next = text[at..].chars().next();
    next == Some('\u{200d}') || before == Some('\u{200d}') || cluster_start(&text[at..])
}

/// Начинается ли с этого места ГРОЗДЬ знаков — то есть можно ли тут рвать.
///
/// Знаки-продолжения грозди: соединительная огласовка (класс CM по UAX-14),
/// нулевой соединитель и знаки вариации.
pub(super) fn cluster_start(rest: &str) -> bool {
    let Some(ch) = rest.chars().next() else {
        return true;
    };
    // Продолжения грозди: нулевой соединитель, знаки вариации, знаки-теги
    // (флаги вроде уэльского), модификаторы тона кожи. Все они принадлежат
    // предыдущему знаку и в другую строку не уходят (`line-breaking-014`).
    if matches!(
        ch as u32,
        0x200D
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
            | 0xE0020..=0xE007F
            | 0x1F3FB..=0x1F3FF
    ) {
        return false;
    }
    !matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}
