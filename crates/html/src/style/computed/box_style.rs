//! Стиль коробки: вычисленный стиль элемента (общий, `Rc`, копия при записи) и контекст коробки —
//! то, что знает только дерево коробок: содержащий блок и его определённость, родитель-сетка/flex,
//! предки трансформа и наложения, распространённые линии оформления (css-text-decor-3 §2.1), ключи
//! коробки в реестрах кадра. Копия стиля коробки не копирует вычисленный стиль (P-18; Servo:
//! `Arc<ComputedValues>` и фрагмент; Blink: `ComputedStyle` и `LayoutObject`).

use crate::style::computed::*;
use std::rc::Rc;

mod background;
mod context;
mod inputs;
use crate::box_facts::BoxContext;
use crate::style::box_input::BoxStyleInput;

mod inline_format;
pub(crate) use inline_format::InlineFormat;

/// Вычисленный стиль элемента и его вариант от дополнительного источника каскада.
/// Идентичность исходного стиля связывает потомка с родителем-элементом даже при
/// перестройке дерева коробок. Обе ссылки остаются свободны от used-правок.
#[derive(Clone, Debug)]
pub(crate) struct ElementStyle {
    pub original: Rc<Computed>,
    pub computed: Rc<Computed>,
    /// Активный вариант предка: сохраняет источник через анонимные обёртки
    /// и перенесённые коробки, не сохраняя всё обычное дерево элементов.
    pub parent: Option<Rc<ElementStyle>>,
}

impl ElementStyle {
    pub(crate) fn variant_for(&self, original: &Rc<Computed>) -> Option<&Rc<Computed>> {
        std::iter::successors(Some(self), |p| p.parent.as_deref())
            .find(|p| Rc::ptr_eq(&p.original, original))
            .filter(|p| !Rc::ptr_eq(&p.original, &p.computed))
            .map(|p| &p.computed)
    }

    pub(crate) fn active(&self) -> Option<Rc<Self>> {
        if Rc::ptr_eq(&self.original, &self.computed) {
            self.parent.clone()
        } else {
            Some(Rc::new(self.clone()))
        }
    }
}

/// Стиль одной коробки.
#[derive(Clone, Debug, Default)]
pub struct BoxStyle {
    /// Факты коробки-родителя отдельно от вычисленных значений элемента.
    pub(crate) context: BoxContext<super::writing_mode::WritingMode, super::Align>,
    /// Собственный источник холста, отдельно от computed CSS.
    pub(crate) canvas_background: Option<Rc<crate::style::canvas_background::CanvasBackground>>,
    /// Собственный used-фон коробки, отдельно от наследуемого computed CSS.
    pub(crate) background_paint: crate::style::background_paint::BackgroundPaint,
    /// Физическая реализация текущего строчного контекста, отдельно от CSS элемента.
    pub(crate) inline_format: Option<InlineFormat>,
    /// Оформление разрезанного inline, не вычисленные CSS-значения.
    pub(crate) inline_decoration: crate::style::inline_decoration::InlineDecoration,
    /// Вычисленный стиль (с used-значениями раскладки); общий для коробок элемента.
    style: Rc<Computed>,
    /// Computed-стадия элемента отдельно от стиля его коробки; у анонимных коробок её нет.
    pub(crate) element_style: Option<ElementStyle>,
    /// Стиль элемента с вариантами `position-try-fallbacks`: вариант вычисляется
    /// заново из его каскада (`resolve::try_rule_onto`).
    pub(crate) try_style: Option<crate::dom::SharedStyle>,
    /// Служебные поля якорного шага, ставит `render::element`: свой `node_id`
    /// (ключ реестра содержащих блоков `anchor::CB`), `node_id` ближайшего
    /// содержащего блока абсолюта (`style::cascade::inherit`; 0 — начальный, окно),
    /// порядковый номер сборки в кадре («последний якорь раньше по дереву» в
    /// реестре прошлого кадра) и ключ коробки в реестре размеров клетки.
    pub(crate) self_node: u64,
    pub(crate) cb_node: u64,
    /// Used classic gutters of the nearest containing block and viewport.
    pub(crate) cb_scroll_insets: ([f32; 4], [f32; 4]),
    pub(crate) anchor_seq: u32,
    pub(crate) anchor_key: u64,
    /// Коробка РАСТЯНУТА раскладкой: элемент гибкого контейнера или сетки без
    /// своей высоты получает её от полосы, и для потомков она определена.
    pub(crate) stretched: bool,
    /// Есть ли выше по дереву коробка, устанавливающая содержащий блок для
    /// внепоточных потомков (§10.1 п.4). Ставится при наследовании: сам
    /// каскад предков не видит.
    pub(crate) cb_ancestor: bool,
    /// How many ancestors establish a containing block for absolute
    /// descendants (`establishes_cb`). An absolute box's nearest CB is the
    /// ancestor with index `cb_depth + establishes_cb(parent)`; strict CB
    /// layers (`cb_open_strict`) take only boxes whose nearest CB they are.
    pub(crate) cb_depth: u16,
    /// Есть ли выше по дереву НЕКОРНЕВОЙ контекст наложения (CSS 2.1 прил. E):
    /// без него отрицательный `z-index` принадлежит корневому контексту и
    /// рисуется под всем его потоком. Ставится при наследовании.
    pub(crate) sc_ancestor: bool,
    /// Definite content inline size (physical height) of a box in vertical flow: the basis of
    /// inline-axis percentages of its children (`layout::writing_mode::inline_definite`).
    /// Set by `render::element`; not inherited.
    pub(crate) inline_definite: Option<f32>,
    /// ОПРЕДЕЛЕНА ли высота содержащего блока: от неё зависит, считается ли
    /// доля высоты вообще (§10.5 — иначе значение равно `auto`).
    pub(crate) cb_height_def: bool,
    /// Режим quirks: высота, от которой ДЕТИ этой коробки решают долю
    /// высоты при неопределённом содержащем блоке (Quirks Mode §3.5 «The
    /// percentage height calculation quirk» — ближайший предок с не-`auto`
    /// высотой). `None` — квирка нет или опоры не нашлось.
    pub(crate) quirk_pct_base: Option<f32>,
}

impl BoxStyle {
    pub(crate) fn set_parent_context(&mut self, parent: &Self) {
        self.context = context::child_of(parent);
    }

    /// Коробка со стилем `style` и пустым контекстом.
    pub fn new(style: Computed) -> Self {
        Self::shared(Rc::new(style))
    }

    /// Коробка с общим вычисленным стилем и переданными фактами родителя.
    pub fn shared(input: impl Into<BoxStyleInput>) -> Self {
        let BoxStyleInput {
            style,
            context,
            inline_decoration,
            background_paint,
            canvas_background,
        } = input.into();
        // Определённость высоты и квирк-опора доли, которые раскладка положила
        // в стиль элемента-копии, — исходные значения контекста его коробки.
        Self {
            context: context.as_deref().cloned().unwrap_or_default(),
            inline_decoration,
            background_paint,
            canvas_background,
            cb_height_def: style.cb_height_def_hint,
            quirk_pct_base: style.quirk_pct_base_hint,
            style,
            ..Self::default()
        }
    }

    /// Та же коробка (контекст) с другим вычисленным стилем `style`.
    pub fn restyled(&self, style: Computed) -> Self {
        Self {
            style: Rc::new(style),
            ..self.clone()
        }
    }

    /// `Computed::resolve_viewport` без копии общего стиля, когда решать нечего.
    pub fn resolve_viewport(
        &mut self,
        viewport: (f32, f32),
        vcx: crate::document::state::ValueCx<'_>,
    ) {
        if self.style.has_viewport_lengths(vcx) {
            Rc::make_mut(&mut self.style).resolve_viewport(viewport, vcx);
        }
    }

    /// `Computed::resolve_decor_inset_viewport` без копии общего стиля, когда решать нечего.
    pub(crate) fn resolve_decor_inset_viewport(
        &mut self,
        viewport: (f32, f32),
        vcx: crate::document::state::ValueCx<'_>,
    ) {
        if self.style.has_decor_insets() {
            Rc::make_mut(&mut self.style).resolve_decor_inset_viewport(viewport, vcx);
        }
    }

    /// Вычисленный стиль коробки — общий указатель.
    pub fn style_rc(&self) -> &Rc<Computed> {
        &self.style
    }
}

impl BoxStyle {
    /// Вычисленный стиль без контекста коробки (стиль элемента, сделанного раскладкой).
    pub fn into_computed(self) -> Computed {
        Rc::unwrap_or_clone(self.style)
    }
}

impl From<&BoxStyle> for BoxStyleInput {
    fn from(style: &BoxStyle) -> Self {
        Self::new(
            Rc::clone(style.style_rc()),
            Some(Rc::new(style.context.clone())),
            style.inline_decoration,
            style.background_paint,
            style.canvas_background.clone(),
        )
    }
}

impl From<BoxStyle> for BoxStyleInput {
    fn from(style: BoxStyle) -> Self {
        let context = Rc::new(style.context.clone());
        let inline_decoration = style.inline_decoration;
        Self::new(
            style.style,
            Some(context),
            inline_decoration,
            style.background_paint,
            style.canvas_background,
        )
    }
}

impl From<Computed> for BoxStyle {
    fn from(style: Computed) -> Self {
        Self::new(style)
    }
}

impl std::ops::Deref for BoxStyle {
    type Target = Computed;

    fn deref(&self) -> &Computed {
        &self.style
    }
}

/// TRANSITIONAL(cascade-7b-R10): Used values остаются в Computed, поэтому differing write клонирует весь стиль; удалить общий storage после перевода paint и layout-made readers на BoxStyle.
impl std::ops::DerefMut for BoxStyle {
    fn deref_mut(&mut self) -> &mut Computed {
        Rc::make_mut(&mut self.style)
    }
}
