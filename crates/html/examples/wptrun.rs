//! Прогон reftest-ов WPT: тест и эталон рисуются НАШИМ движком, снимки
//! сравниваются между собой.
//!
//!     cargo run -p kamin-html --example wptrun -- <список.txt> [ширина] [высота]
//!
//! Формат списка: по строке на пару, `тест|эталон`. Эталон в reftest написан
//! примитивной вёрсткой, дающей заведомо тот же результат, поэтому
//! эталонного снимка из браузера не нужно: если наша раскладка верна, оба
//! файла дают одинаковую картинку.
//!
//! Всё в ОДНОМ процессе и одном окне: запуск окна стоит секунды, а пар —
//! тысячи. Документ подменяется в той же сущности, кадр снимается прямо
//! отсюда (`PrintWindow`), и следующая пара идёт без перезапуска.

use gpui::{
    AppContext as _, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    Styled, Timer, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, div, point, px, rgb, size,
};
use kamin_html::{BROWSER_CSS, Document, RenderOpts, render};
use std::rc::Rc;
use std::time::Duration;

struct Page {
    doc: Rc<Document>,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut text = window.text_style();
        text.color = rgb(0x000000).into();
        // Умолчание документа в браузере на Windows — Times New Roman 16px.
        // Тесты, которые шрифт не задают, меряются ИМ: ширина слова решает,
        // куда встанет перенос и до какого минимума сожмётся элемент ряда.
        text.font_family = "Times New Roman".into();
        // Полноширинную подстановку для кириллицы/кана НЕ навязываем: замер
        // показал 1041 → 1006 по css-text. Метрики системной подстановки
        // (кана в 0.82 кегля) совпадают с эталонами чаще, чем `MS Gothic`.
        text.font_size = px(16.).into();
        let opts = RenderOpts {
            viewport: (
                f32::from(window.viewport_size().width),
                f32::from(window.viewport_size().height),
            ),
            text,
            normal_line_height: 1.31,
        };
        // Начальный содержащий блок — ВИДИМАЯ ОБЛАСТЬ, и размер ему нужен
        // точный, в точках. С долей (`size_full`) высота корня остаётся
        // неопределённой, и абсолютный элемент, растянутый краями
        // (`top: 0; bottom: 0`), схлопывается в ноль — целые семейства
        // css-backgrounds и css-position выходили пустыми.
        if std::env::var("HTML_VIEWPORT").is_ok() {
            eprintln!("VIEWPORT {:?}", opts.viewport);
        }
        div()
            .w(px(opts.viewport.0))
            .h(px(opts.viewport.1))
            .bg(rgb(0xffffff))
            .text_size(px(16.))
            .children(render(self.doc.nodes(), &opts))
    }
}

/// Снимок клиентской области окна: массив байт BGRA и его размеры.
fn capture(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        let mut rect = Default::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let (w, h) = (
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        );
        if w == 0 || h == 0 {
            return None;
        }
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, w as i32, h as i32);
        let old = SelectObject(dc, bitmap.into());
        // Флаг 3 = PW_RENDERFULLCONTENT: без него аппаратно нарисованное окно
        // снимается пустым.
        let ok = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(3)).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                // Отрицательная высота — строки сверху вниз.
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            h,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(dc, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        (ok && lines > 0).then_some((w, h, pixels))
    }
}

/// Доля точек, разошедшихся сильнее допуска.
/// Выгрузить снимок пары в PNG рядом с отчётом.
///
/// Число расхождения говорит только «не сошлось». Что именно разъехалось —
/// видно лишь на картинке, и без неё правка идёт вслепую. Пишется по просьбе
/// (`WPT_DUMP=1`) и только для разошедшихся пар: на полном прогоне это тысячи
/// файлов.
fn dump(name: &str, shot: &(u32, u32, Vec<u8>)) {
    let dir = std::path::Path::new("target/wpt-shots");
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Ok(file) = std::fs::File::create(dir.join(format!("{name}.png"))) else {
        return;
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.0, shot.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let Ok(mut writer) = encoder.write_header() else {
        return;
    };
    // Снимок приходит от системы в порядке BGRA, PNG ждёт RGBA. Заодно
    // выставляется полная непрозрачность: у слоя окна альфа нулевая, и без
    // этого картинка открывается пустой.
    let mut rgba = shot.2.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    let _ = writer.write_image_data(&rgba);
}

/// Своя рабочая память в мегабайтах — второй след деградации стенда рядом со
/// временем пары. Растёт вместе с ним, если дело в утечке ресурсов окна.
fn rss_mb() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb).is_ok() {
                return (counters.WorkingSetSize / (1024 * 1024)) as u64;
            }
        }
        0
    }
    #[cfg(not(windows))]
    0
}

/// Сколько точек снимка ЯВНО красные.
///
/// Второй признак провала, не зависящий от эталона. Сравнение снимков слепо
/// там, где движок ломает обе стороны одинаково: пара сходится в ноль, а тест
/// провален. Но добрая половина набора прямо пишет «no red», и рисует красное
/// подложкой под зелёным — если красное видно, тест провален независимо от
/// совпадения. Порог по каналам взят с запасом, чтобы не считать красным
/// сглаживание кромок тёмного текста.
fn red_pixels(shot: &[u8]) -> usize {
    shot.chunks_exact(4)
        .filter(|p| p[2] > 150 && p[1] < 100 && p[0] < 100)
        .count()
}

/// Ниже этого числа нарисованных точек сторона считается ПУСТОЙ.
///
/// ★ Порог намеренно низкий. Прежняя тысяча объявляла пустыми целые семейства,
/// где обе стороны рисовали ОДИНАКОВО и немного (892/892, 675/675) — страница
/// из десятка знаков Ahem это норма, а не пустота. Пустота — это когда рисовать
/// нечего вовсе; несравнимость ловится отдельно, по РАЗНИЦЕ чернил сторон.
const INK_MIN: usize = 40;

/// Разделитель между страницами. Он РОВНЫЙ (все точки одного цвета) — по этому
/// признаку стенд понимает, что прошлая страница уже стёрта. Цвет заметный, а
/// не белый: окно стенда висит на экране десятками минут, и белизна неотличима
/// от зависшего стенда — на неё уже дважды жаловались.
const SEPARATOR: &str = "<body style=\"background:#cfd8e8;margin:0\"></body>";

fn diff(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 100.0;
    }
    let mut bad = 0usize;
    for (x, y) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        // Порог тот же, что у стенда против Chrome: сглаживание кромок — не
        // расхождение раскладки.
        if x[..3].iter().zip(&y[..3]).any(|(p, q)| p.abs_diff(*q) > 40) {
            bad += 1;
        }
    }
    bad as f32 * 400.0 / a.len() as f32
}

/// Разрешить ссылки документа: адреса картинок — в абсолютные пути, внешние
/// таблицы стилей — внутрь разметки.
///
/// Своего разрешения адресов у движка нет и быть не должно: он не знает, откуда
/// взялся документ. Знает это тот, кто документ открыл, — здесь стенд. Без
/// этого шага половина эталонов WPT рисует подпись `alt` вместо картинки, а
/// правила из `<link rel=stylesheet>` пропадают целиком.
/// Сколько точек кадра отличается от разделителя — «чернила» страницы.
///
/// Белый экран — это ОШИБКА, а не совпадение: пара, где обе стороны почти
/// ничего не нарисовали, сходится с нулевым расхождением и уходит в зелёные,
/// хотя сравнивать там нечего.
fn ink(shot: &[u8], blank: Option<&Vec<u8>>) -> usize {
    let Some(blank) = blank else {
        return usize::MAX;
    };
    if blank.len() != shot.len() {
        return usize::MAX;
    }
    shot.chunks_exact(4)
        .zip(blank.chunks_exact(4))
        .filter(|(x, y)| x[..3].iter().zip(&y[..3]).any(|(p, q)| p.abs_diff(*q) > 40))
        .count()
}

/// Ровный ли кадр — все точки одного цвета. Таким выходит пустая страница.
fn flat(buf: &[u8]) -> bool {
    let Some(first) = buf.get(..4) else {
        return true;
    };
    buf.chunks_exact(4).all(|px| px == first)
}

fn resolve_links(html: &str, path: &str) -> String {
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
        if lower.starts_with("<img") || lower.starts_with("<link") {
            let attr = if lower.starts_with("<img") {
                "src"
            } else {
                "href"
            };
            match attr_value(tag, attr).and_then(|v| resolve(&v).map(|p| (v, p))) {
                Some((href, file)) if lower.starts_with("<img") => {
                    // Разделитель пути в адресе — прямая косая даже на
                    // Windows: с обратной загрузчик картинок молча ничего не
                    // показывал, и эталоны из одних картинок выходили пустой
                    // страницей.
                    let uri = format!("file:///{}", file.display()).replace('\\', "/");
                    out.push_str(&tag.replace(&href, &uri));
                }
                Some((_, file)) if lower.contains("stylesheet") => {
                    let css = std::fs::read_to_string(&file).unwrap_or_default();
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
    while let Some(at) = find_url(tail) {
        with_urls.push_str(&tail[..at + 4]);
        let rest = &tail[at + 4..];
        let Some(close) = rest.find(')') else {
            with_urls.push_str(rest);
            return with_urls;
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
        match resolve(&decoded).or_else(|| resolve(&bare)) {
            // Кавычки ставятся ТОЛЬКО когда без них нельзя: адрес попадает и
            // в атрибут `style="…"`, а двойная кавычка внутри него обрывает
            // сам атрибут — правило теряется целиком вместе с картинкой
            // (`background-size-near-zero-png` и вся родня с оформлением по
            // месту). В имени файла из набора встречается апостроф
            // (`'green block.png` из `uri-004`), поэтому запасные кавычки —
            // двойные: в пути Windows их не бывает.
            Some(file) => {
                // Разделитель — ПРЯМАЯ косая: обратная в записи адреса
                // означает экранирование, и путь Windows терял её вместе со
                // следующим знаком (`C:\Users` превращалось в `C:Users`).
                let path = file.display().to_string().replace('\\', "/");
                let plain = !path.contains([' ', '\'', '"', '(', ')', ',', '\t']);
                if plain {
                    with_urls.push_str(&path);
                } else {
                    with_urls.push_str(&format!("\"{path}\""));
                }
            }
            None => with_urls.push_str(raw),
        }
        with_urls.push(')');
        tail = &rest[close + 1..];
    }
    with_urls.push_str(tail);
    with_urls
}

/// Ближайшая запись `url(` без учёта регистра.
fn find_url(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(3)).find(|&i| bytes[i..i + 4].eq_ignore_ascii_case(b"url("))
}

/// Адрес с раскодированными процентами.
fn percent_decode(raw: &str) -> String {
    if !raw.contains('%') {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

/// Значение атрибута в теге — в кавычках или без.
///
/// Разбор идёт с учётом кавычек: имя атрибута ищется только ВНЕ значений,
/// иначе `alt="см. src=1"` подсовывает чужое значение.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    let mut word_start: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' => {
                quote = Some(c);
                word_start = None;
            }
            b'=' => {
                if let Some(start) = word_start.take()
                    && tag[start..i].trim().eq_ignore_ascii_case(name)
                {
                    let value = tag[i + 1..].trim_start();
                    let quoted = |q: char| value.strip_prefix(q).and_then(|v| v.split(q).next());
                    return quoted('"')
                        .or_else(|| quoted('\''))
                        .or_else(|| value.split([' ', '>', '/']).next())
                        .map(str::to_string);
                }
            }
            c if c.is_ascii_whitespace() => word_start = None,
            _ => {
                if word_start.is_none() {
                    word_start = Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let list = args.get(1).cloned().unwrap_or_default();
    let w: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(800.);
    let h: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(600.);
    // `WPT_DUMP=1` — снимки разошедшихся пар, `WPT_DUMP=all` — всех подряд.
    // Второе нужно ручному разбору: там проверяется и то, что стенд счёл
    // сошедшимся (см. `scripts/wpt_review.py`).
    let dump_mode = std::env::var("WPT_DUMP").unwrap_or_default();
    let dumping = !dump_mode.is_empty();
    let dump_all = dump_mode == "all";
    let pairs: Vec<(String, String)> = std::fs::read_to_string(&list)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('|'))
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    if pairs.is_empty() {
        eprintln!("пустой список пар: {list}");
        std::process::exit(1);
    }

    Application::new().run(move |cx| {
        // Ahem — служебный шрифт набора: все его буквы одинаковые чёрные
        // квадраты в кегль. На нём построены сотни тестов: фигуры сходятся
        // ровно потому, что метрики предсказуемы. Без него текст набирается
        // системным шрифтом, и «квадраты» разъезжаются.
        for name in ["Ahem.ttf", "AHEM____.TTF"] {
            let path = std::path::Path::new("vendor/wpt-parsing/fonts").join(name);
            if let Ok(bytes) = std::fs::read(&path) {
                let _ = cx.text_system().add_fonts(vec![bytes.into()]);
                break;
            }
        }
        // Щуп метрик ставится ПОСЛЕ регистрации: до неё `Ahem` ещё не найден,
        // и `ch` мерился бы по шрифту-подмене.
        kamin_html::metrics::use_text_system(cx.text_system().clone());
        // Свои шрифты страницы (`@font-face`): файл отдаётся системе, а её имя
        // семейства запоминается под тем, которым шрифт зовут в разметке.
        let fonts = cx.text_system().clone();
        kamin_html::fonts::install_loader(move |bytes| {
            let before: std::collections::HashSet<String> =
                fonts.all_font_names().into_iter().collect();
            fonts.add_fonts(vec![bytes.into()]).ok()?;
            fonts
                .all_font_names()
                .into_iter()
                .find(|name| !before.contains(name))
        });
        let empty = Rc::new(Document::new("", BROWSER_CSS));
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(40.), px(40.)),
                        size: size(px(w), px(h)),
                    })),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    window_background: WindowBackgroundAppearance::Opaque,
                    ..Default::default()
                },
                |_, cx| -> Entity<Page> { cx.new(|_| Page { doc: empty }) },
            )
            .unwrap();
        // Фокус НЕ забираем: стенд идёт десятками минут, и всё это время его
        // окно висело поверх чужой работы — в паузах между страницами белое.
        // Снимок делает `PrintWindow` с флагом 3, ему передний план не нужен:
        // достаточно, чтобы окно не было СВЁРНУТО.

        cx.spawn(async move |cx| {
            // Первый кадр окна: до него снимок пустой.
            Timer::after(Duration::from_millis(900)).await;
            // Указатель окна нужен снимку: рисует его система, а не мы.
            let hwnd = cx
                .update_window(window.into(), |_, window, _| {
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    match window.window_handle().map(|h| h.as_raw()) {
                        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as isize,
                        _ => 0,
                    }
                })
                .unwrap_or(0);
            let mut report = String::new();
            // Показать страницу и дождаться ЕЁ кадра.
            //
            // Снимок делает система, а рисует окно по своему расписанию:
            // сна на глазок не хватает, и тяжёлая страница отдаёт кадр
            // ПРЕДЫДУЩЕЙ. По числам это выглядит как совпадение (ноль
            // расхождения) или как случайные скачки между прогонами.
            // Поэтому кадр опрашивается, пока не сменится и не устоится.
            // Чем кончился опрос кадра у каждого показа пары: `s` — кадр
            // сменился и устоялся, `b` — страница осталась равной разделителю,
            // `o` — запас опроса вышел, кадр есть, `n` — окно за весь запас не
            // отдало НИ ОДНОГО кадра. Без этого «пара идёт 27 секунд» и «пара
            // пустая» — два разных наблюдения без связи между ними.
            let trace = Rc::new(std::cell::RefCell::new(String::new()));
            let note = trace.clone();
            let mut show = async |html: String, prev: Option<Vec<u8>>, want_flat: bool| {
                let _ = cx.update_window(window.into(), |view, window, cx| {
                    if let Ok(page) = view.downcast::<Page>() {
                        page.update(cx, |page, cx| {
                            page.doc = Rc::new(Document::new(&html, BROWSER_CSS));
                            cx.notify();
                        });
                    }
                    window.refresh();
                });
                let lower = html.to_ascii_lowercase();
                let has_image = lower.contains("<img");
                // Страница с ДВИЖЕНИЕМ устаивается ложно: медленная анимация
                // даёт два одинаковых кадра подряд просто потому, что за 16 мс
                // картинка не изменилась на целую точку. Такой странице нужен
                // куда более длинный признак покоя — иначе одна и та же пара
                // скачет между прогонами (`css-flexbox-height-animation-stretch`:
                // то 1.13 %, то 0.03 %).
                let has_anim = lower.contains("animation") || lower.contains("transition");
                let mut last: Option<(u32, u32, Vec<u8>)> = None;
                let mut same = 0u32;
                let mut steady = 0u32;
                // Запас опроса: тяжёлой странице нужно время устояться, а лишнего
                // ожидания на лёгких не будет — цикл выходит по первому же
                // устоявшемуся кадру. Прежние 120 шагов (около двух секунд)
                // такие страницы обрезали, и одна и та же пара скакала между
                // прогонами на 15% расхождения.
                for _ in 0..400 {
                    Timer::after(Duration::from_millis(16)).await;
                    let shot = capture(hwnd);
                    let Some(now) = shot else { continue };
                    let changed = prev.as_ref().is_none_or(|before| *before != now.2);
                    let settled = last.as_ref().is_some_and(|before| before.2 == now.2);
                    // ПРОБОВАЛИ И ОТКАТИЛИ: требовать ТРИ одинаковых кадра
                    // подряд вместо двух — ради устойчивости чисел. Вышло
                    // наоборот: тяжёлая страница не успевает набрать три за
                    // отведённый опрос, снимок берётся последним и уходит
                    // недорисованным (css-grid 386 → 363, и разброс между
                    // двумя одинаковыми прогонами вырос до 18 пар). Лечится
                    // не строгостью признака, а ЗАПАСОМ ВРЕМЕНИ.
                    // Картинка приходит ПОЗЖЕ первого устоявшегося кадра:
                    // файл читается и раскодируется вне кадра, и страница
                    // успевает «устояться» без неё. На странице с `<img>`
                    // ждём подряд ЧЕТЫРЕ одинаковых кадра вместо двух —
                    // иначе одна и та же пара скачет между прогонами
                    // (`wm-propagation-body-032`: то 1.01 %, то 2.90 %, и
                    // картинки то есть, то нет).
                    let need = if has_anim {
                        30
                    } else if has_image {
                        16
                    } else {
                        4
                    };
                    steady = if settled { steady + 1 } else { 0 };
                    // Разделителю мало «устоялся»: он обязан быть РОВНЫМ.
                    // Иначе на экране остаётся кадр прошлой страницы, он и
                    // уходит в `blank`, а дальше от него считаются и «сменился»,
                    // и чернила — то есть следующая страница выглядит пустой
                    // (`css-position/multicol/static-position/*`).
                    if changed && steady + 1 >= need && (!want_flat || flat(&now.2)) {
                        note.borrow_mut().push('s');
                        return Some(now);
                    }
                    // Страница, которая НЕ нарисовалась, равна разделителю, и
                    // «сменился» на ней не наступает никогда — стенд вырабатывал
                    // весь запас опроса. На экране это долгая белизна, в
                    // числах — по 12 секунд на пару. Устоявшийся неизменный
                    // кадр принимается как ответ: он и есть итог, просто пустой.
                    // Ранний выход — ТОЛЬКО для кадра, равного разделителю:
                    // это и есть «страница не нарисовалась», ждать больше
                    // нечего. Принимать по устойчивости любой кадр нельзя —
                    // тяжёлая страница успевает застыть на полпути, и в
                    // сравнение уходит недорисованное (замерено: css-position
                    // просел 49 → 22 на одних таблицах).
                    let blank_now = prev.as_ref().is_some_and(|before| *before == now.2);
                    same = if settled && blank_now { same + 1 } else { 0 };
                    // Порог намеренно высокий (около двух секунд): тяжёлая
                    // страница даёт первый кадр далеко не сразу, и прежние
                    // восемь шагов (130 мс) объявляли пустой любую такую
                    // страницу. Ценой полутора секунд на действительно пустой
                    // странице покупается верный вердикт на тяжёлой
                    // (`css-position/multicol/static-position/*`: все 16 пар
                    // выходили пустыми, а по одной рисуются).
                    if same >= 120 {
                        note.borrow_mut().push('b');
                        return Some(now);
                    }
                    last = Some(now);
                }
                note.borrow_mut().push(if last.is_some() { 'o' } else { 'n' });
                last
            };
            // Время на пару: страница, у которой один кадр считается
            // секундами, выглядит на экране долгой пустотой и неотличима от
            // зависшего стенда. В отчёт оно не идёт (там сравниваются числа
            // расхождения), зато сразу видно, что тормозит.
            let mut slow: Vec<(u128, String)> = vec![];
            let mut timing = String::new();
            let mut timing_lines = 0usize;
            for (test, reference) in &pairs {
                let started = std::time::Instant::now();
                let mut shots = vec![];
                // Разделитель между страницами: без него «тест совпал с
                // эталоном» неотличимо от «ни один не перерисовался».
                // Разделитель обязан быть РОВНЫМ: если он сам не нарисовался,
                // «сменился» дальше считается от чужого кадра, и страница
                // принимается по устаревшему снимку. В числах это скачок вида
                // 17.45 % → 0.00 % между двумя одинаковыми прогонами.
                let mut blank = None;
                for path in [test, reference] {
                    // Разделитель показывается перед КАЖДОЙ стороной, а не один
                    // раз на пару. Иначе эталон снимается, пока на экране ещё
                    // устоявшийся ТЕСТ: его кадр отличается от разделителя, а
                    // значит принимается за кадр эталона — и пара получает
                    // ложный ноль расхождения.
                    blank = show(SEPARATOR.into(), None, true).await.map(|s| s.2);
                    let html =
                        resolve_links(&std::fs::read_to_string(path).unwrap_or_default(), path);
                    let mut shot = show(html.clone(), blank.clone(), false).await;
                    // Страница, не отличившаяся от разделителя, не нарисовалась:
                    // повторяем показ, а не записываем пустоту в отчёт.
                    for _ in 0..2 {
                        // Мерой служат ЧЕРНИЛА, а не побайтовое отличие: кадр с
                        // одной кромкой уже «не равен» разделителю, но рисовать
                        // на нём нечего. Страница, которая рисоваться умеет,
                        // получает ещё две попытки — пустой вердикт должен
                        // означать «действительно пусто», а не «не успела».
                        let drew = shot
                            .as_ref()
                            .is_some_and(|s| ink(&s.2, blank.as_ref()) >= INK_MIN);
                        // Повтор нужен странице, которая НЕ УСПЕЛА, а не той,
                        // что устоялась пустой: `b` в следе означает 120 равных
                        // разделителю кадров подряд — рисовать там нечего, и
                        // ещё два показа только жгут по две секунды каждый
                        // (css-backgrounds: 23 с на пару вместо 0.6 с).
                        let settled_blank = trace.borrow().ends_with('b');
                        if drew || settled_blank {
                            break;
                        }
                        shot = show(html.clone(), blank.clone(), false).await;
                    }
                    shots.push(shot);
                }
                // Тест сам сказал, чего быть не должно: «no red». Проверяем
                // это ДО сравнения с эталоном — оно слепо к случаю, когда обе
                // стороны сломаны одинаково.
                let source = std::fs::read_to_string(test).unwrap_or_default();
                let forbids_red = source.to_ascii_lowercase().contains("no red");
                let red_seen = forbids_red
                    && shots[0]
                        .as_ref()
                        .is_some_and(|s| red_pixels(&s.2) > s.2.len() / 4 / 2000);
                // WPT разрешает тесту НЕСКОЛЬКО эталонов (`rel=match`):
                // совпадение с любым — зачёт. Стенд сравнивает с первым, а
                // остальные проверяет, только если первый не сошёлся
                // (`hyphens-manual-011`: два эталона — с дефисом-минусом и с
                // настоящим знаком переноса).
                // ГОЧА: `rel="mismatch"` СОДЕРЖИТ подстроку `match`, а значит
                // при поиске по подстроке анти-эталон шёл запасным эталоном —
                // и тест, совпавший с ним (то есть по-настоящему провалившийся),
                // получал маленькое число и красился зелёным. Разбираем `rel`
                // списком слов и держим анти-эталоны отдельным оракулом.
                let mut alternates: Vec<String> = vec![];
                let mut mismatches: Vec<String> = vec![];
                let source_for_refs = std::fs::read_to_string(test).unwrap_or_default();
                for tag in source_for_refs.to_ascii_lowercase().split("<link").skip(1) {
                    let head = &tag[..tag.find('>').unwrap_or(tag.len())];
                    let Some(rel_at) = head.find("rel") else { continue };
                    let rel = head[rel_at + 3..]
                        .trim_start()
                        .strip_prefix('=')
                        .map(|r| r.trim_start())
                        .unwrap_or("");
                    let rel = match rel.chars().next() {
                        Some(q @ ('"' | '\'')) => {
                            let rest = &rel[1..];
                            &rest[..rest.find(q).unwrap_or(rest.len())]
                        }
                        _ => &rel[..rel.find(char::is_whitespace).unwrap_or(rel.len())],
                    };
                    let anti = rel.split_whitespace().any(|t| t == "mismatch");
                    if !anti && !rel.split_whitespace().any(|t| t == "match") {
                        continue;
                    }
                    let Some(at) = head.find("href") else { continue };
                    let rest = &head[at + 4..];
                    let Some(open) = rest.find(['"', '\'']) else {
                        continue;
                    };
                    let quote = rest.as_bytes()[open] as char;
                    let Some(close) = rest[open + 1..].find(quote) else {
                        continue;
                    };
                    let href = &rest[open + 1..open + 1 + close];
                    let dir = std::path::Path::new(test).parent().unwrap_or_else(|| std::path::Path::new("."));
                    let full = dir.join(href);
                    let full = full.to_string_lossy().replace('/', "\\");
                    if anti {
                        if !mismatches.contains(&full) {
                            mismatches.push(full);
                        }
                    } else if !full.eq_ignore_ascii_case(reference) && !alternates.contains(&full) {
                        alternates.push(full);
                    }
                }
                let verdict = match (&shots[0], &shots[1]) {
                    _ if red_seen => "красное видно".into(),
                    // Пустая страница совпадает с разделителем, и такая пара
                    // дала бы ложный ноль. Это не «сошлось», это «нечего
                    // сравнивать»: страница не нарисовалась вовсе.
                    // Белый экран у ЛЮБОЙ из сторон — ошибка. Побайтового
                    // равенства разделителю мало: страница с единственной
                    // кромкой сглаживания уже «не равна», а рисовать на ней
                    // нечего, и пара уходит в зелёные с нулём расхождения.
                    // Пусто у любой из сторон — либо одна нарисовала на порядок
                    // меньше другой: значит она не отрисовалась, а сравнение
                    // ничего не проверяет.
                    (Some((_, _, a)), Some((_, _, b)))
                        if {
                            let (x, y) = (ink(a, blank.as_ref()), ink(b, blank.as_ref()));
                            x.min(y) < INK_MIN || x.min(y) * 8 < x.max(y)
                        } =>
                    {
                        format!(
                            "пустая страница {}/{}",
                            ink(a, blank.as_ref()),
                            ink(b, blank.as_ref())
                        )
                    }
                    (Some((_, _, a)), Some((_, _, b))) => {
                        let d = diff(a, b);
                        format!("{d:.2}")
                    }
                    _ => "снимок не получен".into(),
                };
                // Не сошлось с первым эталоном — пробуем остальные.
                let mut verdict = verdict;
                if verdict.parse::<f32>().is_ok_and(|d| d > 0.5) {
                    for other in &alternates {
                        if !std::path::Path::new(other).is_file() {
                            continue;
                        }
                        let html = resolve_links(
                            &std::fs::read_to_string(other).unwrap_or_default(),
                            other,
                        );
                        blank = show(SEPARATOR.into(), None, true).await.map(|s| s.2);
                        let Some(shot) = show(html, blank.clone(), false).await else {
                            continue;
                        };
                        let Some((_, _, a)) = &shots[0] else { continue };
                        let d = diff(a, &shot.2);
                        if d < verdict.parse::<f32>().unwrap_or(f32::MAX) {
                            verdict = format!("{d:.2}");
                        }
                        if d <= 0.5 {
                            break;
                        }
                    }
                }
                // Анти-эталон (`rel="mismatch"`): совпадение с ним — ПРОВАЛ.
                // Проверяется всегда, даже когда пара уже зелёная: именно
                // зелёная пара и подозрительна — обе стороны могли сломаться
                // одинаково.
                if verdict.parse::<f32>().is_ok() {
                    for other in &mismatches {
                        if !std::path::Path::new(other).is_file() {
                            continue;
                        }
                        blank = show(SEPARATOR.into(), None, true).await.map(|s| s.2);
                        let html = resolve_links(
                            &std::fs::read_to_string(other).unwrap_or_default(),
                            other,
                        );
                        let Some(shot) = show(html, blank.clone(), false).await else {
                            continue;
                        };
                        let Some((_, _, a)) = &shots[0] else { continue };
                        if diff(a, &shot.2) <= 0.5 {
                            verdict = "совпал с анти-эталоном".into();
                            break;
                        }
                    }
                }
                if dumping && (dump_all || verdict.parse::<f32>().map_or(true, |d| d > 0.5)) {
                    let stem = std::path::Path::new(test)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if let Some(shot) = &shots[0] {
                        dump(&stem, shot);
                    }
                    if let Some(shot) = &shots[1] {
                        dump(&format!("{stem}--ref"), shot);
                    }
                }
                report.push_str(&format!("{test}|{reference}|{verdict}\n"));
                // Отчёт пишется после каждой пары: падение на одном файле не
                // должно стирать результат всего прогона.
                let _ = std::fs::write("target/wpt-report.txt", &report);
                // Порог — вдвое больше, чем нужно на три страницы с полным
                // опросом кадра. Всё, что дольше, тормозит не из-за ожидания.
                let spent = started.elapsed().as_millis();
                if spent > 8000 {
                    slow.push((spent, test.clone()));
                }
                // Замер по КАЖДОЙ паре, не только по медленным. Стенд
                // деградирует на длинном прогоне — страницы перестают отдавать
                // кадр, показ выбирает весь запас опроса, и числа раздела
                // становятся недостоверными. Порог в 8 секунд ловит только
                // хвост; кривая времени и памяти показывает, КОГДА поворот.
                timing.push_str(&format!(
                    "{}|{spent}|{}|{}\n",
                    timing_lines,
                    rss_mb(),
                    trace.replace(String::new()),
                ));
                timing_lines += 1;
                let _ = std::fs::write("target/wpt-timing.txt", &timing);
            }
            let _ = std::fs::write("target/wpt-report.txt", report);
            slow.sort_by(|a, b| b.0.cmp(&a.0));
            let mut lines = String::new();
            for (ms, test) in &slow {
                lines.push_str(&format!("{ms}|{test}\n"));
            }
            let _ = std::fs::write("target/wpt-slow.txt", lines);
            eprintln!("медленных пар: {}", slow.len());
            cx.update(|cx| cx.quit()).ok();
        })
        .detach();
    });
}
