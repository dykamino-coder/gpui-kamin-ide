//! tokenColors contributed-темы → оверлей SyntaxColors gpui-component
//! (задача #71). Оверлей хранится глобально: `apply` вливает его в
//! highlight-тему редактора при каждом перекладе темы, а SetThemeChoice
//! чистит его при возврате на builtin-тему.

/// Оверлей SyntaxColors из tokenColors активной contributed-темы (JSON по
/// serde-ключам gpui-component). None — тема не contributed / без tokenColors:
/// редактор остаётся на builtin-подсветке. Чистится в SetThemeChoice.
static CONTRIB_SYNTAX: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);

pub fn set_contrib_syntax(v: Option<serde_json::Value>) {
    *CONTRIB_SYNTAX.lock().unwrap() = v;
}

/// Текущий оверлей (копия) для слияния в highlight-тему.
pub(super) fn contrib_syntax() -> Option<serde_json::Value> {
    CONTRIB_SYNTAX.lock().unwrap().clone()
}

/// TextMate tokenColors → оверлей SyntaxColors (задача #71). Правила идут по
/// порядку, поздние переопределяют (семантика VS Code); селектор матчится
/// ДЛИННЕЙШИМ префиксом таблицы по границам точек. fontStyle: italic/underline
/// переносим, bold нет (формат font_weight у gpui-component другой).
pub(super) fn token_colors_to_syntax(theme: &serde_json::Value) -> Option<serde_json::Value> {
    // TextMate-префикс → serde-ключи SyntaxColors gpui-component.
    const MAP: &[(&str, &[&str])] = &[
        ("comment.block.documentation", &["comment.doc"]),
        ("comment", &["comment"]),
        ("string.regexp", &["string.regex"]),
        ("constant.character.escape", &["string.escape"]),
        ("string", &["string"]),
        ("constant.numeric", &["number"]),
        ("constant.language", &["boolean", "constant"]),
        ("constant.other.symbol", &["string.special.symbol"]),
        ("constant", &["constant"]),
        ("keyword.operator", &["operator"]),
        ("keyword.control.directive", &["preproc"]),
        ("keyword", &["keyword"]),
        ("storage", &["keyword"]),
        ("entity.name.function", &["function"]),
        ("support.function", &["function"]),
        ("entity.name.type", &["type"]),
        ("entity.name.class", &["type"]),
        ("entity.other.inherited-class", &["type"]),
        ("support.type.property-name", &["property"]),
        ("support.type", &["type"]),
        ("support.class", &["type"]),
        ("entity.name.tag", &["tag"]),
        ("entity.other.attribute-name", &["attribute"]),
        ("entity.name.label", &["label"]),
        ("variable.other.property", &["property"]),
        ("support.variable.property", &["property"]),
        ("variable", &["variable"]),
        ("punctuation", &["punctuation"]),
        ("markup.heading", &["title"]),
        ("markup.italic", &["emphasis"]),
        ("markup.bold", &["emphasis.strong"]),
        ("markup.inline.raw", &["text.literal"]),
        ("markup.underline.link", &["link_uri"]),
        ("meta.preprocessor", &["preproc"]),
    ];
    let rules = theme.get("tokenColors")?.as_array()?;
    let mut out = serde_json::Map::new();
    for rule in rules {
        let Some(settings) = rule.get("settings") else {
            continue;
        };
        let fg = settings.get("foreground").and_then(|x| x.as_str());
        let fs = settings
            .get("fontStyle")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if fg.is_none() && fs.is_empty() {
            continue;
        }
        // scope: строка (возможно с запятыми) или массив строк. Правило БЕЗ
        // scope задаёт глобальный foreground редактора — не про подсветку.
        let mut scopes: Vec<String> = Vec::new();
        match rule.get("scope") {
            Some(serde_json::Value::String(s)) => {
                scopes.extend(s.split(',').map(|x| x.trim().to_string()));
            }
            Some(serde_json::Value::Array(a)) => {
                scopes.extend(a.iter().filter_map(|x| x.as_str()).map(str::to_string));
            }
            _ => continue,
        }
        for sel in &scopes {
            let mut best: Option<&(&str, &[&str])> = None;
            for e in MAP {
                let pfx = e.0;
                let is_pfx = sel == pfx
                    || (sel.starts_with(pfx) && sel.as_bytes().get(pfx.len()) == Some(&b'.'));
                if is_pfx && best.is_none_or(|b| pfx.len() > b.0.len()) {
                    best = Some(e);
                }
            }
            let Some((_, keys)) = best else { continue };
            let mut style = serde_json::Map::new();
            if let Some(c) = fg {
                style.insert("color".into(), serde_json::json!(c));
            }
            if fs.contains("italic") {
                style.insert("font_style".into(), serde_json::json!("italic"));
            } else if fs.contains("underline") {
                style.insert("font_style".into(), serde_json::json!("underline"));
            }
            if style.is_empty() {
                continue;
            }
            for k in *keys {
                out.insert((*k).to_string(), serde_json::Value::Object(style.clone()));
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn token_colors_prefix_match_order_and_shape() {
        let theme = json!({
            "tokenColors": [
                {"scope": "comment", "settings": {"foreground": "#11aa22", "fontStyle": "italic"}},
                {"scope": ["keyword.operator", "string"], "settings": {"foreground": "#334455"}},
                {"scope": "keyword, storage.type", "settings": {"foreground": "#667788"}},
                // Позднее и БОЛЕЕ специфичное правило переопределяет comment
                {"scope": "comment.line.double-slash", "settings": {"foreground": "#99aabb"}},
                // Без scope = глобальный foreground, в подсветку не идёт
                {"settings": {"foreground": "#000000"}},
                // "keywords" НЕ префикс "keyword" по границе точки
                {"scope": "keywordsomething", "settings": {"foreground": "#ff0000"}}
            ]
        });
        let v = token_colors_to_syntax(&theme).expect("overlay");
        assert_eq!(v["comment"]["color"], "#99aabb");
        assert_eq!(v["operator"]["color"], "#334455");
        assert_eq!(v["string"]["color"], "#334455");
        assert_eq!(v["keyword"]["color"], "#667788");
        assert!(v.get("function").is_none());
        // Оверлей обязан валидно десериализоваться в SyntaxColors вендора
        // (hex-строки → Hsla через Rgba).
        let _sc: gpui_component::highlighter::SyntaxColors =
            serde_json::from_value(v).expect("SyntaxColors roundtrip");
    }

    #[test]
    fn token_colors_none_without_rules() {
        assert!(token_colors_to_syntax(&json!({})).is_none());
        assert!(token_colors_to_syntax(&json!({"tokenColors": []})).is_none());
    }
}
