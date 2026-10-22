//! `text-box-trim`, `text-box-edge` and the `text-box` shorthand (css-inline-3 §4):
//! the parsed trim sides and edge metrics of a block container.
use super::*;

pub(super) fn apply(this: &mut Computed, key: &str, v: &str) {
    match key {
        // `text-box-trim` (css-inline-3 §4.2): у блочного контейнера
        // срезается ПОЛУЛИДИНГ первой и/или последней строки, чтобы край
        // содержимого сел на метрику текста. Свойство НЕ наследуется.
        "text-box-trim" => {
            if let Some((s, e)) = text_box_values::parse_trim(v) {
                this.accept_declaration();
                this.text_box_trim_start = s;
                this.text_box_trim_end = e;
            }
        }
        // `text-box-edge` (css-inline-3 §4.3): по какой метрике срезать.
        // Первое слово — верхний край, второе — нижний; при одном слове
        // второй край берёт то же значение, а если оно ему не подходит —
        // `text` (спека: «else 'text' is assumed as the missing value»).
        // `auto` = `text` (начальное `line-fit-edge: leading` читается
        // как `text`).
        "text-box-edge" => {
            if let Some((over, under)) = text_box_values::parse_edge(v) {
                this.accept_declaration();
                this.text_box_edge_set = true;
                this.text_box_over = over;
                this.text_box_under = under;
            }
        }
        // Сокращение `text-box` (css-inline-3 §4.1): без `text-box-trim`
        // подразумевается `trim-both` (НЕ начальное значение), без
        // `text-box-edge` — `auto`. `normal` гасит оба.
        "text-box" => {
            if let Some(((s, e), (over, under))) = text_box_values::parse_shorthand(v) {
                this.accept_declaration();
                this.text_box_trim_start = s;
                this.text_box_trim_end = e;
                this.text_box_over = over;
                this.text_box_under = under;
                this.text_box_edge_set = true;
            }
        }
        _ => {}
    }
}
