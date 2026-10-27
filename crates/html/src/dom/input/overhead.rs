//! Фиксированный логический overhead очередей парсера.

use html5ever::tendril::StrTendril;

// markup5ever 0.40.0, util/buffer_queue.rs: Default выделяет 16 StrTendril.
// Один входной chunk и rewind char-ref (не более двух буферов) не растят очередь.
// Во время EOF одновременно живут input, EOF и временная SIMD-очередь
// (html5ever 0.40.1, tokenizer/mod.rs: end/data_state_simd_fast_path).
// Это логический overhead; метаданные аллокатора в ledger не входят.
pub(crate) const QUEUE_OVERHEAD: usize = 3 * 16 * std::mem::size_of::<StrTendril>();
