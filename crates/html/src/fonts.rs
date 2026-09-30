//! Шрифты страницы: правило `@font-face`.
//!
//! Разметка вправе принести СВОЙ шрифт и назвать его как угодно:
//!
//! ```css
//! @font-face { font-family: 'мой'; src: url('/fonts/mplus.woff') }
//! p { font-family: 'мой' }
//! ```
//!
//! Система шрифтов знает файл под его СОБСТВЕННЫМ именем, а разметка просит
//! по своему — поэтому здесь два дела: загрузить файл в систему и запомнить,
//! какое настоящее имя стоит за придуманным. Подмену делает [`alias`], её
//! зовут все места, где семейство уходит в набор.
//!
//! Упаковки `woff` и `woff2` — это тот же sfnt: у первой сжаты таблицы, у
//! второй сверх того перестроены глифы. Распаковку делает крейт `wuff`.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Придуманное разметкой имя → имя, под которым шрифт знает система.
    static ALIASES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// Придуманное имя → дескриптор `font-feature-settings` его правила
    /// (css-fonts-4 §7.2, шаг 2). Ключ — ИМЯ ИЗ РАЗМЕТКИ: три правила с одним
    /// файлом (`lato-ffs-`, `lato-ffs-0`, `lato-ffs-1`) дают одно настоящее
    /// имя, а возможности у них разные.
    static FEATURES: RefCell<HashMap<String, Vec<(String, u32)>>> =
        RefCell::new(HashMap::new());
    /// ВСЕ лица семейства в порядке объявления: ширина из дескриптора
    /// `font-stretch` (отрезок процентов) и имя в системе. Одного слота мало:
    /// правил `@font-face` на одно имя бывает много, и выбор между ними ведёт
    /// css-fonts-4 §font-matching, а не «кто объявлен последним».
    static FACES: RefCell<HashMap<String, Vec<((f32, f32), String)>>> =
        RefCell::new(HashMap::new());
    /// Придуманное имя → есть ли в семействе знак U+0020. Семейство собирают
    /// из нескольких правил (подмножества знаков), и пробел у него есть,
    /// если его несёт ХОТЬ ОДНО.
    static HAS_SPACE: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    /// Придуманное имя → множитель `size-adjust` (css-fonts-5). `NaN` — правила
    /// одного семейства дают РАЗНЫЕ множители: лицо по наклону и весу мы не
    /// выбираем, и общий множитель красил бы чужое лицо.
    static SIZE_ADJUST: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
    /// Уже загруженные файлы: одно и то же правило встречается на странице
    /// не по разу, а разбор шрифта дорог.
    static LOADED: RefCell<HashMap<String, Option<String>>> =
        RefCell::new(HashMap::new());
    /// Приёмник шрифта: отдаёт системе байты и возвращает её имя семейства.
    static LOADER: RefCell<Option<Loader>> = const { RefCell::new(None) };
    /// Алфавитная базовая линия файла (`BASE`, тег `romn`) в долях em над
    /// нулём глифа — по адресу файла: байты есть только при первой загрузке.
    static ROMN_BY_SRC: RefCell<HashMap<String, Option<f32>>> = RefCell::new(HashMap::new());
    /// То же по придуманному страницей имени семейства (живёт одну страницу).
    static ROMN: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}

/// Загрузчик: получает содержимое файла, отдаёт имя семейства в системе.
type Loader = Box<dyn Fn(Vec<u8>) -> Option<String>>;

/// Поставить загрузчик шрифтов. Зовут там, где живёт система шрифтов.
pub fn install_loader(loader: impl Fn(Vec<u8>) -> Option<String> + 'static) {
    LOADER.with(|l| *l.borrow_mut() = Some(Box::new(loader)));
}

/// Настоящее имя семейства за именем из разметки.
pub fn alias(family: &str) -> Option<String> {
    ALIASES.with(|a| a.borrow().get(&family.to_ascii_lowercase()).cloned())
}

/// Возможности из дескриптора `font-feature-settings` правила `@font-face`.
pub fn face_features(family: &str) -> Vec<(String, u32)> {
    FEATURES.with(|f| {
        f.borrow()
            .get(&family.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    })
}

/// Ширина из дескриптора `font-stretch` правила `@font-face` — отрезок в
/// процентах (css-fonts-4 §font-stretch-desc). Одно значение даёт вырожденный
/// отрезок, пропуск и негодная запись — `normal`.
fn stretch_desc(value: Option<&str>) -> (f32, f32) {
    fn one(word: &str) -> Option<f32> {
        Some(match word.trim().to_ascii_lowercase().as_str() {
            "ultra-condensed" => 50.0,
            "extra-condensed" => 62.5,
            "condensed" => 75.0,
            "semi-condensed" => 87.5,
            "normal" => 100.0,
            "semi-expanded" => 112.5,
            "expanded" => 125.0,
            "extra-expanded" => 150.0,
            "ultra-expanded" => 200.0,
            pct => pct.strip_suffix('%')?.trim().parse::<f32>().ok()?,
        })
    }
    let Some(value) = value else {
        return (100.0, 100.0);
    };
    let mut parts = value.split_whitespace().filter_map(one);
    let Some(lo) = parts.next() else {
        return (100.0, 100.0);
    };
    let hi = parts.next().unwrap_or(lo);
    (lo.min(hi), lo.max(hi))
}

/// Настоящее имя семейства с учётом ЗАПРОШЕННОЙ ширины начертания.
///
/// css-fonts-4 §font-matching, шаг `font-stretch`: при запросе `<= 100%`
/// сперва перебираются лица НЕ ШИРЕ запроса — от ближайшего вниз, — затем
/// более широкие вверх; при запросе `> 100%` наоборот. Пока выбор был «кто
/// объявлен последним», семь пар `font-stretch-12…18` рисовались файлом
/// `fail.woff`, стоящим вторым, вместо `pass.woff`.
///
/// При РАВНОМ расстоянии побеждает объявленное ПОЗЖЕ — ровно то, что делал
/// прежний одиночный слот: семьи, где на одно имя приходится несколько правил
/// с одинаковым (отсутствующим) дескриптором — `unicode-range`,
/// `first-available-font-*` — не двигаются. Семейство без правил `@font-face`
/// уходит в прежнюю подмену.
pub fn alias_stretch(family: &str, want: Option<f32>) -> Option<String> {
    let key = family.to_ascii_lowercase();
    let want = want.unwrap_or(100.0);
    let picked = FACES.with(|f| {
        let faces = f.borrow();
        let list = faces.get(&key)?;
        let rank = |face: &((f32, f32), String)| -> (u8, f32) {
            let (lo, hi) = face.0;
            let v = want.clamp(lo, hi);
            if want <= 100.0 {
                if v <= want { (0, want - v) } else { (1, v - want) }
            } else if v >= want {
                (0, v - want)
            } else {
                (1, want - v)
            }
        };
        let mut best: Option<(&str, (u8, f32))> = None;
        for face in list {
            let r = rank(face);
            if best.is_none_or(|(_, b)| (r.0, r.1) <= (b.0, b.1)) {
                best = Some((face.1.as_str(), r));
            }
        }
        best.map(|(name, _)| name.to_string())
    });
    picked.or_else(|| alias(family))
}

/// Есть ли в семействе знак пробела (U+0020).
///
/// Вопрос не праздный: «первым доступным» шрифтом (css-fonts-4
/// §first-available-font) семейство становится, только если пробел в нём
/// есть, — от первого доступного считаются метрики строки, `line-height:
/// normal`, `ch` и `ex`. Ответ «нет» бывает единственно у правила
/// `@font-face` с дескриптором `unicode-range`, где пробел не назван. Про
/// все прочие имена ответ утвердительный: дескриптора у них нет, а его
/// умолчание — весь набор знаков.
pub fn covers_space(family: &str) -> bool {
    HAS_SPACE.with(|s| {
        s.borrow()
            .get(&family.to_ascii_lowercase())
            .copied()
            .unwrap_or(true)
    })
}

/// Покрывает ли дескриптор `unicode-range` знак U+0020.
///
/// Видов записи три (css-fonts-4 §unicode-range-desc): одиночный знак `U+20`,
/// отрезок `U+0-7F` и маска `U+00??`. Негодная запись делает дескриптор
/// недействительным целиком, а его умолчание — `U+0-10FFFF`, то есть пробел
/// покрыт.
fn range_has_space(value: Option<&str>) -> bool {
    let Some(value) = value else { return true };
    let mut any = false;
    for part in value.split(',') {
        let part = part.trim();
        let Some(body) = part.strip_prefix("U+").or_else(|| part.strip_prefix("u+")) else {
            return true;
        };
        let (lo, hi) = match body.split_once('-') {
            Some((a, b)) => (
                u32::from_str_radix(a.trim(), 16).ok(),
                u32::from_str_radix(b.trim(), 16).ok(),
            ),
            None if body.contains('?') => (
                u32::from_str_radix(&body.replace('?', "0"), 16).ok(),
                u32::from_str_radix(&body.replace('?', "F"), 16).ok(),
            ),
            None => {
                let one = u32::from_str_radix(body, 16).ok();
                (one, one)
            }
        };
        match (lo, hi) {
            (Some(lo), Some(hi)) if lo <= hi => any |= (lo..=hi).contains(&0x20),
            _ => return true,
        }
    }
    any
}

/// Разобрать правила `@font-face` из таблицы стилей и загрузить шрифты.
///
/// Путь в `url(...)` берётся как есть: страницу до движка доводит стенд, и
/// адреса в ней уже разрешены в файлы.
pub fn load_faces(css: &str) {
    // Имена семейств придумывает страница, и на соседней странице то же имя
    // значит другой файл — поэтому таблица подмены живёт РОВНО одну страницу.
    ALIASES.with(|a| a.borrow_mut().clear());
    FEATURES.with(|f| f.borrow_mut().clear());
    FACES.with(|f| f.borrow_mut().clear());
    HAS_SPACE.with(|s| s.borrow_mut().clear());
    SIZE_ADJUST.with(|s| s.borrow_mut().clear());
    ROMN.with(|r| r.borrow_mut().clear());
    load_faces_into(css);
}

/// Дозагрузка БЕЗ сброса подмен: вложенный документ (`<iframe>`) разбирается
/// посреди отрисовки внешнего — сброс крал бы шрифты хозяина, и весь текст
/// после рамки падал в подстановочный шрифт.
pub fn load_faces_additive(css: &str) {
    load_faces_into(css);
}

fn load_faces_into(css: &str) {
    for block in faces(css) {
        let (Some(family), Some(src)) = (declaration(&block, "font-family"), source(&block)) else {
            continue;
        };
        let real = LOADED.with(|c| c.borrow().get(&src).cloned());
        let real = match real {
            Some(hit) => hit,
            None => {
                let loaded = read_font(&src).and_then(|bytes| {
                    let romn = sfnt_romn_baseline(&bytes);
                    ROMN_BY_SRC.with(|m| m.borrow_mut().insert(src.clone(), romn));
                    LOADER.with(|l| l.borrow().as_ref().and_then(|load| load(bytes)))
                });
                LOADED.with(|c| c.borrow_mut().insert(src.clone(), loaded.clone()));
                loaded
            }
        };
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("FONT_DBG").is_ok());
            *ON
        } {
            eprintln!("FONT_DBG face family={family:?} src={src:?} real={real:?}");
        }
        if let Some(real) = real {
            let name = family.trim_matches(is_quote).to_ascii_lowercase();
            if let Some(Some(v)) = ROMN_BY_SRC.with(|m| m.borrow().get(&src).copied()) {
                ROMN.with(|r| r.borrow_mut().insert(name.clone(), v));
            }
            // Правил на одно семейство бывает много: пробел у семейства есть,
            // если его несёт хоть одно из них.
            let space = range_has_space(declaration(&block, "unicode-range").as_deref());
            HAS_SPACE.with(|s| {
                let mut s = s.borrow_mut();
                let seen = s.entry(name.clone()).or_insert(false);
                *seen |= space;
            });
            // Лицо запоминается вместе с шириной из дескриптора: выбор между
            // правилами одного семейства ведёт §font-matching.
            // `size-adjust: <percentage [0,∞]>`; без дескриптора — 100%.
            let adjust = declaration(&block, "size-adjust")
                .and_then(|v| v.trim().strip_suffix('%')?.trim().parse::<f32>().ok())
                .filter(|p| *p >= 0.0)
                .map_or(1.0, |p| p / 100.0);
            SIZE_ADJUST.with(|s| {
                let mut s = s.borrow_mut();
                let slot = s.entry(name.clone()).or_insert(adjust);
                if *slot != adjust {
                    *slot = f32::NAN;
                }
            });
            let width = stretch_desc(declaration(&block, "font-stretch").as_deref());
            FACES.with(|f| {
                f.borrow_mut()
                    .entry(name.clone())
                    .or_default()
                    .push((width, real.clone()));
            });
            if let Some(list) = declaration(&block, "font-feature-settings")
                .and_then(|v| crate::computed::feature_list(&v))
            {
                FEATURES.with(|f| f.borrow_mut().insert(name.clone(), list));
            }
            ALIASES.with(|a| a.borrow_mut().insert(name, real));
        }
    }
}

/// Множитель `size-adjust` семейства (1.0 — нет дескриптора или правила
/// семейства расходятся между собой).
pub fn size_adjust(family: &str) -> f32 {
    SIZE_ADJUST.with(|s| {
        s.borrow()
            .get(&family.to_ascii_lowercase())
            .copied()
            .filter(|k| k.is_finite())
            .unwrap_or(1.0)
    })
}

fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

/// Алфавитная базовая линия семейства в долях em над нулём глифа: у
/// `BaselineDiagnostic` она на 50/1000 выше нуля, у обычного шрифта — ноль
/// (css-inline-3 §4.3 `alphabetic`: «Use the alphabetic baseline»).
pub fn alphabetic_em(family: &str) -> f32 {
    let key = family.trim().trim_matches(is_quote).to_ascii_lowercase();
    ROMN.with(|r| r.borrow().get(&key).copied()).unwrap_or(0.0)
}

/// Координата базовой линии `romn` горизонтальной оси таблицы `BASE`
/// (OpenType BASE: Axis → BaseTagList/BaseScriptList → BaseValues →
/// BaseCoord) в долях em; скрипт `latn`, затем `DFLT`, затем первый.
/// `None` — таблицы, оси или тега нет.
fn sfnt_romn_baseline(bytes: &[u8]) -> Option<f32> {
    let be16 = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
    };
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let num_tables = be16(4)? as usize;
    let (mut base, mut head) = (None, None);
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        match bytes.get(rec..rec + 4)? {
            b"BASE" => base = Some(be32(rec + 8)? as usize),
            b"head" => head = Some(be32(rec + 8)? as usize),
            _ => {}
        }
    }
    let (base, head) = (base?, head?);
    let upem = be16(head + 18)? as f32;
    let horiz = be16(base + 4)? as usize;
    if upem <= 0.0 || horiz == 0 {
        return None;
    }
    let axis = base + horiz;
    let (tags_off, scripts_off) = (be16(axis)? as usize, be16(axis + 2)? as usize);
    if tags_off == 0 || scripts_off == 0 {
        return None;
    }
    let (tags, scripts) = (axis + tags_off, axis + scripts_off);
    let tag_count = be16(tags)? as usize;
    let romn = (0..tag_count)
        .find(|i| bytes.get(tags + 2 + i * 4..tags + 6 + i * 4) == Some(b"romn".as_slice()))?;
    let script_count = be16(scripts)? as usize;
    let mut script = None;
    for want in [b"latn", b"DFLT"] {
        for i in 0..script_count {
            let rec = scripts + 2 + i * 6;
            if bytes.get(rec..rec + 4)? == want.as_slice() {
                script = Some(scripts + be16(rec + 4)? as usize);
                break;
            }
        }
        if script.is_some() {
            break;
        }
    }
    let script = match script {
        Some(s) => s,
        None if script_count > 0 => scripts + be16(scripts + 6)? as usize,
        None => return None,
    };
    let values_off = be16(script)? as usize;
    if values_off == 0 {
        return None;
    }
    let values = script + values_off;
    if romn >= be16(values + 2)? as usize {
        return None;
    }
    let coord = values + be16(values + 4 + romn * 2)? as usize;
    Some(be16(coord + 2)? as i16 as f32 / upem)
}

/// Тела всех правил `@font-face` в таблице.
fn faces(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Комментарии срезаются ДО поиска: `/* @font-face {...} */` разбирался
    // как живое правило и грузил чужой файл.
    let css = &crate::css::strip_comments(css);
    let lower = css.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower[from..].find("@font-face") {
        let start = from + at;
        let Some(open) = css[start..].find('{') else {
            break;
        };
        let Some(close) = css[start + open..].find('}') else {
            break;
        };
        let body = css[start + open + 1..start + open + close].to_string();
        from = start + open + close;
        // Правило внутри ложного `@media`/`@supports` не действует
        // (css-conditional-3 §2): иначе вторая, «запасная» грань того же
        // семейства перебивала первую (`at-media-content-002`,
        // `at-supports-content-002`: `local('Arial')` вместо Ahem). Сюда
        // приходит вся разметка — стек скобок считается от начала своего
        // `<style>`.
        let base = lower[..start]
            .rfind("<style")
            .map_or(0, |s| lower[s..start].find('>').map_or(start, |g| s + g + 1));
        if crate::css::in_false_group(&css[base..], start - base, crate::css::Media::default()) {
            continue;
        }
        out.push(body);
    }
    out
}

/// Значение свойства внутри правила.
fn declaration(block: &str, name: &str) -> Option<String> {
    block.split(';').find_map(|decl| {
        let (key, value) = decl.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

/// Первый пригодный файл из `src`: берём тот, чью упаковку умеем открыть.
fn source(block: &str) -> Option<String> {
    let src = declaration(block, "src")?;
    let mut fallback = None;
    for part in src.split(',') {
        let Some(open) = part.find("url(") else {
            continue;
        };
        let rest = &part[open + 4..];
        let Some(close) = rest.find(')') else {
            continue;
        };
        let path = rest[..close].trim().trim_matches(is_quote).to_string();
        let known = matches!(
            std::path::Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("ttf" | "otf" | "woff" | "woff2")
        );
        if known {
            return Some(path);
        }
        fallback.get_or_insert(path);
    }
    fallback
}

/// Прочитать файл шрифта и, если он упакован, распаковать.
///
/// Обе упаковки — это тот же sfnt: в `woff` таблицы просто сжаты, в `woff2`
/// вдобавок перестроены таблицы глифов. Разбор второй руками не пишут, он
/// взят крейтом.
/// Годен ли sfnt: первые четыре байта — его версия.
///
/// Распаковка обязана дать именно sfnt. Иначе в систему уходит мусор, она
/// молча подменяет его своим шрифтом, и страница выглядит так, будто чужой
/// шрифт ПРИНЯТ — ровно то, чего негодный файл не должен добиваться.
fn sfnt_ok(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some(b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"typ1" | b"ttcf")
    )
}

fn read_font(path: &str) -> Option<Vec<u8>> {
    let path = path.strip_prefix("file:///").unwrap_or(path);
    let bytes = std::fs::read(path).ok()?;
    // Упаковку задаёт АДРЕС, а подпись внутри файла — то, что файл о себе
    // заявляет. Их расхождение — отказ (§4.1 WOFF2): файл `.woff2` с
    // подписью `XXXX` прежде проваливался мимо обеих веток распаковки и
    // уходил в систему сырым, будто это голый sfnt.
    let packing = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let bytes = match packing.as_deref() {
        Some("woff") => {
            if !bytes.starts_with(b"wOFF") {
                return None;
            }
            wuff::decompress_woff1(&bytes).ok()?
        }
        Some("woff2") => {
            if !bytes.starts_with(b"wOF2") {
                return None;
            }
            wuff::decompress_woff2(&bytes).ok()?
        }
        // Адрес без расширения (`data:`-подобные пути набора) — судим по
        // подписи, как прежде.
        _ if bytes.starts_with(b"wOFF") => wuff::decompress_woff1(&bytes).ok()?,
        _ if bytes.starts_with(b"wOF2") => wuff::decompress_woff2(&bytes).ok()?,
        _ => bytes,
    };
    if !sfnt_ok(&bytes) {
        return None;
    }
    Some(ensure_windows_names(bytes))
}

/// Дописать в name-таблицу записи Windows (platform 3, en-US).
///
/// DirectWrite строит имена семейства только из записей platform 3 (или Mac);
/// шрифт, где имена лежат лишь под platform 0 / language 0 (обычное дело для
/// тестовых шрифтов WPT — FontWithFancyFeatures и родня), получает ПУСТОЙ
/// набор локализованных имён: его не найти по имени, и текст молча рисуется
/// системной подменой. Строки platform 0 — тот же UTF-16BE, так что записи
/// просто дублируются с (3, 1, 0x0409) поверх того же строкового блоба;
/// пересобранная таблица дописывается в конец файла, а запись каталога
/// перенацеливается на неё.
fn ensure_windows_names(bytes: Vec<u8>) -> Vec<u8> {
    fn be16(b: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
    }
    fn be32(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            *b.get(at)?,
            *b.get(at + 1)?,
            *b.get(at + 2)?,
            *b.get(at + 3)?,
        ]))
    }
    let rebuild = || -> Option<Vec<u8>> {
        let num_tables = be16(&bytes, 4)? as usize;
        let mut dir_at = None;
        for i in 0..num_tables {
            let rec = 12 + i * 16;
            if bytes.get(rec..rec + 4)? == b"name" {
                dir_at = Some(rec);
                break;
            }
        }
        let rec = dir_at?;
        let base = be32(&bytes, rec + 8)? as usize;
        let count = be16(&bytes, base + 2)? as usize;
        let str_off = base + be16(&bytes, base + 4)? as usize;
        let mut records: Vec<[u16; 6]> = Vec::with_capacity(count * 2);
        let mut str_end = 0usize;
        for i in 0..count {
            let r = base + 6 + i * 12;
            let rec6 = [
                be16(&bytes, r)?,
                be16(&bytes, r + 2)?,
                be16(&bytes, r + 4)?,
                be16(&bytes, r + 6)?,
                be16(&bytes, r + 8)?,
                be16(&bytes, r + 10)?,
            ];
            if rec6[0] == 3 && rec6[3] == 1 {
                // Имя семейства для Windows уже есть — файл не трогается.
                return None;
            }
            str_end = str_end.max(rec6[4] as usize + rec6[5] as usize);
            records.push(rec6);
        }
        let blob = bytes.get(str_off..str_off + str_end)?.to_vec();
        let mut extra: Vec<[u16; 6]> = records
            .iter()
            .filter(|r| r[0] == 0 && matches!(r[3], 1 | 2 | 4 | 6 | 16 | 17))
            .map(|r| [3, 1, 0x0409, r[3], r[4], r[5]])
            .collect();
        if extra.is_empty() {
            return None;
        }
        records.append(&mut extra);
        records.sort();
        // Новая таблица целиком, в конец файла.
        let mut table = Vec::with_capacity(6 + records.len() * 12 + blob.len());
        for v in [0u16, records.len() as u16, 6 + records.len() as u16 * 12] {
            table.extend_from_slice(&v.to_be_bytes());
        }
        for r in &records {
            for v in r {
                table.extend_from_slice(&v.to_be_bytes());
            }
        }
        table.extend_from_slice(&blob);
        let mut out = bytes.clone();
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let new_off = out.len() as u32;
        out.extend_from_slice(&table);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        // Каталог: offset, length и честная контрольная сумма таблицы.
        let sum = table
            .chunks(4)
            .map(|c| {
                let mut w = [0u8; 4];
                w[..c.len()].copy_from_slice(c);
                u32::from_be_bytes(w)
            })
            .fold(0u32, u32::wrapping_add);
        out[rec + 4..rec + 8].copy_from_slice(&sum.to_be_bytes());
        out[rec + 8..rec + 12].copy_from_slice(&new_off.to_be_bytes());
        out[rec + 12..rec + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        Some(out)
    };
    rebuild().unwrap_or(bytes)
}

/// Имя семейства из name-таблицы sfnt (nameID 16, затем 1): регистрация
/// в системе может НЕ добавить нового имени в общий список (diff пуст —
/// FontWithFancyFeatures терял алиас и весь текст шёл шрифтом-подменой),
/// поэтому имя читается из самого файла.
pub fn sfnt_family(bytes: &[u8]) -> Option<String> {
    let be16 = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
    };
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let num_tables = be16(4)? as usize;
    let mut name_off = None;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if bytes.get(rec..rec + 4)? == b"name" {
            name_off = Some(be32(rec + 8)? as usize);
            break;
        }
    }
    let base = name_off?;
    let count = be16(base + 2)? as usize;
    let strings = base + be16(base + 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = base + 6 + i * 12;
        let platform = be16(rec)?;
        let name_id = be16(rec + 6)?;
        if name_id != 1 && name_id != 16 {
            continue;
        }
        let len = be16(rec + 8)? as usize;
        let off = strings + be16(rec + 10)? as usize;
        let raw = bytes.get(off..off + len)?;
        let text = match platform {
            // Windows/Unicode: UTF-16BE.
            0 | 3 => String::from_utf16(
                &raw.chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .ok()?,
            _ => String::from_utf8_lossy(raw).into_owned(),
        };
        if text.trim().is_empty() {
            continue;
        }
        // nameID 16 (typographic family) сильнее 1.
        let rank = if name_id == 16 { 2 } else { 1 };
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, t)| t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_rule_gives_family_and_file() {
        let css = "@font-face { font-family: 'мой'; src: url('/f/a.woff2') format('woff2'), \
                   url('/f/a.woff') format('woff') } p { color: red }";
        let block = faces(css);
        assert_eq!(block.len(), 1);
        assert_eq!(
            declaration(&block[0], "font-family").as_deref(),
            Some("'мой'")
        );
        // Обе упаковки нам по силам, поэтому берётся первая же.
        assert_eq!(source(&block[0]).as_deref(), Some("/f/a.woff2"));
    }

    #[test]
    fn only_font_face_blocks_are_taken() {
        let css = "p { font-family: 'нет' } @font-face { font-family: 'да'; src: url(a.ttf) }";
        let block = faces(css);
        assert_eq!(block.len(), 1);
        assert_eq!(
            declaration(&block[0], "font-family").as_deref(),
            Some("'да'")
        );
    }

    #[test]
    fn space_decides_the_first_available_font() {
        // Дескриптора нет — покрыт весь набор знаков.
        assert!(range_has_space(None));
        // Одиночный знак и отрезок.
        assert!(range_has_space(Some("U+20")));
        assert!(!range_has_space(Some("U+0061")));
        assert!(range_has_space(Some("U+0-7F")));
        assert!(!range_has_space(Some("U+0021-00FF")));
        // Несколько кусков: хватает одного.
        assert!(range_has_space(Some("U+20,U+41-5A")));
        assert!(!range_has_space(Some("U+0061, U+0062")));
        // Маска.
        assert!(range_has_space(Some("U+00??")));
        assert!(!range_has_space(Some("U+04??")));
        // Негодная запись — дескриптор недействителен, умолчание покрывает всё.
        assert!(range_has_space(Some("мусор")));
    }
}
