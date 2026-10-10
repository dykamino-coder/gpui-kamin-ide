//! Resolve a document's links against its file: images to absolute URIs, stylesheets inlined.

use super::html_attrs::{attr_value, is_stylesheet_link, percent_decode};
use super::style_imports::expand_style_imports;
use super::stylesheet::{find_url, read_stylesheet, rebase_css_urls};

/// Разрешить ссылки документа: адреса картинок — в абсолютные пути, внешние
/// таблицы стилей — внутрь разметки.
///
/// Своего разрешения адресов у движка нет и быть не должно: он не знает, откуда
/// взялся документ. Знает это тот, кто документ открыл, — здесь стенд. Без
/// этого шага половина эталонов WPT рисует подпись `alt` вместо картинки, а
/// правила из `<link rel=stylesheet>` пропадают целиком.
pub(super) fn resolve_links(html: &str, path: &str) -> String {
    let dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    // Корень набора: от него считаются адреса, начинающиеся со слэша.
    let root = dir
        .ancestors()
        .find(|p| p.join("css").is_dir() && p.join("html").is_dir())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.clone());
    let resolve = |href: &str| -> Option<std::path::PathBuf> {
        if href.starts_with("data:") || href.contains("://") {
            return None;
        }
        let file = match href.strip_prefix('/') {
            Some(rest) => root.join(rest),
            None => dir.join(href),
        };
        file.exists().then_some(file)
    };

    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(end) = tail.find('>') else {
            out.push_str(tail);
            return out;
        };
        let tag = &tail[..=end];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("<img")
            || lower.starts_with("<link")
            || lower.starts_with("<iframe")
            || lower.starts_with("<embed")
            || lower.starts_with("<object")
            || lower.starts_with("<video")
        {
            let attr = if lower.starts_with("<link") {
                "href"
            } else if lower.starts_with("<object") {
                "data"
            } else if lower.starts_with("<video") {
                "poster"
            } else {
                "src"
            };
            // Таблица `data:text/css,…` (RFC 2397): содержимое — прямо в адресе,
            // файла нет, поэтому `resolve` её не находил и правила пропадали
            // (`layer-stylesheet-sharing*`). Разворачивается в `<style>`.
            if lower.starts_with("<link")
                && is_stylesheet_link(tag)
                && let Some(href) = attr_value(tag, "href")
                && let Some(body) = href
                    .strip_prefix("data:text/css,")
                    .or_else(|| href.strip_prefix("data:text/css;charset=utf-8,"))
            {
                out.push_str("<style>");
                out.push_str(&percent_decode(body));
                out.push_str("</style>");
                rest = &tail[end + 1..];
                continue;
            }
            // Пустой адрес (`<img src="">`) не переписывается: `replace("", uri)`
            // вставлял адрес между каждыми двумя символами тега
            // (block-max-height-004).
            match attr_value(tag, attr)
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| resolve(&v).map(|p| (v, p)))
            {
                Some((href, file))
                    if lower.starts_with("<img")
                        || lower.starts_with("<iframe")
                        || lower.starts_with("<embed")
                        || lower.starts_with("<object")
                        || lower.starts_with("<video") =>
                {
                    // Разделитель пути в адресе — прямая косая даже на
                    // Windows: с обратной загрузчик картинок молча ничего не
                    // показывал, и эталоны из одних картинок выходили пустой
                    // страницей.
                    let uri = format!("file:///{}", file.display()).replace('\\', "/");
                    out.push_str(&tag.replace(&href, &uri));
                }
                Some((_, file)) if is_stylesheet_link(tag) => {
                    let css = read_stylesheet(&file);
                    // Адреса внутри ПОДКЛЮЧЁННОГО файла считаются от ЕГО
                    // папки: после вставки в документ база сместилась бы на
                    // папку теста, и `url(WidthTest-Regular.otf)` из
                    // support/width-test.css терял шрифт (compression-004).
                    let css = match file.parent() {
                        Some(base) => rebase_css_urls(&css, base),
                        None => css,
                    };
                    out.push_str("<style>");
                    out.push_str(&css);
                    out.push_str("</style>");
                }
                _ => out.push_str(tag),
            }
        } else {
            out.push_str(tag);
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    // ПРОБОВАЛИ И ОТКАТИЛИ ДВАЖДЫ: подставлять `@import "…";` в `<style>`
    // содержимым файла. Первый заход искал по всему документу и рвал
    // разметку (css-text 992 → 930, шестьдесят две пустые страницы).
    // Второй разбирал только содержимое `<style>` — работает, но счёт
    // 992 → 991: `letter-spacing-206` зелёным не стал, а `-201`
    // покраснел. Причина: без подстановки ОБЕ стороны пары набирались
    // подменой шрифта и сходились; с ней Ahem получает только та
    // сторона, где `@import` есть. Возвращать вместе с разбором пар.
    // Адреса внутри стилей — `url(...)` у `@font-face` и фонов: их разрешаем
    // отдельным проходом, потому что в тегах они не лежат.
    let mut with_urls = String::with_capacity(out.len());
    let mut tail = out.as_str();
    // Имя записи регистронезависимо (§3.3): `URL(` и `Url(` — та же запись.
    while let Some((at, head)) = find_url(tail) {
        with_urls.push_str(&tail[..at + head]);
        let rest = &tail[at + head..];
        // Незакрытая запись живёт до конца СВОЕГО стиля — его закрывает
        // EOF таблицы (§4.2), а не конец документа (`uri-017`). Скобка,
        // найденная уже за `</style`, — из чужой разметки, не наша.
        let (close, after_len, closed) = match rest.find(')') {
            Some(close) if !rest[..close].to_ascii_lowercase().contains("</style") => {
                (close, close + 1, true)
            }
            _ => {
                let end = rest
                    .to_ascii_lowercase()
                    .find("</style")
                    .unwrap_or(rest.len());
                (end, end, false)
            }
        };
        let raw = rest[..close].trim();
        let bare = raw.trim_matches(|c| c == '\'' || c == '"');
        // Проценты раскодируются: в адресе они стоят вместо знаков, которые в
        // имени файла записаны как есть (`%27green%20block.png` — это файл
        // `'green block.png`). Браузер раскодирует их при обращении к файлу,
        // и без этого путь просто не находится (`uri-004`).
        // Экранирование снимается ДО поиска файла: в адресе оно записывает
        // знаки, которые в имени файла стоят как есть (`support/\\'green\\ block.png`
        // — это файл `'green block.png`). Стенд ищет файл так же, как его
        // нашёл бы браузер (`uri-005`).
        let bare = kamin_html::css::unescape(bare);
        let decoded = percent_decode(&bare);
        // Адрес с фрагментом (`file.svg#mask`): файл существует без хвоста —
        // резолвим базу, хвост приклеиваем обратно
        // (mask-image-url-remote-mask).
        let frag_split = |s: &str| -> (String, Option<String>) {
            match s.split_once('#') {
                Some((b, f)) if !b.is_empty() => (b.to_string(), Some(f.to_string())),
                _ => (s.to_string(), None),
            }
        };
        let (dec_base, dec_frag) = frag_split(&decoded);
        let (bare_base, bare_frag) = frag_split(&bare);
        let resolved = resolve(&decoded)
            .map(|f| (f, None))
            .or_else(|| resolve(&bare).map(|f| (f, None)))
            .or_else(|| {
                dec_frag
                    .as_ref()
                    .and_then(|fr| resolve(&dec_base).map(|f| (f, Some(fr.clone()))))
            })
            .or_else(|| {
                bare_frag
                    .as_ref()
                    .and_then(|fr| resolve(&bare_base).map(|f| (f, Some(fr.clone()))))
            });
        match resolved {
            // Кавычки ставятся ТОЛЬКО когда без них нельзя: адрес попадает и
            // в атрибут `style="…"`, а двойная кавычка внутри него обрывает
            // сам атрибут — правило теряется целиком вместе с картинкой
            // (`background-size-near-zero-png` и вся родня с оформлением по
            // месту). В имени файла из набора встречается апостроф
            // (`'green block.png` из `uri-004`), поэтому запасные кавычки —
            // двойные: в пути Windows их не бывает.
            Some((file, frag)) => {
                // Разделитель — ПРЯМАЯ косая: обратная в записи адреса
                // означает экранирование, и путь Windows терял её вместе со
                // следующим знаком (`C:\Users` превращалось в `C:Users`).
                let mut path = file.display().to_string().replace('\\', "/");
                if let Some(fr) = frag {
                    path.push('#');
                    path.push_str(&fr);
                }
                let plain = !path.contains([' ', '\'', '"', '(', ')', ',', '\t']);
                if plain {
                    with_urls.push_str(&path);
                } else {
                    with_urls.push_str(&format!("\"{path}\""));
                }
            }
            None => with_urls.push_str(raw),
        }
        if closed {
            with_urls.push(')');
        }
        tail = &rest[after_len..];
    }
    with_urls.push_str(tail);
    // Подключения разворачиваются ПОСЛЕ разбора адресов: к этому мигу
    // `@import url(...)` уже несёт разрешённый путь к файлу.
    let with_urls = expand_style_imports(&with_urls, &dir);
    if std::env::var("WPT_HTML_DUMP").is_ok() {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open("target/dbg-page.html")
        {
            let _ = writeln!(f, "<!-- ==== page ==== -->");
            let _ = f.write_all(with_urls.as_bytes());
        }
    }
    with_urls
}
