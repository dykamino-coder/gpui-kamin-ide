//! Стили счётчиков: позиционные восточноазиатские по алгоритму группировки
//! четырьмя разрядами (CSS Counter Styles 3 §7.1.5) — пропуск единицы перед
//! маркером, внутренние нули, корейские пробелы между группами; за пределом
//! диапазона — `cjk-decimal`.

pub(super) use super::cjk_data::CJK_STYLES;
use super::cjk_data::Cjk;
use super::{CJK_DIGITS, numeric};

fn group(n: usize, t: &Cjk, extended: bool) -> String {
    if n == 0 {
        return t.digits[0].to_string();
    }
    let mut slot: [Option<char>; 7] = [None; 7];
    let ones = n % 10;
    if ones != 0 {
        slot[6] = Some(t.digits[ones]);
    }
    // Ноль пишется знаком нуля только ВНУТРИ числа: пока справа одни нули,
    // писать нечего («drop any trailing zeros»).
    let mut trailing_zero = ones == 0;
    let mut div = 10;
    for (step, threshold) in [9usize, 99, 999].into_iter().enumerate() {
        if n <= threshold {
            break;
        }
        let digit = n / div % 10;
        div *= 10;
        let at = 4 - step * 2;
        if digit == 0 {
            if t.inner_zero && !trailing_zero {
                slot[at] = Some(t.digits[0]);
            }
        } else {
            if !(t.drop_one && digit == 1 && !(extended && !t.korean && step == 2)) {
                slot[at] = Some(t.digits[digit]);
            }
            slot[at + 1] = Some(t.markers[step]);
        }
        trailing_zero &= digit == 0;
    }
    if t.drop_teen && n < 20 {
        slot[4] = None;
    }
    // Подряд идущие нули схлопываются в один. Хвостового нуля тут быть не
    // может: знак нуля ставится, лишь когда правее уже есть ненулевая цифра.
    let mut out = String::new();
    let mut was_zero = false;
    for sign in slot.into_iter().flatten() {
        let zero = sign == t.digits[0];
        if !(zero && was_zero) {
            out.push(sign);
        }
        was_zero = zero;
    }
    out
}

pub(super) fn cjk_positional(n: usize, t: &Cjk) -> String {
    if n == 0 {
        return t.digits[0].to_string();
    }
    let mut groups = [0; 4];
    let mut rest = n;
    for value in &mut groups {
        *value = rest % 10_000;
        rest /= 10_000;
    }
    let mut out = String::new();
    let mut skipped = false;
    for i in (0..4).rev() {
        let value = groups[i];
        if value == 0 {
            skipped |= !out.is_empty();
            continue;
        }
        if !out.is_empty() {
            if t.korean {
                out.push(' ');
            } else if t.inner_zero && (skipped || value < 1000) {
                out.push(t.digits[0]);
            }
        }
        let first_extended = out.is_empty() && i > 0;
        if !(t.korean && t.drop_one && i == 1 && value == 1) {
            out.push_str(&group(value, t, first_extended));
        }
        out.push_str(t.groups[i]);
        skipped = false;
    }
    out
}

pub(super) fn cjk_repr(value: i64, t: &Cjk) -> String {
    if value.unsigned_abs() > 9_999_999_999_999_999 {
        // The extended algorithm specifies cjk-decimal outside its range.
        return if value < 0 {
            value.to_string()
        } else {
            numeric(value, &CJK_DIGITS)
        };
    }
    let body = cjk_positional(value.unsigned_abs() as usize, t);
    if value < 0 {
        format!("{}{body}", t.negative)
    } else {
        body
    }
}
