//! Монотонные ключи synthetic-коробок документа без переполнения и повторов.

use super::DocState;

impl DocState {
    pub(crate) fn next_synthetic_id(&self) -> String {
        // Десятичные цифры от младшей к старшей; пустое состояние означает ноль.
        let mut digits = self.synthetic_id.borrow_mut();
        if digits.is_empty() {
            digits.push(0);
        }
        let id = digits.iter().rev().map(|d| char::from(b'0' + d)).collect();
        for digit in digits.iter_mut() {
            if *digit < 9 {
                *digit += 1;
                return id;
            }
            *digit = 0;
        }
        digits.push(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carry_extends_the_key_without_reusing_zero() {
        let doc = DocState::new(Default::default());
        assert_eq!(doc.next_synthetic_id(), "0");
        *doc.synthetic_id.borrow_mut() = vec![9; 24];
        assert_eq!(doc.next_synthetic_id(), "9".repeat(24));
        assert_eq!(doc.next_synthetic_id(), format!("1{}", "0".repeat(24)));
    }
}
