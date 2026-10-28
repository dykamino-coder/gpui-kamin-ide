//! Проверка компонентов размещения сетки до записи в Computed.

pub(in crate::style::computed::props::grid) fn placement_list(v: &str, max: usize) -> bool {
    let parts: Vec<_> = v.split('/').collect();
    parts.len() <= max
        && parts.iter().all(|p| {
            if p.trim() == "auto" {
                return true;
            }
            let (mut span, mut number, mut name) = (false, false, false);
            for word in p.split_whitespace() {
                if word == "span" {
                    if span {
                        return false;
                    }
                    span = true;
                } else if let Ok(n) = word.parse::<i32>() {
                    if number || n == 0 || (span && n < 0) {
                        return false;
                    }
                    number = true;
                } else if crate::style::css::selector_tokens::ident(word) && word != "auto" {
                    if name {
                        return false;
                    }
                    name = true;
                } else {
                    return false;
                }
            }
            number || name
        })
}
