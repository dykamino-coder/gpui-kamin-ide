//! Computed::apply_one: display, flex*, align-*/justify-*/place-*, gap, order.

use crate::style::computed::*;
use crate::style::values::value::Len;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_display_flex(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "display"
                if v.trim().eq_ignore_ascii_case("-webkit-box")
                    || v.trim().eq_ignore_ascii_case("-webkit-inline-box") =>
            {
                self.webkit_box = Some(true);
                // `-webkit-inline-box` — та же легаси-коробка, но ВСТРОЕННАЯ
                // (по факту `inline-block`): соседний текст обязан стоять с
                // ней в одной строке (`webkit-line-clamp-024`).
                if v.trim().eq_ignore_ascii_case("-webkit-inline-box") {
                    self.display = Some(Display::InlineBlock);
                }
            }
            "display" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::DISPLAY;
                    return;
                }
                self.inline_display = None;
                // Каскад мог поставить группу выше по важности, а ниже —
                // обычный блок: метка рода не переживает своё значение.
                self.row_group_kind = None;
                self.col_role = None;
                // Роль руби живёт вместе со значением `display`: более
                // важное `display: block` на `span.rt` снимает её.
                self.ruby_role = None;
                // Запись из ДВУХ слов (CSS Display 3): `inline grid-lanes`,
                // `block flow` и родня — внешний вид и внутренний.
                //
                // ★ ЗАМЕРЕНО: разбирать её ЦЕЛИКОМ (внешний `inline` → свои
                // строчные виды) — минус: css-grid 393 → 381. В наборе 233
                // файла пишут `display: inline grid`, и наша строчная сетка
                // им хуже блочной. Поэтому из двух слов берётся только то,
                // чего иначе не выразить вовсе, — раскладка лунками.
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разбор записи из ДВУХ слов
                // (`display: inline grid-lanes`, CSS Display 3). Полный разбор
                // (внешний `inline` → свои строчные виды) — css-grid 393 → 381:
                // в наборе 233 файла пишут `display: inline grid`, и наша
                // строчная сетка им хуже блочной. Разбор только ради лунок —
                // 393 → 386, обтяжка содержимого у них же — 382. То есть наша
                // раскладка лунками этим 188 файлам пока ХУЖЕ обычного блока;
                // возвращать разбор — вместе с настоящей строчной коробкой.
                self.display = match v {
                    "flex" => Some(Display::Flex),
                    "inline-flex" => Some(Display::InlineFlex),
                    "grid" => Some(Display::Grid),
                    // CSS Grid 3: раскладка ЛУНКАМИ. Элементы идут в самую
                    // короткую лунку, а не в решётку — поэтому это отдельный
                    // вид, а не разновидность сетки.
                    // Строчный вариант ведёт себя в потоке иначе, но лунки
                    // внутри те же: без него контейнер падал в умолчание, и
                    // вся укладка шла столбиком (`*-subgrid-grid-gap-*`).
                    "grid-lanes" | "masonry" => Some(Display::GridLanes),
                    "inline-grid-lanes" | "inline-masonry" => {
                        self.lanes_inline = true;
                        Some(Display::GridLanes)
                    }
                    // Двухсловная строчная сетка: прежний откат (393→381) был
                    // БЕЗ обтяжки InlineGrid — теперь строчная сетка обнимает
                    // Px-дорожки, и разбор снимается с полки (эталоны
                    // subgrid-alignment-* пишут `display: inline grid`).
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разбирать `display: inline
                    // grid-lanes` и `inline masonry` (сейчас запись не
                    // разбирается вовсе — `_ => self.display`, и 150 пар свода
                    // получают обычный блок вместо лунок). Отдавали блочные
                    // лунки, БЕЗ обтяжки `lanes_inline` (её прошлый замер:
                    // grid-семья 573 → 558). Срез из этих 150 пар: 28 зелёных
                    // → 18. Приобретено НОЛЬ, потеряно десять — все
                    // подсеточные и по содержимому (`grid-lanes-subgrid-001b/
                    // c/d` 0.09 → 11.10, `grid-lanes-subgrid-intrinsic-sizing`
                    // 0.28 → 10.27, `column-subgrid-extra-margin-002/004`).
                    // То есть этим 122 красным мешает не отсутствие лунок, а
                    // подсетка и вклад содержимого: настоящий контейнер лунок
                    // им пока ХУЖЕ блока. Возвращать вместе с подсеткой лунок.
                    "inline grid" => Some(Display::InlineGrid),
                    "inline flex" => Some(Display::InlineFlex),
                    "none" => Some(Display::None),
                    "block" => Some(Display::Block),
                    "inline-block" => Some(Display::InlineBlock),
                    "inline" => {
                        // Метка «настоящий строчный»: блокификация под
                        // float/abspos (§9.7) и запрет width/height на
                        // незамещаемом (§10.2) решаются после каскада.
                        self.inline_display = Some(true);
                        Some(Display::InlineBlock)
                    }
                    "inline-grid" => Some(Display::InlineGrid),
                    // Элемент исчезает, дети встают на его место.
                    "contents" => Some(Display::Contents),
                    "list-item" => Some(Display::ListItem),
                    // Табличные роли: своей табличной раскладки у нас нет,
                    // но строка — это ряд, ячейка — блок, а сама таблица
                    // ведёт себя как блок. Это ближе к правде, чем ничего.
                    "table" => Some(Display::Table),
                    // Таблица, стоящая В СТРОКЕ, как inline-block.
                    "inline-table" => Some(Display::InlineTable),
                    // css-ruby-1 §2.1: руби-виды. Контейнер и внутренние
                    // коробки — настоящие строчные (как `display: inline`),
                    // роль хранится отдельно (`ruby_role`); `block ruby`
                    // (§2.1.2) — блок с ролью контейнера, строчный контейнер
                    // внутри него синтезирует `dom::walk`. Blink знает только
                    // `ruby`, `block ruby` и `ruby-text` (`css_value_keywords`),
                    // остальные роли — по спеке и A.1.
                    "ruby" | "inline ruby" | "ruby-base" | "ruby-text" | "ruby-base-container"
                    | "ruby-text-container" => {
                        self.ruby_role = Some(match v {
                            "ruby-base" => RubyRole::Base,
                            "ruby-text" => RubyRole::Text,
                            "ruby-base-container" => RubyRole::BaseContainer,
                            "ruby-text-container" => RubyRole::TextContainer,
                            _ => RubyRole::Container,
                        });
                        self.inline_display = Some(true);
                        Some(Display::InlineBlock)
                    }
                    "block ruby" => {
                        self.ruby_role = Some(RubyRole::Container);
                        Some(Display::Block)
                    }
                    "table-row-group" | "table-header-group" | "table-footer-group" => {
                        self.row_group_kind = Some(match v {
                            "table-header-group" => 0,
                            "table-footer-group" => 2,
                            _ => 1,
                        });
                        Some(Display::TableRowGroup)
                    }
                    // run-in решается ПОСЛЕ построения дерева: вбегает
                    // первым строчным в следующий блок или остаётся блоком
                    // (dom::fold_run_ins).
                    "run-in" => {
                        self.run_in = Some(true);
                        Some(Display::Block)
                    }
                    "flow-root" => {
                        // Коробка блочная, но признак не теряется: это
                        // свой контекст форматирования (css-display-3).
                        self.flow_root = Some(true);
                        Some(Display::Block)
                    }
                    "table-row" => Some(Display::TableRow),
                    "table-cell" => Some(Display::TableCell),
                    // Заголовок таблицы — обычный блок. Колонки коробок не
                    // порождают вовсе: они только задают ширину столбцам.
                    "table-caption" => {
                        // Заголовок — блочная коробка с МЕТКОЙ: таблица ищет
                        // его по ней, а не только по тегу caption.
                        self.is_caption = Some(true);
                        Some(Display::Block)
                    }
                    // Колонка коробки НЕ порождает (§17.2.1): `Display`
                    // остаётся `None`. Метка живёт отдельно — по ней узел
                    // переживает разбор дерева, и только по ней его находит
                    // таблица.
                    "table-column" | "table-column-group" => {
                        self.col_role = Some(u8::from(v == "table-column-group"));
                        Some(Display::None)
                    }
                    // Блочный пункт списка со своим контекстом (css-display-3
                    // §2.3: `flow-root list-item`, `block flow-root
                    // list-item` — любые перестановки слов). Прежде запись
                    // уходила в `_ => self.display`, и `<span class=li>`
                    // оставался строчным (`display-flow-root-list-item-001`
                    // 10.88 против 5.86 у той же разметки без `list-item`).
                    two if {
                        let mut w: Vec<&str> = two.split_whitespace().collect();
                        w.sort_unstable();
                        w == ["flow-root", "list-item"] || w == ["block", "flow-root", "list-item"]
                    } =>
                    {
                        self.flow_root = Some(true);
                        Some(Display::ListItem)
                    }
                    // Запись из ДВУХ слов: из неё берётся только внутренний
                    // вид «лунки» — его иначе не выразить вовсе. Полный разбор
                    // двух слов ЗАМЕРЕН и откачен (см. комментарий выше).
                    two if two
                        .split_whitespace()
                        .any(|w| w == "grid-lanes" || w == "masonry") =>
                    {
                        if two.split_whitespace().any(|w| w == "inline") {
                            self.lanes_inline = true;
                        }
                        Some(Display::GridLanes)
                    }
                    _ => self.display,
                }
            }
            "flex-direction" => {
                self.flex_dir = match v {
                    "row" => Some(FlexDir::Row),
                    "row-reverse" => Some(FlexDir::RowReverse),
                    "column" => Some(FlexDir::Col),
                    "column-reverse" => Some(FlexDir::ColReverse),
                    _ => self.flex_dir,
                }
            }
            "flex-wrap" => {
                // css-flexbox-2 §5.2: `nowrap | [ wrap | wrap-reverse ] || balance`;
                // `balance` без `wrap*` ведёт себя как `wrap`. Невалидное
                // сочетание (`nowrap balance`, два режима) отбрасывается.
                let (mut wrap, mut reverse, mut balance, mut nowrap, mut modes, mut valid) =
                    (false, false, 0u8, false, 0u8, true);
                for word in v.split_ascii_whitespace() {
                    match word {
                        "wrap" => modes += 1,
                        "wrap-reverse" => {
                            modes += 1;
                            reverse = true;
                        }
                        "balance" => balance += 1,
                        "nowrap" => nowrap = true,
                        _ => valid = false,
                    }
                }
                wrap |= modes > 0 || balance > 0;
                if valid && modes <= 1 && balance <= 1 && (!nowrap || (modes == 0 && balance == 0)) {
                    self.flex_wrap = Some(wrap);
                    // Обратный перенос кладёт строки с другого края: одна строка
                    // в контейнере уезжает вниз, а не остаётся вверху.
                    self.flex_wrap_reverse = Some(reverse);
                    self.flex_balance = Some(balance > 0);
                }
            }
            "flex-line-count" => {
                // css-flexbox-2 §5.3: `<integer [1,∞]>`; действует только у
                // balance (как в Blink — `balance-min-line-count-007/008`).
                if let Ok(n) = v.trim().parse::<u32>()
                    && n >= 1
                {
                    self.flex_line_count = Some(n.min(u32::from(u16::MAX)) as u16);
                }
            }
            // Отрицательные значения невалидны (css-flexbox-1 §7.2: «Negative
            // values are not allowed») — объявление отбрасывается целиком
            // (`flex-shrink-002`, `flex-basis-004`).
            "flex-grow" => {
                if let Some(g) = flex_factor(v) {
                    self.flex_grow = Some(g);
                }
            }
            "flex-shrink" => {
                if let Some(g) = flex_factor(v) {
                    self.flex_shrink = Some(g);
                }
            }
            // `flex: 1` — сокращение для grow/shrink/basis; берём первое число.
            // `flex: <рост> <сжатие> <основа>` со всеми сокращёнными формами.
            // Раньше бралось только первое число, и `flex: 0 0 200px` терял
            // фиксированную основу — блок начинал растягиваться.
            "flex" => match v {
                "auto" => {
                    self.flex_grow = Some(1.0);
                    self.flex_shrink = Some(1.0);
                    self.flex_basis = Some(Len::Auto);
                }
                "none" => {
                    self.flex_grow = Some(0.0);
                    self.flex_shrink = Some(0.0);
                    self.flex_basis = Some(Len::Auto);
                }
                "initial" => {
                    self.flex_grow = Some(0.0);
                    self.flex_shrink = Some(1.0);
                    self.flex_basis = Some(Len::Auto);
                }
                _ => {
                    // Опущенные части сокращения берут НЕ начальные значения
                    // свойств: рост и сжатие становятся 1, а основа — 0%, а не
                    // `auto`. Отсюда весь смысл записи `flex: 1`: элемент
                    // делит место поровну, забыв свою ширину. Раньше основа при
                    // двух числах оставалась `auto`, и `flex: 0 1` держал
                    // ширину элемента вместо нуля.
                    let parts: Vec<&str> = v.split_whitespace().collect();
                    let number = |t: &str| flex_factor(t);
                    match parts.as_slice() {
                        [one] => match number(one) {
                            Some(g) => {
                                self.flex_grow = Some(g);
                                self.flex_shrink = Some(1.0);
                                self.flex_basis = Some(Len::Pct(0.0));
                            }
                            None => {
                                self.flex_grow = Some(1.0);
                                self.flex_shrink = Some(1.0);
                                self.flex_basis = Len::parse(one);
                            }
                        },
                        [a, b] => {
                            self.flex_grow = number(a);
                            match number(b) {
                                Some(shrink) => {
                                    self.flex_shrink = Some(shrink);
                                    self.flex_basis = Some(Len::Pct(0.0));
                                }
                                None => {
                                    self.flex_shrink = Some(1.0);
                                    self.flex_basis = Len::parse(b);
                                }
                            }
                        }
                        [a, b, c] => {
                            // Безразмерная основа кроме нуля делает ВСЁ
                            // объявление невалидным (`flex: 0 0 4` не
                            // применяется вовсе, flexbox_flex-*-unitless-basis).
                            if crate::style::values::value::number(c).is_some_and(|n| n != 0.0) {
                                return;
                            }
                            self.flex_grow = number(a);
                            self.flex_shrink = number(b);
                            // `content` — ключевое слово основы (css-flexbox-1 §7.2),
                            // а не длина: `Len::parse` его не знает, и `flex: 0 0
                            // content` падал в `auto` с заданной шириной
                            // (`flexbox-flex-basis-content-001b/002b`,
                            // `percentage-heights-016`). Смысл тот же, что у длинной
                            // формы `flex-basis: content` ниже.
                            if c.eq_ignore_ascii_case("content") {
                                self.flex_basis = Some(Len::Auto);
                                self.basis_content = Some(true);
                            } else {
                                self.flex_basis = Len::parse(c);
                            }
                        }
                        _ => {}
                    }
                }
            },
            "flex-basis" if v == "content" => {
                self.flex_basis = Some(Len::Auto);
                self.basis_content = Some(true);
            }
            "flex-basis" => {
                if let Some(l) = Len::parse(v)
                    && !matches!(l, Len::Px(x) | Len::Pct(x) if x < 0.0)
                {
                    self.flex_basis = Some(l);
                }
            }
            "align-self" if v.trim() == "inherit" => {
                self.align_self_inherit = true;
            }
            "align-items" | "justify-items" | "align-content" | "justify-content"
            | "justify-self"
                if v.trim() == "inherit" =>
            {
                self.align_inherit |= match key {
                    "align-items" => ainh::ALIGN_ITEMS,
                    "justify-items" => ainh::JUSTIFY_ITEMS,
                    "align-content" => ainh::ALIGN_CONTENT,
                    "justify-content" => ainh::JUSTIFY_CONTENT,
                    _ => ainh::JUSTIFY_SELF,
                };
            }
            "align-self" => {
                // `left`/`right` у `align-self` недействительны: это
                // `<self-position>` без них, физические стороны есть только у
                // `justify-self` (css-align-3 §6.1) — объявление отбрасывается
                // (`align-self-static-position-008`: `right` ждёт `start`;
                // `grid-abspos-staticpos-align-self-rtl-*`).
                let last = v.split_whitespace().last();
                if !matches!(last, Some("left") | Some("right"))
                    && let Ok(a) = align_keyword(v)
                {
                    self.align_self = a;
                    self.align_self_decl = Some(a);
                    self.align_self_inherit = false;
                    self.align_self_safe = is_safe(v);
                    self.align_self_normal = v.trim() == "normal";
                    self.align_self_own_axis = matches!(last, Some("self-start") | Some("self-end"));
                    self.align_self_flex_kw = matches!(last, Some("flex-start") | Some("flex-end"));
                    self.align_self_last = v.split_whitespace().any(|w| w == "last");
                }
            }
            "align-items" => {
                self.align_inherit &= !ainh::ALIGN_ITEMS;
                if let Ok(a) = align_keyword(v) {
                    self.align_items = a;
                    self.align_items_safe = is_safe(v);
                    self.align_items_last = v.split_whitespace().any(|w| w == "last");
                }
            }
            // `space-evenly` и `space-around` различаются шириной крайних
            // промежутков — сведение их в одно значение расходилось с
            // браузером на 27 точек (поймано сравнением).
            "justify-content" => {
                self.align_inherit &= !ainh::JUSTIFY_CONTENT;
                self.justify_content = parse_justify(v);
                self.justify_content_safe = is_safe(v);
            }
            "gap" => {
                // Куски — по пробелам ВНЕ скобок: `calc(15% + 7px) calc(10px +
                // 5%)` рвался на шесть кусков, и объявление молча отбрасывалось
                // (`grid-gutters-011/012`). Смесь долей с точками доживает
                // индексом (`parse_mixed`): зазор разрешает её от своей стороны
                // контент-бокса в `apply.rs` (css-gaps-1 §gap-percent).
                let tokens = split_outside_parens(v);
                let parts: Vec<Option<Len>> = tokens.iter().map(|t| Len::parse_mixed(t)).collect();
                self.gap = match parts.len() {
                    1 => Some((parts[0], parts[0])),
                    2 => Some((parts[0], parts[1])),
                    _ => self.gap,
                };
                // Короткая форма задаёт и `column-gap` многоколоночника
                // (css-align-3 §8.3: `gap` = `row-gap` + `column-gap`).
                // Колонки читают только `column_gap` (`render.rs` `used_gap`,
                // `column_flow`), и `gap: 20px 0` прежде оставлял кегль —
                // `column-wrap-no-constraints-001`, красная полоса между
                // колонками.
                // Многоколоночнику — прежний разбор без смеси: его зазор долю
                // с точками не читает.
                if parts.len() == 1 || parts.len() == 2 {
                    self.column_gap = Len::parse(&tokens[tokens.len() - 1]);
                }
            }
            "row-gap" => self.gap = Some((Len::parse(v), self.gap.and_then(|g| g.1))),
            // Одно свойство служит двум раскладкам: в сетке и гибкой строке
            // это зазор между ячейками, в многоколоночном потоке — между
            // колонками. Пишем в оба поля, читает нужное та раскладка, которая
            // включена.
            "column-gap" => {
                self.gap = Some((self.gap.and_then(|g| g.0), Len::parse(v)));
                self.column_gap = Len::parse(v);
            }
            "order" => self.order = v.parse().ok(),
            "flex-flow" => {
                for token in v.split_whitespace() {
                    let prop = if token.starts_with("wrap") || token == "nowrap" {
                        "flex-wrap"
                    } else {
                        "flex-direction"
                    };
                    self.apply_one(prop, token);
                }
            }
            "align-content" => {
                self.align_inherit &= !ainh::ALIGN_CONTENT;
                self.align_content = parse_justify(v);
                self.align_content_safe = is_safe(v);
                // css-align-3 §align-block: ЛЮБОЕ не-`normal` значение делает
                // блочный контейнер корнем блочного контекста форматирования.
                // `parse_justify` этого не покажет: `baseline`/`first`/`last`
                // дают `None` так же, как `normal`. Приставка `safe`/`unsafe`
                // снимается — она про переполнение, а не про значение.
                let word = v
                    .split_whitespace()
                    .find(|w| !matches!(*w, "safe" | "unsafe"))
                    .unwrap_or("");
                self.align_content_block =
                    self.align_content.is_some() || matches!(word, "baseline" | "first" | "last");
            }
            "justify-items" => {
                self.align_inherit &= !ainh::JUSTIFY_ITEMS;
                self.justify_items = parse_align(v);
                self.justify_items_safe = is_safe(v);
                self.justify_items_last = v.split_whitespace().any(|w| w == "last");
            }
            "justify-self" => {
                self.align_inherit &= !ainh::JUSTIFY_SELF;
                self.justify_self = parse_align(v);
                self.justify_self_physical = match v.split_whitespace().last() {
                    Some("left") => Some(false),
                    Some("right") => Some(true),
                    _ => None,
                };
                self.justify_self_normal = v.trim() == "normal";
                self.justify_self_safe = is_safe(v);
                self.justify_self_own_axis = matches!(
                    v.split_whitespace().last(),
                    Some("self-start") | Some("self-end")
                );
                self.justify_self_last = v.split_whitespace().any(|w| w == "last");
            }
            // `place-*` — сокращения «поперёк / вдоль»; одно значение задаёт обе оси.
            "place-items" | "place-content" | "place-self" => {
                let (a, b) = match v.split_once(char::is_whitespace) {
                    Some((a, b)) => (a.trim(), b.trim()),
                    None => (v, v),
                };
                let (cross, main) = match key {
                    "place-items" => ("align-items", "justify-items"),
                    "place-content" => ("align-content", "justify-content"),
                    _ => ("align-self", "justify-self"),
                };
                self.apply_one(cross, a);
                self.apply_one(main, b);
            }
            _ => *hit = false,
        }
    }
}
