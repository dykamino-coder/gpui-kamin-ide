//! @keyframes: разбор кадров и условных групп.

use crate::style::css::*;

/// Кадры анимации: доля времени и объявления на этой доле.
pub type Keyframes = Vec<(f32, Decls)>;

/// `@keyframes имя { 0% {…} 100% {…} }` — все наборы кадров таблицы.
///
/// Разбираются отдельно от правил: у `@keyframes` тело состоит не из
/// объявлений, а из вложенных блоков, и общий разборщик такое телом правила
/// не считает.
pub fn parse_keyframes(css: &str) -> HashMap<String, Keyframes> {
    parse_keyframes_in(css, None)
}

/// Лежит ли место `at` таблицы внутри группы `@media`/`@supports`, чьё
/// условие ЛОЖНО (css-conditional-3 §2: правила такой группы не действуют —
/// и `@font-face`, и `@keyframes`, а не только правила стиля:
/// `at-media-content-002/003`, `at-supports-content-002/003`).
/// Стек заголовков открытых блоков считается по скобкам, строки пропускаются.
pub(crate) fn in_false_group(css: &str, at: usize, media: Media) -> bool {
    let end = at.min(css.len());
    let b = css.as_bytes();
    let mut heads: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < end {
        match b[i] {
            q @ (b'"' | b'\'') => {
                i += 1;
                i += skip_string(&css[i..], q as char);
                continue;
            }
            b'{' => {
                heads.push((start, i));
                start = i + 1;
            }
            b'}' => {
                heads.pop();
                start = i + 1;
            }
            b';' => start = i + 1,
            _ => {}
        }
        i += 1;
    }
    heads.iter().any(|&(s, e)| {
        let head = css[s..e].trim();
        let low = head.to_ascii_lowercase();
        (low.starts_with("@media") && !media.matches(&low))
            || (low.starts_with("@supports") && !supports(&head[9..]))
    })
}

/// Наборы кадров таблицы. `media` — условия окружения: с ними кадры ложной
/// группы не берутся; `None` (лист агента) — берутся все, как прежде.
pub fn parse_keyframes_in(css: &str, media: Option<Media>) -> HashMap<String, Keyframes> {
    let cleaned = strip_comments(css);
    let mut out: HashMap<String, Keyframes> = HashMap::new();
    let mut rest = cleaned.as_str();
    while let Some(at) = rest.find("@keyframes") {
        let pos = cleaned.len() - rest.len() + at;
        let skip = media.is_some_and(|m| in_false_group(&cleaned, pos, m));
        rest = &rest[at + "@keyframes".len()..];
        let Some(brace) = rest.find('{') else { break };
        let name = rest[..brace].trim().to_string();
        let Some(close) = find_matching(&rest[brace..]) else {
            break;
        };
        let body = &rest[brace + 1..brace + close];
        rest = &rest[brace + close + 1..];
        if skip {
            continue;
        }

        let mut frames: Keyframes = vec![];
        let mut inner = body;
        while let Some(b) = inner.find('{') {
            let stops = inner[..b].trim();
            let Some(c) = find_matching(&inner[b..]) else {
                break;
            };
            // Объявление кадра с `!important` игнорируется ЦЕЛИКОМ
            // (css-animations-1 §3: «declarations in a keyframe rule that are
            // qualified with !important are ignored»). Отсекается ДО разбора:
            // `parse_decls` оставляет из двух одноимённых важное, и обычное
            // `border-color: green` того же кадра пропадало вместе с ним
            // (`important-prop`).
            let plain: Vec<&str> = split_top_level(&inner[b + 1..b + c], ';')
                .into_iter()
                .filter(|d| top_level_bang(d.trim()).is_none())
                .collect();
            let decls = parse_decls(&plain.join(";"));
            inner = &inner[b + c + 1..];
            for stop in stops.split(',') {
                let at = match stop.trim() {
                    "from" => Some(0.0),
                    "to" => Some(1.0),
                    other => other
                        .trim_end_matches('%')
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .map(|v| v / 100.0),
                };
                if let Some(at) = at {
                    frames.push((at, decls.clone()));
                }
            }
        }
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        if !frames.is_empty() {
            out.insert(name, frames);
        }
    }
    out
}

#[cfg(test)]
mod keyframe_tests {
    use super::*;

    #[test]
    fn keyframes_are_read_with_their_stops() {
        let k = parse_keyframes(
            "@keyframes pulse { from { opacity: 0 } 50% { opacity: 1 } to { opacity: 0 } }",
        );
        let frames = k.get("pulse").expect("набор кадров по имени");
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[1].0, 0.5);
        assert_eq!(frames[0].1.get("opacity").map(String::as_str), Some("0"));
    }

    #[test]
    fn a_stylesheet_without_keyframes_gives_nothing() {
        assert!(parse_keyframes(".a { color: red }").is_empty());
    }
}
