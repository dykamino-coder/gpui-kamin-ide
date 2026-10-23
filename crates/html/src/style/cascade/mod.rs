//! Каскад: разрешение, применение объявлений, слои.

use crate::document::state::ValueCx;

use crate::style::cascade::defaults::initial_value;
use crate::style::cascade::vars::{CascadeCx, resolve_attrs, resolve_sibling, resolve_vars};
use crate::style::computed::{Computed, ParseResult, css_wide_keyword};
use crate::style::css::{CssWide, DeclBlock, LonghandId, Rule, ShorthandId, VarMap};

mod compute_cx;
pub mod defaults;
pub mod inherit;
pub(crate) mod inherited_value;
pub(crate) mod key;
pub(crate) mod map;
mod pass;
mod phase;
pub mod vars;
use map::CascadeMap;

impl Computed {
    /// Собрать стиль узла: правила таблицы (по специфичности), затем `style=""`.
    pub fn resolve(matched: &[&Rule], inline: &DeclBlock, vcx: ValueCx<'_>) -> Computed {
        Computed::resolve_with_vars(matched, inline, &VarMap::new(), vcx)
    }

    /// То же с переменными темы.
    pub fn resolve_with_vars(
        matched: &[&Rule],
        inline: &DeclBlock,
        vars: &VarMap,
        vcx: ValueCx<'_>,
    ) -> Computed {
        Computed::resolve_in(matched, inline, vars, CascadeCx::NONE, vcx)
    }

    /// То же для элемента `cx`: его атрибуты и место среди братьев видят
    /// `attr()` и `sibling-index()`.
    pub fn resolve_in(
        matched: &[&Rule],
        inline: &DeclBlock,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) -> Computed {
        let mut c = Computed::default();
        c.cascade_onto(matched, inline, vars, cx, vcx);
        c
    }

    /// Каскад правил `matched` и `style=""` поверх уже стоящих значений:
    /// один победитель на имя свойства по ключу каскада (css-cascade-5
    /// §6.2), откаты `revert`/`revert-layer` по истории кандидатов.
    /// Слои стиля (`::first-letter`, `:hover`) кладутся этим же путём на
    /// копию стиля хозяина.
    pub(crate) fn cascade_onto(
        &mut self,
        matched: &[&Rule],
        inline: &DeclBlock,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) {
        let map = CascadeMap::collect(matched, inline);
        self.apply_map(&map, vars, cx, vcx);
        // Окраска фильтром здесь НЕ делается: результат оседал в
        // долгоживущем стиле узла, и покадровая окраска в `style::cascade::inherit`
        // применяла фильтр ВТОРОЙ раз (grayscale темнил вдвое). Единственная
        // точка окраски — слияние при отрисовке.
    }

    // `pub(crate)`: `motion` синтезирует строку `transform` и кормит её тем же
    // разборщиком — отдельного конвейера под offset-трансформ нет.
    pub(crate) fn apply_one(&mut self, key: &str, val: &str, vcx: ValueCx<'_>) {
        let v = val.trim();
        // Устаревшее имя — то же свойство (css-cascade-5 §2.1), как при
        // разборе блока.
        let key = crate::style::css::legacy_name_alias(key).unwrap_or(key);
        // Сокращение реестра вне разбора блока (прямой вызов, слово `all`
        // не в счёт: оно длинные свойства называет сами) раскрывается тем же
        // разбором, что и в блоке.
        if let Some(shorthand) = ShorthandId::from_name(key) {
            for (id, _, text) in shorthand.expand(v, Some(vcx)).unwrap_or_default() {
                self.apply_one(id.name(), &text, vcx);
            }
            return;
        }
        // Общее слово у длинного свойства реестра решается ОДНИМ путём
        // (`apply_wide`); наблюдатель разбора ниже — для свойств вне реестра.
        if let Some(id) = LonghandId::from_name(key) {
            if let Some(wide) = CssWide::parse(v) {
                return self.apply_wide(id, wide, vcx);
            }
            // Наследуемые свойства вне шрифта и текста: один разбор на
            // свойство (`props::inherited`), обработчики по имени их не видят.
            if crate::style::computed::props::inherited::owns(id) {
                return self.apply_inherited_longhand(id, v, vcx);
            }
        }
        // Общие слова `initial`/`unset`/`revert` у свойства ВНЕ реестра: все
        // такие свойства не наследуются, и `unset` у них — `initial`
        // (css-cascade-5 §7.3.3).
        if matches!(v, "initial" | "unset" | "revert" | "revert-layer")
            && let Some(start) = initial_value(key)
        {
            return self.apply_one(key, start, vcx);
        }
        // Начальные значения, которые годятся ТОЛЬКО для `initial`/`unset`:
        // `revert` у автора откатывает к таблице агента, а там у `div`
        // `display: block`, не начальное `inline`.
        if matches!(v, "initial" | "unset")
            && let Some(start) = match key {
                "display" => Some("inline"),
                "float" => Some("none"),
                "position" => Some("static"),
                "opacity" => Some("1"),
                _ => None,
            }
        {
            return self.apply_one(key, start, vcx);
        }
        if inherited_value::note(self, key, v, vcx) {
            inherit::note_flow_align(self, key, v);
            return;
        }
        // Наблюдатель разбора (`@supports`, запасной кандидат каскада) и
        // общее слово для свойств ВНЕ реестра: их грамматику проверяет
        // обработчик, а не разбор блока. Длинные свойства реестра пришли
        // проверенными и общее слово решили выше (`apply_wide`).
        let observer = self.declaration_parse;
        self.declaration_parse = ParseResult::Invalid;
        // Каждое свойство принимает CSS-wide keyword (css-cascade-5 §7.3); не
        // покрытое таблицами выше слово сбрасывает поля свойства к пустому,
        // начальному представлению, а не отвергается грамматикой значения.
        let css_wide = css_wide_keyword(v);
        let outer = std::mem::replace(&mut self.css_wide_value, css_wide);
        self.apply_property(key, v, vcx);
        self.css_wide_value = outer;
        let accepted = self.declaration_parse == ParseResult::Accepted || css_wide;
        self.declaration_parse = if observer == ParseResult::Inactive {
            observer
        } else if accepted {
            ParseResult::Accepted
        } else {
            observer
        };
        if accepted {
            inherit::note_flow_align(self, key, v);
        }
    }

    /// Общее слово у длинного свойства реестра (css-cascade-5 §7.3):
    /// `initial` — начальное значение реестра, `inherit` — наследование
    /// обработчиком, `unset` — по наследуемости свойства. `revert` и
    /// `revert-layer` каскад разрешает сам; вне каскада откатывать некуда,
    /// и они значат `unset` (§7.3.4).
    pub(super) fn apply_wide(&mut self, id: LonghandId, wide: CssWide, vcx: ValueCx<'_>) {
        let inherit = match wide {
            CssWide::Inherit => true,
            CssWide::Initial => false,
            CssWide::Unset | CssWide::Revert | CssWide::RevertLayer => id.inherited(),
        };
        // Наследование ненаследуемого — копия полей родителя по реестру
        // полей `inherited_value`; у наследуемого свои поля пусты (значение
        // придёт слиянием с родителем) — очистку знает модуль свойства или
        // его обработчик.
        if inherit {
            if inherited_value::note(self, id.name(), "inherit", vcx) {
                // Записано в `explicit_inherit`.
            } else if crate::style::computed::props::inherited::owns(id) {
                self.inherit_longhand(id);
            } else {
                self.apply_property(id.name(), "inherit", vcx);
            }
        } else {
            self.apply_one(id.name(), id.initial(), vcx);
            // Quirks Mode §3.12: таблица с НАЧАЛЬНЫМ цветом из листа агента
            // берёт цвет `<body>` (`dom::quirks_color`).
            if id == LonghandId::Color {
                self.color_initial = true;
            }
        }
        // Откаченное объявление — уже не авторское: признак авторского фона
        // за ним не остаётся, и неявный фон агента у поля ввода живёт
        // (`appearance-revert-001`).
        if matches!(wide, CssWide::Revert | CssWide::RevertLayer)
            && id == LonghandId::BackgroundColor
        {
            self.bg_explicit = false;
        }
    }

    /// Query the actual property dispatcher, including delegated property groups.
    pub(crate) fn recognizes_property(key: &str, vcx: ValueCx<'_>) -> bool {
        let key = crate::style::css::legacy_name_alias(key).unwrap_or(key);
        ShorthandId::from_name(key).is_some()
            || LonghandId::from_name(key).is_some()
            || Self::default().apply_property(key, "", vcx)
    }

    // The temporary cascade callback borrows the current property and variable state.
    #[allow(clippy::type_complexity)]
    fn apply_property(&mut self, key: &str, v: &str, vcx: ValueCx<'_>) -> bool {
        let groups: [fn(&mut Self, &str, &str, &mut bool, ValueCx<'_>); 14] = [
            Self::apply_scroll,
            Self::apply_display_flex,
            Self::apply_grid,
            Self::apply_box_model,
            Self::apply_border,
            Self::apply_position,
            Self::apply_background,
            Self::apply_mask_clip,
            Self::apply_font,
            Self::apply_text,
            Self::apply_text_decor,
            Self::apply_multicol,
            Self::apply_transform,
            Self::apply_effects,
        ];
        for group in groups {
            let mut hit = true;
            group(self, key, v, &mut hit, vcx);
            if hit {
                return true;
            }
        }
        false
    }
}
