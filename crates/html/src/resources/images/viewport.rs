//! Конкретная область просмотра SVG-картинки (CSS Images 3 §4.3.2).

/// Задать рисунку область просмотра размером с плитку.
///
/// Область просмотра фонового рисунка — это его ПЛИТКА, а не что-то своё:
/// доли внутри рисунка (`height="50%"`, `<rect width="100%">`) считаются от
/// неё. Растеризатор же разбирает разметку как отдельный документ, и рисунок
/// с долевым размером корня не имеет для него размера вовсе — выходил пустой
/// растр, а с ним и пустая страница (вся папка `background-size/vector`).
/// Поэтому свои `width`/`height` корня заменяются размером плитки.
pub(crate) fn with_viewport(markup: &str, tile: (f32, f32)) -> String {
    let Some(open) = markup.find("<svg") else {
        return markup.to_string();
    };
    let Some(close) = markup[open..].find('>').map(|e| open + e) else {
        return markup.to_string();
    };
    let mut head = markup[open + 4..close].to_string();
    let mut own = [None::<String>, None::<String>];
    for (i, name) in ["width", "height"].iter().enumerate() {
        while let Some(at) = head.find(&format!("{name}=")) {
            let rest = &head[at + name.len() + 1..];
            let Some(quote) = rest.chars().next() else {
                break;
            };
            let Some(end) = rest[1..].find(quote) else {
                break;
            };
            own[i] = Some(rest[1..1 + end].to_string());
            head.replace_range(at..at + name.len() + 2 + end + 1, "");
        }
    }
    // Без viewBox содержимое НЕ растёт под новый вьюпорт: рисунок 50x50 в
    // плитке 100x100 занимал четверть, а маска-плитка выходила с прозрачными
    // полосами (mask-repeat-1, mask-size-cover). Свои размеры рута становятся
    // рамкой просмотра — содержимое масштабируется, как в браузере.
    if !head.contains("viewBox")
        && let (Some(w), Some(h)) = (&own[0], &own[1])
    {
        let plain = |v: &str| v.trim().trim_end_matches("px").parse::<f32>().ok();
        if let (Some(w), Some(h)) = (plain(w), plain(h)) {
            head.push_str(&format!(" viewBox=\"0 0 {w} {h}\""));
        }
    }
    format!(
        "{}<svg width=\"{}\" height=\"{}\"{head}>{}",
        &markup[..open],
        tile.0,
        tile.1,
        &markup[close + 1..]
    )
}
