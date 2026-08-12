//! Разбор CSS: декларации из `style=""` и правила из `<style>`.
//!
//! Своя реализация вместо `cssparser`: по замеру нашего же кода 82% селекторов —
//! одиночный класс, глубина не больше трёх, комбинаторов `>`/`+` на весь проект
//! четырнадцать. Полноценная CSS-машина здесь не окупается, а лишняя
//! зависимость — окупается ещё меньше.

use std::collections::HashMap;

/// Пара «свойство: значение». Значение хранится сырым — разбор откладывается
/// до момента применения, чтобы неизвестные свойства не стоили ничего.
pub type Decls = HashMap<String, String>;

/// Одно правило: с чем сопоставлять и что применять.
#[derive(Clone, Debug)]
pub struct Rule {
    pub sel: Selector,
    pub decls: Decls,
    /// Порядок в исходнике: при равной специфичности выигрывает последнее.
    pub order: usize,
    /// Откуда правило: 0 — таблица агента, 1 — таблица документа.
    ///
    /// Происхождение СТАРШЕ специфичности (CSS Cascade §6.4.4): авторское
    /// правило перебивает умолчание агента, даже когда специфичность у него
    /// ниже. Пока обе таблицы лежали в одном списке и сравнивались только
    /// специфичностью, `* { margin: 0 }` со специфичностью (0,0,0) проигрывал
    /// нашему `p { margin: 6px 0 }` — то есть не работал ни один reset.
    pub origin: u8,
}

/// Простой селектор — ровно то подмножество, которое встречается на практике.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Псевдокласс `:hover` и т.п. — применяется отдельным слоем.
    pub pseudo: Option<String>,
    /// Предок для `.a .b` и `.a > .b`. Прямой ли — во втором поле.
    pub ancestor: Option<Box<(Selector, bool)>>,
}

impl Selector {
    /// Специфичность как в CSS: (id, класс+псевдо, тег). Сравнивается лексикографически.
    pub fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (
            self.id.is_some() as u32,
            self.classes.len() as u32 + self.pseudo.is_some() as u32,
            self.tag.is_some() as u32,
        );
        if let Some(anc) = &self.ancestor {
            let a = anc.0.specificity();
            s = (s.0 + a.0, s.1 + a.1, s.2 + a.2);
        }
        s
    }

    fn parse_compound(raw: &str) -> Option<Selector> {
        let s = raw.trim();
        if s.is_empty() || s == "*" {
            return Some(Selector {
                tag: None,
                id: None,
                classes: vec![],
                pseudo: None,
                ancestor: None,
            });
        }
        let mut sel = Selector {
            tag: None,
            id: None,
            classes: vec![],
            pseudo: None,
            ancestor: None,
        };
        // Разделитель ищется ВНЕ скобок: в `:not(:first-child)` двоеточие и
        // точка — часть записи псевдокласса, а не начало следующего куска.
        // Пока это не учитывалось, `:not(...)` разбирался на два бессмысленных
        // псевдокласса и правило не совпадало ни с чем.
        let delim = |s: &str| {
            let mut depth = 0i32;
            let mut escaped = false;
            s.char_indices()
                .find(|(_, ch)| {
                    if escaped {
                        escaped = false;
                        return false;
                    }
                    match ch {
                        '\\' => escaped = true,
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '.' | '#' | ':' if depth == 0 => return true,
                        _ => {}
                    }
                    false
                })
                .map_or(s.len(), |(i, _)| i)
        };
        // Разбираем слева направо: имя тега идёт первым, дальше .класс/#id/:псевдо.
        let mut rest = s;
        let head_end = delim(rest);
        if head_end > 0 {
            sel.tag = Some(unescape(rest[..head_end].trim()).to_ascii_lowercase());
        }
        rest = &rest[head_end..];
        while !rest.is_empty() {
            let kind = rest.as_bytes()[0] as char;
            let body = &rest[1..];
            let end = delim(body);
            let name = &body[..end];
            match kind {
                '.' => sel.classes.push(unescape(name)),
                '#' => sel.id = Some(unescape(name)),
                // `:hover` и `::before` дают одно и то же имя: различать их
                // незачем — псевдоэлементы отбираются по имени.
                ':' => {
                    sel.pseudo = Some(unescape(name.trim_start_matches(':')).to_ascii_lowercase())
                }
                _ => return None,
            }
            rest = &body[end..];
        }
        Some(sel)
    }

    /// `.card > .title`, `.card .title`, `div.card` — всё сюда.
    pub fn parse(raw: &str) -> Option<Selector> {
        // Атрибутные селекторы и прочее, чего мы не умеем, отбрасываем целиком:
        // тихо применить половину правила хуже, чем не применить его совсем.
        // Двойное двоеточие — та же запись псевдоэлемента, что одинарная:
        // в разметке пишут , и отбрасывать его значило терять
        // значки и разделители, для которых сборка уже написана.
        // Внутри скобок `+` и `~` — не комбинаторы, а часть записи `2n+1` в
        // `:nth-child()`. Отбраковка по всей строке резала такие правила.
        let outside_parens = |c: char| {
            let mut depth = 0i32;
            raw.chars().any(|ch| {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0 && ch == c
            })
        };
        if raw.contains('[') || outside_parens('~') || outside_parens('+') {
            return None;
        }
        let mut parts: Vec<(String, bool)> = vec![];
        for chunk in raw.split('>') {
            let direct = !parts.is_empty();
            let mut first = true;
            for word in split_words(chunk) {
                parts.push((word.to_string(), direct && first));
                first = false;
            }
        }
        let (last, head) = parts.split_last()?;
        let mut sel = Selector::parse_compound(&last.0)?;
        // Флаг «предок обязан быть прямым» принадлежит ПОТОМКУ, а не предку:
        // в `.a > .b` его несёт `.b`. Поэтому при подъёме вверх флаг берётся
        // от текущего узла, а не от того, которого мы сейчас разбираем.
        let mut direct = last.1;
        let mut cursor = &mut sel;
        for (raw_part, part_direct) in head.iter().rev() {
            let parent = Selector::parse_compound(raw_part)?;
            cursor.ancestor = Some(Box::new((parent, direct)));
            direct = *part_direct;
            cursor = &mut cursor.ancestor.as_mut()?.0;
        }
        Some(sel)
    }
}

/// Где в значении стоит восклицательный знак — вне строк, скобок и
/// экранирования. `content: "!"` пометкой важности не является.
fn top_level_bang(value: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += value[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&value[at..], ch);
                continue;
            }
            _ if at_url(&value[at..]) => {
                at += skip_url(&value[at..]);
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            '!' if depth == 0 => return Some(at),
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

/// Разбор `style="a: 1; b: 2"`.
pub fn parse_decls(raw: &str) -> Decls {
    let mut out = Decls::new();
    for item in split_top_level(raw, ';') {
        // Двоеточие ищется НЕэкранированное: `bac\\kground` — это имя
        // `background`, а `background\\:` — имя с двоеточием внутри, то есть
        // объявление без двоеточия вовсе, и его надо отбросить
        // (`escapes-002`, `escapes-003`).
        let mut colon = None;
        let mut escaped = false;
        for (i, ch) in item.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                ':' => {
                    colon = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(colon) = colon else {
            continue;
        };
        let (k, v) = (&item[..colon], &item[colon + 1..]);
        let key = unescape(k.trim()).to_ascii_lowercase();
        // После восклицательного знака в объявлении стоит ровно `important` и
        // ничего больше; всё прочее делает объявление недействительным, и
        // отбрасывается оно целиком (CSS 2.1 §4.1.8). Пока пометка просто
        // срезалась с конца, `background: red ! fail` доезжало значением
        // `red ! fail`, а разбор цвета брал из него первое слово и красил
        // (`core-syntax-006`).
        let val = v.trim();
        if let Some(bang) = top_level_bang(val)
            && !val[bang + 1..].trim().eq_ignore_ascii_case("important")
        {
            continue;
        }
        // Пометка важности ОСТАЁТСЯ в значении: снимет её тот, кто раскладывает
        // каскад (`Computed::resolve_with_vars`), а срезав её здесь, мы теряли
        // важность целиком — объявление конкурировало на общих основаниях.
        let val = &unescape_value(val);
        if !key.is_empty() && !val.is_empty() {
            out.insert(key, val.to_string());
        }
    }
    out
}

/// Разбор содержимого `<style>` — список правил в порядке появления.
/// Условия окружения для `@media`.
///
/// Ширина и высота — в точках, тема — тёмная или светлая. Без них правило
/// пропускалось целиком, и разметка, написанная от узкого экрана вверх,
/// навсегда оставалась в узком виде.
#[derive(Clone, Copy, Debug)]
pub struct Media {
    pub width: f32,
    pub height: f32,
    pub dark: bool,
}

impl Default for Media {
    fn default() -> Self {
        Media {
            width: 1280.0,
            height: 800.0,
            dark: true,
        }
    }
}

impl Media {
    /// Выполняется ли условие `@media`.
    ///
    /// Поддержаны те проверки, что встречаются в разметке интерфейсов:
    /// ширина, высота и предпочтение темы. Незнакомую проверку считаем
    /// невыполненной — правило тогда не применяется, как и раньше.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().trim_start_matches("@media").trim();
        // Запятая — это «или».
        query.split(',').any(|alt| {
            alt.split(" and ").all(|part| {
                let part = part.trim().trim_start_matches('(').trim_end_matches(')');
                let Some((name, value)) = part.split_once(':') else {
                    // `screen`, `all` — верны; `print` — нет.
                    return matches!(part.trim(), "screen" | "all" | "");
                };
                let value = value.trim();
                let number = value
                    .trim_end_matches("px")
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(f32::NAN);
                match name.trim() {
                    "min-width" => self.width >= number,
                    "max-width" => self.width <= number,
                    "min-height" => self.height >= number,
                    "max-height" => self.height <= number,
                    "prefers-color-scheme" => (value == "dark") == self.dark,
                    _ => false,
                }
            })
        })
    }
}

pub fn parse_stylesheet(css: &str) -> Vec<Rule> {
    parse_stylesheet_media(css, Media::default())
}

/// То же, но с известными условиями окружения.
pub fn parse_stylesheet_media(css: &str, media: Media) -> Vec<Rule> {
    let mut out = vec![];
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    let mut order = 0usize;
    while let Some((piece, tail)) = next_piece(rest) {
        rest = tail;
        // At-правило-ПРЕДЛОЖЕНИЕ блока не имеет и кончается точкой с запятой:
        // `@import`, `@charset`, `@namespace`, `@layer a, b;`. Ни одно из них
        // ничего не задаёт нашей отрисовке, поэтому запись просто пропускается
        // вместе со всей своей преамбулой.
        let (head, body) = match piece {
            Piece::Statement { .. } => continue,
            Piece::Block { head, body } => (head.trim(), body),
        };
        // Незакрытый блок в КОНЦЕ таблицы закрывается неявно (CSS Syntax
        // §5.4.1): правило всё равно действует. Прежде такое правило
        // отбрасывалось целиком — а в наборе оно встречается прямо в тесте
        // (`break-spaces-009`: у `.test` нет закрывающей скобки, и коробка
        // теряла свою ширину вместе со всем остальным).
        // At-правила: тело у них устроено иначе, поэтому обычными правилами
        // их применять нельзя. `@media` и `@supports` разбираются как обёртка
        // над обычными правилами, `@keyframes` — отдельно (см.
        // `parse_keyframes`), остальные пропускаются.
        if head.starts_with('@') {
            // Имя at-правила регистронезависимо (§3.3): `@MeDIa` — то же
            // самое, что `@media` (`case-sensitive-001`).
            let name = head.to_ascii_lowercase();
            let inner = if name.starts_with("@media") {
                media.matches(&name)
            } else if name.starts_with("@supports") {
                supports(name.trim_start_matches("@supports"))
            } else {
                // Слой — прозрачная обёртка: внутри обычные правила, и вся
                // разница в приоритете, которого у нас пока нет. Отбрасывая
                // блок целиком, мы теряли всю разметку современных наборов
                // стилей — они целиком лежат в `@layer`.
                name.starts_with("@layer")
            };
            if inner {
                for r in parse_stylesheet_media(body, media) {
                    out.push(Rule {
                        order: order + r.order,
                        ..r
                    });
                }
                order += 1000;
            }
            continue;
        }
        let decls = parse_decls(body);
        if decls.is_empty() {
            continue;
        }
        for one in head.split(',') {
            if let Some(sel) = Selector::parse(one) {
                out.push(Rule {
                    sel,
                    decls: decls.clone(),
                    order,
                    // Разбор не знает, чья это таблица: происхождение ставит
                    // тот, кто её подключает (см. `dom.rs`).
                    origin: 0,
                });
                order += 1;
            }
        }
    }
    out
}

/// Аргументы функции CSS через запятую, не заходя внутрь вложенных скобок:
/// `rgba(0,0,0,.4), inset 0 0 2px red` — два аргумента, а не пять.
pub fn split_args(raw: &str) -> Vec<&str> {
    split_top_level(raw, ',')
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Разрез селектора по пробелам — не считая тех, что съедены экранированием.
///
/// Один пробел после шестнадцатеричного кода — его ограничитель, а не
/// комбинатор потомка: `.css\\0032 p` это ОДИН класс `css2p`, и в разметке
/// он так и записан (`escapes-013`). Пока резали обычным `split_whitespace`,
/// селектор превращался в «`p` внутри `.css2`» и совпадал не с тем узлом.
fn split_words(chunk: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut start = None;
    let mut at = 0usize;
    while at < chunk.len() {
        let ch = chunk[at..].chars().next().unwrap_or('\u{0}');
        if ch == '\\' {
            if start.is_none() {
                start = Some(at);
            }
            at += ch.len_utf8();
            at += first_escape(&chunk[at..]).len();
            continue;
        }
        if ch.is_whitespace() {
            if let Some(from) = start.take() {
                out.push(&chunk[from..at]);
            }
        } else if start.is_none() {
            start = Some(at);
        }
        at += ch.len_utf8();
    }
    if let Some(from) = start {
        out.push(&chunk[from..]);
    }
    out
}

/// Выполнено ли условие `@supports`.
///
/// Своё покрытие свойств мы в разборе не знаем, поэтому простое условие
/// считаем выполненным — им в разметке включают современный вариант. А вот
/// ОТРИЦАНИЕ обязано быть разобрано: `@supports not (display: grid)` — это
/// запасная ветка для движка БЕЗ поддержки, и, считая её выполненной, мы
/// применяли ровно то, что применять не должны, да ещё вместе с основной.
fn supports(condition: &str) -> bool {
    let c = condition.trim();
    match c.strip_prefix("not ") {
        Some(rest) => !supports(rest),
        None => true,
    }
}

/// Имя без экранирования (CSS Syntax §4.3.7).
///
/// `BSL0031 ` — знак по шестнадцатеричному коду, до шести цифр, и один
/// пробел после них съедается как ограничитель. `BSL.` — сама точка, а не
/// разделитель составного селектора. Пока этого не было, `p\\.class`
/// разбирался как тег `p` с классом `class` и совпадал с `p class="class"`,
/// хотя обязан искать тег с точкой в имени, то есть не совпадать ни с чем.
pub fn unescape(name: &str) -> String {
    if !name.contains('\\') {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut it = name.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 {
            match it.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    hex.push(*c);
                    it.next();
                }
                _ => break,
            }
        }
        if hex.is_empty() {
            // Экранирован обычный знак — он и остаётся, уже без особого
            // значения. Перевод строки экранировать нельзя, но в имени его и
            // не бывает.
            if let Some(c) = it.next() {
                out.push(c);
            }
            continue;
        }
        // Один пробел после цифр — ограничитель кода, а не часть имени.
        if it.peek().is_some_and(|c| c.is_whitespace()) {
            it.next();
        }
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            // Нулевой знак и суррогаты заменяются знаком замены (§4.3.7).
            Some(c) if c != '\u{0}' => out.push(c),
            _ => out.push('\u{fffd}'),
        }
    }
    out
}

/// Значение без экранирования — но кавычки не трогая.
///
/// Раскрывается всё, что раскрывается в имени, кроме внутренности строк:
/// `"\\""` — это кавычка ВНУТРИ строки, и раскрыв её, мы получили бы три
/// кавычки подряд и порвали значение (`escapes-001`).
///
/// Обрезать результат НЕЛЬЗЯ: `\\0020yellow` раскрывается в имя с пробелом
/// внутри, а такое значение недействительно; обрезка сделала бы из него
/// `yellow` и применила то, что применять нечего (`escapes-014`).
fn unescape_value(value: &str) -> String {
    if !value.contains('\\') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\u{0}');
        if ch == '"' || ch == '\'' {
            let body = at + ch.len_utf8();
            let end = body + skip_string(&value[body..], ch);
            out.push_str(&value[at..end]);
            at = end;
            continue;
        }
        if ch != '\\' {
            out.push(ch);
            at += ch.len_utf8();
            continue;
        }
        let tail = &value[at + ch.len_utf8()..];
        let taken = first_escape(tail);
        let one = unescape(&format!("\\{taken}"));
        // Пробел, полученный из кода, — часть ИМЕНИ, а не отступ, и
        // значение с таким именем недействительно. Наш конвейер
        // обрезает значение при использовании, поэтому раскрытие
        // потеряло бы ровно ту особенность, из-за которой объявление и
        // должно отпасть (`escapes-014`, `color:\\0020yellow`).
        if one.chars().all(char::is_whitespace) {
            out.push(ch);
            out.push_str(taken);
        } else {
            out.push_str(&one);
        }
        at += ch.len_utf8() + taken.len();
    }
    out
}

/// Сколько байт после обратного слэша съедает одно экранирование: до шести
/// шестнадцатеричных цифр и один пробел за ними, либо ровно один знак.
fn first_escape(tail: &str) -> &str {
    let mut end = 0usize;
    let mut digits = 0usize;
    for (i, ch) in tail.char_indices() {
        if digits < 6 && ch.is_ascii_hexdigit() {
            digits += 1;
            end = i + ch.len_utf8();
            continue;
        }
        if digits > 0 && ch.is_whitespace() {
            end = i + ch.len_utf8();
        }
        break;
    }
    if digits == 0 {
        end = tail.chars().next().map_or(0, char::len_utf8);
    }
    &tail[..end]
}

/// Чем кончилась очередная запись таблицы.
enum Piece<'a> {
    /// Правило с телом: заголовок и содержимое фигурных скобок.
    Block { head: &'a str, body: &'a str },
    /// At-правило-предложение: заголовок до точки с запятой, тела нет.
    Statement { head: &'a str },
}

/// Отрезать от таблицы одну запись, вернув её и остаток.
///
/// Скобки ВСЕХ ВИДОВ считаются вместе (§5.4.1): преамбула правила поглощает
/// уравновешенные `[]`, `()` и `{}`, а кончается на точке с запятой или на
/// теле в фигурных скобках — смотря что встретится раньше НА ВЕРХНЕМ УРОВНЕ.
/// Пока искалась просто первая `{`, неизвестное at-правило с мусором в
/// преамбуле (`@foo ] } ) … ;`) уводило разбор внутрь своего мусора, и вся
/// таблица за ним разъезжалась (`matching-brackets-001`, `core-syntax-001`).
fn next_piece(text: &str) -> Option<(Piece<'_>, &str)> {
    let mut square = 0i32;
    let mut round = 0i32;
    let mut at = 0usize;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&text[at..], ch);
                continue;
            }
            _ if at_url(&text[at..]) => {
                at += skip_url(&text[at..]);
                continue;
            }
            // Глубина не уходит в минус: лишняя `]` или `)` — просто знак
            // (§5.4.1), а не закрытие несуществующей скобки. Пока уходила,
            // преамбула `@foo ] } ) …` делала следующую настоящую `[`
            // нулевым уровнем, и точка с запятой ВНУТРИ скобок обрывала
            // at-правило раньше времени (`matching-brackets-001`).
            '[' => square += 1,
            ']' => square = (square - 1).max(0),
            '(' => round += 1,
            ')' => round = (round - 1).max(0),
            ';' if square == 0 && round == 0 => {
                let head = &text[..at];
                return Some((Piece::Statement { head }, &text[at + 1..]));
            }
            '{' if square == 0 && round == 0 => {
                let head = &text[..at];
                let rest = &text[at..];
                let (body, tail) = match find_matching(rest) {
                    Some(close) => (&rest[1..close], &rest[close + 1..]),
                    // Незакрытый блок в КОНЦЕ таблицы закрывается неявно
                    // (§5.4.1): правило всё равно действует.
                    None => (&rest[1..], ""),
                };
                return Some((Piece::Block { head, body }, tail));
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

/// Начинается ли здесь запись `url(`.
pub(crate) fn at_url(text: &str) -> bool {
    // Сравнение по БАЙТАМ: срез по четвёртому байту может разрезать
    // многобайтовый знак, и обычный срез строки на этом падает.
    let b = text.as_bytes();
    b.len() >= 4 && b[..4].eq_ignore_ascii_case(b"url(")
}

/// Где кончается запись `url(…)`, считая от `u`.
///
/// Незакавыченное содержимое — отдельный вид токена (§4.3.6): фигурная
/// скобка, точка с запятой и начало комментария внутри него ничего не значат.
/// Пока запись разбиралась как обычный текст, `url( { test )` открывал блок,
/// и остаток таблицы съезжал (`uri-012`).
pub(crate) fn skip_url(text: &str) -> usize {
    let mut at = 4; // `url(`
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&text[at..], ch);
                continue;
            }
            ')' => return at + ch.len_utf8(),
            _ => at += ch.len_utf8(),
        }
    }
    text.len()
}

/// Где кончается строка в кавычках, начавшаяся на `quote`.
///
/// Внутри неё не значат ничего ни скобки, ни точка с запятой, ни начало
/// комментария (CSS Syntax §4.3.5): `content: "}"` не закрывает правило, а
/// `content: "a;b"` — одно объявление. Обратный слэш снимает особость
/// следующего знака, в том числе самой кавычки.
pub(crate) fn skip_string(text: &str, quote: char) -> usize {
    let mut it = text.char_indices();
    while let Some((i, ch)) = it.next() {
        if ch == '\\' {
            it.next();
            continue;
        }
        if ch == quote {
            return i + ch.len_utf8();
        }
        // Незакрытая строка обрывается на переводе строки (§4.3.4): дальше
        // идёт обычный текст, а не бесконечная строка до конца таблицы.
        if ch == '\n' {
            return i;
        }
    }
    text.len()
}

/// Индекс `}` , парный первой `{`.
fn find_matching(from_brace: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut square = 0i32;
    let mut round = 0i32;
    let bytes = from_brace.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        let ch = from_brace[at..].chars().next().unwrap_or('\0');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += from_brace[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&from_brace[at..], ch);
                continue;
            }
            _ if at_url(&from_brace[at..]) => {
                at += skip_url(&from_brace[at..]);
                continue;
            }
            // Внутри `[` и `(` фигурная скобка ничего не закрывает.
            '[' => square += 1,
            ']' => square = (square - 1).max(0),
            '(' => round += 1,
            ')' => round = (round - 1).max(0),
            '{' if square == 0 && round == 0 => depth += 1,
            '}' if square == 0 && round == 0 => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut at = 0usize;
    while at < css.len() {
        let ch = css[at..].chars().next().unwrap_or('\0');
        // Кавычки и экранирование сильнее комментария: `content: "/*"` — это
        // текст, а не начало комментария до конца таблицы.
        match ch {
            '\\' => {
                let next = css[at + ch.len_utf8()..].chars().next();
                out.push(ch);
                if let Some(n) = next {
                    out.push(n);
                    at += ch.len_utf8() + n.len_utf8();
                } else {
                    at += ch.len_utf8();
                }
                continue;
            }
            '"' | '\'' => {
                let body = at + ch.len_utf8();
                let end = body + skip_string(&css[body..], ch);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ if at_url(&css[at..]) => {
                let end = at + skip_url(&css[at..]);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ => {}
        }
        if css[at..].starts_with("/*") {
            match css[at + 2..].find("*/") {
                Some(end) => at += 2 + end + 2,
                // Незакрытый комментарий тянется до конца файла (§4.3.2).
                None => return out,
            }
            continue;
        }
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

/// Разрезание по разделителю, не заходя внутрь скобок: `rgba(0, 0, 0, .5)`
/// содержит запятые, а `grid-template: repeat(2, 1fr)` — и запятые, и скобки.
fn split_top_level(raw: &str, sep: char) -> Vec<&str> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut at = 0usize;
    while at < raw.len() {
        let ch = raw[at..].chars().next().unwrap_or('\0');
        match ch {
            // Экранированный разделитель разделителем не служит:
            // `background: red\;` — одно объявление со значением `red;`.
            '\\' => {
                at += ch.len_utf8();
                at += raw[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&raw[at..], ch);
                continue;
            }
            _ if at_url(&raw[at..]) => {
                at += skip_url(&raw[at..]);
                continue;
            }
            // Блоки ЛЮБОГО вида непрозрачны: объявление с фигурными скобками
            // внутри (`test { :nested; color: yellow }`) — одно объявление, и
            // недействительное. Пока считались только круглые скобки, его
            // внутренности разбирались как отдельные объявления и применялись
            // (`core-syntax-001`).
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            c if c == sep && depth == 0 => {
                out.push(&raw[start..at]);
                start = at + ch.len_utf8();
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    out.push(&raw[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decls_keep_commas_inside_functions() {
        let d = parse_decls("color: rgba(1, 2, 3, .5); padding : 4px ");
        assert_eq!(
            d.get("color").map(String::as_str),
            Some("rgba(1, 2, 3, .5)")
        );
        assert_eq!(d.get("padding").map(String::as_str), Some("4px"));
    }

    #[test]
    fn layer_block_is_transparent_and_supports_not_is_honoured() {
        let media = Media::default();
        // Слой — обёртка: правило внутри него живёт.
        let rules = parse_stylesheet_media("@layer base { p { color: red } }", media);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].decls.get("color").map(String::as_str), Some("red"));
        // Отрицание — запасная ветка для движка БЕЗ поддержки; применять её
        // нельзя, иначе применяются обе ветки пары сразу.
        let neg = parse_stylesheet_media("@supports not (display: grid) { p { color: red } }", media);
        assert!(neg.is_empty());
        let pos = parse_stylesheet_media("@supports (display: grid) { p { color: red } }", media);
        assert_eq!(pos.len(), 1);
    }

    #[test]
    fn important_stays_in_the_value_for_the_cascade() {
        // Раньше тест закреплял обратное — что пометка срезается при разборе.
        // Именно из-за этого важность не работала вовсе: до каскада значение
        // доезжало неотличимым от обычного. Снимает пометку тот, кто
        // раскладывает каскад (`Computed::resolve_with_vars`).
        let d = parse_decls("color: red !important");
        assert_eq!(d.get("color").map(String::as_str), Some("red !important"));
    }

    #[test]
    fn bang_that_is_not_important_kills_the_declaration() {
        // После восклицательного знака стоит ровно `important` (CSS 2.1
        // §4.1.8); всё прочее делает объявление недействительным целиком.
        let d = parse_decls("color: red ! fail; background: green");
        assert_eq!(d.get("color"), None);
        assert_eq!(d.get("background").map(String::as_str), Some("green"));
    }

    #[test]
    fn selector_parts_and_specificity() {
        let s = Selector::parse("div.card#main:hover").unwrap();
        assert_eq!(s.tag.as_deref(), Some("div"));
        assert_eq!(s.id.as_deref(), Some("main"));
        assert_eq!(s.classes, vec!["card".to_string()]);
        assert_eq!(s.pseudo.as_deref(), Some("hover"));
        assert_eq!(s.specificity(), (1, 2, 1));
    }

    #[test]
    fn descendant_and_child_combinators() {
        let s = Selector::parse(".card > .title").unwrap();
        assert_eq!(s.classes, vec!["title".to_string()]);
        let anc = s.ancestor.as_ref().unwrap();
        assert_eq!(anc.0.classes, vec!["card".to_string()]);
        assert!(anc.1, "после > предок обязан быть прямым");

        let s = Selector::parse(".card .title").unwrap();
        assert!(!s.ancestor.as_ref().unwrap().1, "пробел = любой предок");
    }

    #[test]
    fn unsupported_selectors_are_dropped_whole() {
        assert!(Selector::parse("a[href]").is_none());
        // Псевдоэлемент разбирается: коробку из него строит `dom.rs`.
        assert_eq!(
            Selector::parse("li::before").and_then(|s| s.pseudo),
            Some("before".to_string())
        );
        assert!(Selector::parse("h1 + p").is_none());
    }

    #[test]
    fn media_rules_apply_when_the_condition_holds() {
        let css = "
            /* заметка */
            .a { color: red }
            @media (min-width: 10px) { .b { color: blue } }
            .c, .d { padding: 2px }
        ";
        let rules = parse_stylesheet(css);
        let sels: Vec<String> = rules.iter().map(|r| r.sel.classes.join(",")).collect();
        // Условие выполнено при ширине по умолчанию — правило внутри работает.
        assert_eq!(sels, vec!["a", "b", "c", "d"]);
        assert_eq!(rules[0].decls.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn media_rules_are_skipped_when_the_condition_fails() {
        let css = "@media (min-width: 2000px) { .b { color: blue } }";
        let rules = parse_stylesheet_media(
            css,
            Media {
                width: 400.0,
                ..Media::default()
            },
        );
        assert!(rules.is_empty(), "узкое окно не берёт правило для широкого");
    }

    #[test]
    fn color_scheme_query_follows_the_theme() {
        let css = "@media (prefers-color-scheme: dark) { .b { color: #fff } }";
        let dark = parse_stylesheet_media(
            css,
            Media {
                dark: true,
                ..Media::default()
            },
        );
        let light = parse_stylesheet_media(
            css,
            Media {
                dark: false,
                ..Media::default()
            },
        );
        assert_eq!(dark.len(), 1);
        assert!(light.is_empty());
    }
}

/// Кадры анимации: доля времени и объявления на этой доле.
pub type Keyframes = Vec<(f32, Decls)>;

/// `@keyframes имя { 0% {…} 100% {…} }` — все наборы кадров таблицы.
///
/// Разбираются отдельно от правил: у `@keyframes` тело состоит не из
/// объявлений, а из вложенных блоков, и общий разборщик такое телом правила
/// не считает.
pub fn parse_keyframes(css: &str) -> HashMap<String, Keyframes> {
    let cleaned = strip_comments(css);
    let mut out: HashMap<String, Keyframes> = HashMap::new();
    let mut rest = cleaned.as_str();
    while let Some(at) = rest.find("@keyframes") {
        rest = &rest[at + "@keyframes".len()..];
        let Some(brace) = rest.find('{') else { break };
        let name = rest[..brace].trim().to_string();
        let Some(close) = find_matching(&rest[brace..]) else {
            break;
        };
        let body = &rest[brace + 1..brace + close];
        rest = &rest[brace + close + 1..];

        let mut frames: Keyframes = vec![];
        let mut inner = body;
        while let Some(b) = inner.find('{') {
            let stops = inner[..b].trim();
            let Some(c) = find_matching(&inner[b..]) else {
                break;
            };
            let decls = parse_decls(&inner[b + 1..b + c]);
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
