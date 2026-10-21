//! Группа свойств `Computed::apply_one`: `display`, `flex*`,
//! `align-*`/`justify-*`/`place-*`, `gap`, `order`. Здесь — разбор `display`
//! во все виды коробок (флекс, сетка, лунки, табличные роли, руби, `run-in`,
//! `flow-root`, `contents`, двухсловные записи CSS Display 3) с сопутствующими
//! метками (`inline_display`, `ruby_role`, `col_role`…) и `flex-direction`;
//! элементы флекса, щели и `place-*` — в подмодулях.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::Len;
mod flex_items;
mod gap_place;

impl Computed {
    pub(crate) fn apply_display_flex(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "display"
                if v.trim().eq_ignore_ascii_case("-webkit-box")
                    || v.trim().eq_ignore_ascii_case("-webkit-inline-box") =>
            {
                self.webkit_box = accept_parsed!(self, Some(true));
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
                let mut lanes_inline = self.lanes_inline;
                let mut flow_root = self.flow_root;
                let mut is_caption = self.is_caption;
                let mut inline_display = None;
                let math_layout = match v {
                    "math" | "inline math" => Some(Display::InlineBlock),
                    "block math" => Some(Display::Block),
                    _ => None,
                };
                // Каскад мог поставить группу выше по важности, а ниже —
                // обычный блок: метка рода не переживает своё значение.
                let mut row_group_kind = None;
                let mut col_role = None;
                // Роль руби живёт вместе со значением `display`: более
                // важное `display: block` на `span.rt` снимает её.
                let mut ruby_role = None;
                self.display = apply_parsed!(
                    self,
                    match v {
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
                            lanes_inline = true;
                            Some(Display::GridLanes)
                        }
                        // CSS Display 3 separates the outer inline mode from
                        // the inner grid or flex formatting context.
                        "inline grid" => Some(Display::InlineGrid),
                        "inline flex" => Some(Display::InlineFlex),
                        "none" => Some(Display::None),
                        "block" => Some(Display::Block),
                        "inline-block" => Some(Display::InlineBlock),
                        "inline" => {
                            // Метка «настоящий строчный»: блокификация под
                            // float/abspos (§9.7) и запрет width/height на
                            // незамещаемом (§10.2) решаются после каскада.
                            inline_display = Some(true);
                            Some(Display::InlineBlock)
                        }
                        "math" | "inline math" => Some(Display::InlineBlock),
                        "block math" => Some(Display::Block),
                        "inline-grid" => Some(Display::InlineGrid),
                        // Элемент исчезает, дети встают на его место.
                        "contents" => Some(Display::Contents),
                        "list-item" => Some(Display::ListItem),
                        // CSS Display 3: an inline list item shares its parent's
                        // inline formatting context; its marker precedes its text.
                        words
                            if {
                                let mut parts: Vec<_> = words.split_whitespace().collect();
                                parts.sort_unstable();
                                parts == ["inline", "list-item"]
                                    || parts == ["flow", "inline", "list-item"]
                            } =>
                        {
                            inline_display = Some(true);
                            Some(Display::ListItem)
                        }
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
                            ruby_role = Some(match v {
                                "ruby-base" => RubyRole::Base,
                                "ruby-text" => RubyRole::Text,
                                "ruby-base-container" => RubyRole::BaseContainer,
                                "ruby-text-container" => RubyRole::TextContainer,
                                _ => RubyRole::Container,
                            });
                            inline_display = Some(true);
                            Some(Display::InlineBlock)
                        }
                        "block ruby" => {
                            ruby_role = Some(RubyRole::Container);
                            Some(Display::Block)
                        }
                        "table-row-group" | "table-header-group" | "table-footer-group" => {
                            row_group_kind = Some(match v {
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
                            flow_root = Some(true);
                            Some(Display::Block)
                        }
                        "table-row" => Some(Display::TableRow),
                        "table-cell" => Some(Display::TableCell),
                        // Заголовок таблицы — обычный блок. Колонки коробок не
                        // порождают вовсе: они только задают ширину столбцам.
                        "table-caption" => {
                            // Заголовок — блочная коробка с МЕТКОЙ: таблица ищет
                            // его по ней, а не только по тегу caption.
                            is_caption = Some(true);
                            Some(Display::Block)
                        }
                        // Колонка коробки НЕ порождает (§17.2.1): `Display`
                        // остаётся `None`. Метка живёт отдельно — по ней узел
                        // переживает разбор дерева, и только по ней его находит
                        // таблица.
                        "table-column" | "table-column-group" => {
                            col_role = Some(u8::from(v == "table-column-group"));
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
                            w == ["flow-root", "list-item"]
                                || w == ["block", "flow-root", "list-item"]
                        } =>
                        {
                            flow_root = Some(true);
                            Some(Display::ListItem)
                        }
                        // Grid-lanes and masonry share the inner layout;
                        // retain inline when it is the outer display mode.
                        two if two
                            .split_whitespace()
                            .any(|w| w == "grid-lanes" || w == "masonry") =>
                        {
                            if two.split_whitespace().any(|w| w == "inline") {
                                lanes_inline = true;
                            }
                            Some(Display::GridLanes)
                        }
                        _ => None,
                    }
                )
                .filter(|_| math_layout.is_none())
                .or(self.display);
                self.inline_display = inline_display;
                self.math_layout = math_layout;
                self.row_group_kind = row_group_kind;
                self.col_role = col_role;
                self.ruby_role = ruby_role;
                self.lanes_inline = lanes_inline;
                self.flow_root = flow_root;
                self.is_caption = is_caption;
            }
            "flex-direction" => {
                self.flex_dir = apply_parsed!(
                    self,
                    match v {
                        "row" => Some(FlexDir::Row),
                        "row-reverse" => Some(FlexDir::RowReverse),
                        "column" => Some(FlexDir::Col),
                        "column-reverse" => Some(FlexDir::ColReverse),
                        _ => None,
                    }
                )
                .or(self.flex_dir)
            }
            _ => self.apply_flex_items(key, v, hit, vcx),
        }
    }
}
