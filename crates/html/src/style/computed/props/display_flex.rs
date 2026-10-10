//! Computed::apply_one: display, flex*, align-*/justify-*/place-*, gap, order.

use crate::style::computed::*;
use crate::style::values::value::Len;
mod gap_place;
mod flex_items;

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
                    "ruby"
                    | "inline ruby"
                    | "ruby-base"
                    | "ruby-text"
                    | "ruby-base-container"
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
            _ => self.apply_flex_items(key, val, v, hit),
        }
    }
}
