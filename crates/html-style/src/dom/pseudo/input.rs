//! Проверка синтаксиса @property и адресов content-картинок.

use crate::values::ValueCx;

/// Подходит ли значение под синтаксис `@property` (css-properties-values-api-1
/// §5). Проверяются однозначные типы; значение с `var()` решается позже и
/// принимается; незнакомый синтаксис — тоже (лучше принять, чем потерять).
pub(in crate::dom) fn syntax_accepts(syntax: &str, value: &str, vcx: ValueCx<'_>) -> bool {
    let v = value.trim();
    if v.contains("var(") || syntax.trim() == "*" {
        return true;
    }
    let one = |ty: &str| -> bool {
        match ty.trim() {
            "<color>" => {
                crate::style::values::value::Color::parse(v, vcx).is_some()
                    || v.eq_ignore_ascii_case("currentcolor")
                    || v.to_ascii_lowercase().starts_with("light-dark(")
            }
            "<length>" => {
                !v.ends_with('%')
                    && (v == "0"
                        || matches!(crate::style::values::value::Len::parse_mixed(v, vcx), Some(l) if !matches!(l, crate::style::values::value::Len::Pct(_) | crate::style::values::value::Len::Auto)))
            }
            "<length-percentage>" => crate::style::values::value::Len::parse_mixed(v, vcx)
                .is_some_and(|l| l != crate::style::values::value::Len::Auto),
            "<percentage>" => v.ends_with('%') && v[..v.len() - 1].trim().parse::<f32>().is_ok(),
            "<number>" => v.parse::<f32>().is_ok(),
            "<integer>" => v.parse::<i64>().is_ok(),
            _ => true,
        }
    };
    syntax.split('|').any(one)
}

/// Адрес картинки из `content: url()` в форме `src` для `<img>`: загрузчик
/// ждёт `file:///` с прямыми косыми (как пишет стенд для `<img src>`), а
/// разбор стиля отдаёт голый путь. `None` — файла нет: такая картинка коробки
/// не даёт (Servo `components/layout/replaced.rs:348`).
pub fn content_image_src(image: &crate::style::computed::ImageSource) -> Option<String> {
    use crate::style::computed::ImageSource;
    let src = match image {
        ImageSource::Url(src) => src,
        ImageSource::Filter { image, .. } => return content_image_src(image),
        _ => return None,
    };
    if src.starts_with("data:") {
        return Some(src.to_string());
    }
    let path = src.trim_start_matches("file:///");
    if !std::path::Path::new(path).is_file() {
        return None;
    }
    Some(format!("file:///{path}").replace('\\', "/"))
}
