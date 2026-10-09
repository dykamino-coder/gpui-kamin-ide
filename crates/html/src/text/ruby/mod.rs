//! Руби: сегменты и уровни.
// owner: A

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::text::text_box::blank_text;

pub(crate) mod ruby_hiding;
pub(crate) mod ruby_transform;
pub(crate) mod container;

/// Единица руби (css-ruby-1 §2.3.2): содержимое одной базы или одной
/// аннотации. Пустой вектор — анонимная пустая единица, добавленная спариванием.
pub(crate) type RubyUnit = Vec<Node>;

/// Уровень аннотаций сегмента: `<rtc>` или ряд `<rt>` прямо в контейнере
/// (анонимный контейнер аннотаций, css-ruby-1 §2.2 п.8).
pub(crate) struct RubyLevel {
    pub(crate) units: Vec<RubyUnit>,
    /// `<rtc>` без `<rt>` внутри — одна анонимная аннотация, накрывающая ВСЕ
    /// базы сегмента (§2.3.2 «spanning annotation»).
    pub(crate) spanning: bool,
    /// Стиль самого `<rtc>`: его аннотации наследуют от него (в том числе
    /// половинный кегль из листа агента).
    pub(crate) container: Option<Computed>,
}

/// Сегмент руби (css-ruby-1 §2.3.1): ряд баз и уровни аннотаций к нему.
pub(crate) struct RubySegment {
    pub(crate) bases: Vec<RubyUnit>,
    pub(crate) levels: Vec<RubyLevel>,
}

/// Пуста ли единица: только схлопываемые пробелы и руби-теги без содержимого.
/// Любой другой элемент — содержимое, даже пустой `<div>` с шириной
/// (`ruby-align-001`: `rt > div { width: 160px }`).
pub(crate) fn ruby_unit_blank(unit: &[Node]) -> bool {
    unit.iter().all(|n| match n {
        Node::Text(t) => blank_text(t),
        Node::Element(k) if ruby_role(k).is_some_and(|r| r != crate::style::computed::RubyRole::Container) => {
            ruby_unit_blank(&k.children)
        }
        Node::Element(_) => false,
    })
}

/// Роль элемента в руби (css-ruby-1 §2.1): своё `display: ruby*`, иначе —
/// тег (A.1: `ruby/rb/rt/rbc/rtc`). Авторский `display` на руби-теге роль
/// СНИМАЕТ (`display: block` на `<rt>` — обычный блок, как в Blink, где
/// `IsInlineRubyText` смотрит на `Display()`, а не на тег): роль по тегу
/// действует только без своего `display`.
pub(crate) fn ruby_role(e: &Element) -> Option<crate::style::computed::RubyRole> {
    use crate::style::computed::RubyRole;
    if let Some(role) = e.style.ruby_role {
        return Some(role);
    }
    if e.style.display.is_some() {
        return None;
    }
    match e.tag.as_str() {
        "ruby" => Some(RubyRole::Container),
        "rb" => Some(RubyRole::Base),
        "rt" => Some(RubyRole::Text),
        "rbc" => Some(RubyRole::BaseContainer),
        "rtc" => Some(RubyRole::TextContainer),
        _ => None,
    }
}

/// Разрезать детей `<ruby>` на сегменты и единицы (css-ruby-1 §2.2 п.3-8, §2.3).
///
/// База после аннотации открывает новый сегмент. Пробелы между руби-коробками
/// решаются по соседям (§2.2 п.4-6): краевые и межуровневые (база →
/// аннотация) выбрасываются; между двумя `<rb>` — своя база, между двумя
/// `<rt>` — своя аннотация; аннотация → база — межсегментный, свой сегмент из
/// одной анонимной базы (так его рисуют эталоны `ruby-box-generation-*`).
/// Прочий строчный контент образует анонимную базу (п.3). `<rp>` не
/// показывается (A.1). Аннотации внутри `<rbc>` (неправильно вложенные, п.2)
/// пока идут содержимым базы — анонимный руби-контейнер для них: шаг 3.
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v146, `scout-ruby-2026-09d.md` шаг 4):
/// дополнительный лидинг §3.4 полем на атом руби (`ruby_leading` +
/// `row.mt/mb`). Срез css-ruby+css-transforms+css-masking+filter-effects+
/// css-writing-modes+css-inline+css-overflow+css-break 3936: −7 —
/// `ruby-align-001/001a/space-around` (0.05…0.08 → 0.69…1.01),
/// `rt-display-001` (0.01 → 0.62), `ruby-lang-specific-style-001`,
/// `ruby-overhang-none`, `ruby-tab-in-base-002`; обещанных плюсов срез
/// не показал. Поле на атоме растит короб строки, но и сдвигает базу
/// относительно соседей — нужен настоящий лидинг строки, а не поле.
pub(crate) fn ruby_segments(children: &[Node]) -> Vec<RubySegment> {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Text,
        Rb,
        Rbc,
        Rt,
        Rtc,
        Blank,
        Drop,
    }
    // Вид — по РОЛИ (тег или `display: ruby*`, `ruby_role`): `span.rt
    // { display: ruby-text }` — аннотация (`rt-display-001`), `span#rbc
    // { display: ruby-base-container }` — контейнер баз (`rbc-rtc-basic-001`).
    let kind_of = |n: &Node| match n {
        Node::Text(t) if blank_text(t) => Kind::Blank,
        Node::Text(_) => Kind::Text,
        Node::Element(k) if k.tag == "rp" => Kind::Drop,
        Node::Element(k) => match ruby_role(k) {
            Some(crate::style::computed::RubyRole::Base) => Kind::Rb,
            Some(crate::style::computed::RubyRole::BaseContainer) => Kind::Rbc,
            Some(crate::style::computed::RubyRole::Text) => Kind::Rt,
            Some(crate::style::computed::RubyRole::TextContainer) => Kind::Rtc,
            _ => Kind::Text,
        },
    };
    let base_kind = |k: Kind| matches!(k, Kind::Text | Kind::Rb | Kind::Rbc);
    let ann_kind = |k: Kind| matches!(k, Kind::Rt | Kind::Rtc);
    let kinds: Vec<Kind> = children.iter().map(kind_of).collect();
    // Ближайший непробельный сосед слева (`-1`) или справа (`1`).
    let neighbour = |from: usize, step: isize| -> Option<Kind> {
        let mut i = from as isize + step;
        while i >= 0 && (i as usize) < kinds.len() {
            let k = kinds[i as usize];
            if !matches!(k, Kind::Blank | Kind::Drop) {
                return Some(k);
            }
            i += step;
        }
        None
    };
    fn fresh() -> RubySegment {
        RubySegment { bases: Vec::new(), levels: Vec::new() }
    }
    fn flush_run(run: &mut RubyUnit, cur: &mut RubySegment) {
        if !run.is_empty() {
            cur.bases.push(std::mem::take(run));
        }
    }
    fn close_segment(cur: &mut RubySegment, out: &mut Vec<RubySegment>) {
        if !cur.bases.is_empty() || !cur.levels.is_empty() {
            out.push(std::mem::replace(cur, fresh()));
        }
    }
    // База после аннотаций — новый сегмент (§2.3.1). Явный `<rbc>` — свой
    // контейнер баз, а сегмент — ОДИН контейнер баз с аннотациями за ним
    // (§2.3.1): база после `<rbc>` его не продолжает, даже без аннотаций.
    // Прежде `<rbc>e</rbc><rbc>f</rbc><rbc>g</rbc><rtc>h</rtc>` склеивались
    // в один сегмент, и `h` вставала над `e`, а не над `g`
    // (`ruby-box-generation-001-ref`).
    fn base_starts(cur: &mut RubySegment, out: &mut Vec<RubySegment>, loose_level: &mut bool, sealed: &mut bool) {
        if !cur.levels.is_empty() || *sealed {
            close_segment(cur, out);
            *loose_level = false;
        }
        *sealed = false;
    }
    let mut out: Vec<RubySegment> = Vec::new();
    let mut cur = fresh();
    // Анонимная база из текста и строчных элементов (§2.2 п.3).
    let mut run: RubyUnit = Vec::new();
    // Открыт ли анонимный уровень из `<rt>` прямо в контейнере.
    let mut loose_level = false;
    // Базы текущего сегмента пришли из явного `<rbc>`.
    let mut sealed = false;
    for (i, node) in children.iter().enumerate() {
        match kinds[i] {
            Kind::Drop => {}
            Kind::Text => {
                base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                run.push(node.clone());
            }
            Kind::Rb => {
                base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                flush_run(&mut run, &mut cur);
                cur.bases.push(vec![node.clone()]);
            }
            Kind::Rbc => {
                base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                flush_run(&mut run, &mut cur);
                // Анонимные базы перед `<rbc>` — свой сегмент.
                close_segment(&mut cur, &mut out);
                loose_level = false;
                sealed = true;
                let Node::Element(k) = node else { continue };
                // Внутри `<rbc>`: каждый `<rb>` — база, пробел между двумя
                // `<rb>` — своя база, краевые пробелы — вон, прочее —
                // анонимная база.
                let kids: Vec<Kind> = k.children.iter().map(kind_of).collect();
                let mut inner: RubyUnit = Vec::new();
                for (j, c) in k.children.iter().enumerate() {
                    match kids[j] {
                        Kind::Rb => {
                            if !inner.is_empty() {
                                cur.bases.push(std::mem::take(&mut inner));
                            }
                            cur.bases.push(vec![c.clone()]);
                        }
                        Kind::Blank => {
                            let prev = kids[..j].iter().rev().copied().find(|k| *k != Kind::Blank);
                            let next = kids[j + 1..].iter().copied().find(|k| *k != Kind::Blank);
                            match (prev, next) {
                                (Some(Kind::Rb), Some(Kind::Rb)) => cur.bases.push(vec![c.clone()]),
                                (Some(Kind::Text), Some(_)) | (Some(_), Some(Kind::Text)) => {
                                    inner.push(c.clone())
                                }
                                _ => {}
                            }
                        }
                        Kind::Drop => {}
                        _ => inner.push(c.clone()),
                    }
                }
                if !inner.is_empty() {
                    cur.bases.push(inner);
                }
            }
            Kind::Rt => {
                flush_run(&mut run, &mut cur);
                if !loose_level {
                    cur.levels.push(RubyLevel { units: Vec::new(), spanning: false, container: None });
                    loose_level = true;
                }
                cur.levels.last_mut().expect("уровень только что открыт").units.push(vec![node.clone()]);
            }
            Kind::Rtc => {
                flush_run(&mut run, &mut cur);
                loose_level = false;
                let Node::Element(k) = node else { continue };
                let rts: Vec<RubyUnit> = k
                    .children
                    .iter()
                    .filter(|c| {
                        matches!(c, Node::Element(r)
                            if ruby_role(r) == Some(crate::style::computed::RubyRole::Text))
                    })
                    .map(|c| vec![c.clone()])
                    .collect();
                let spanning = rts.is_empty();
                let units = if spanning {
                    // Одна анонимная аннотация из всего содержимого, без
                    // краевых пробелов (§2.2 п.4).
                    let mut all: Vec<Node> = k.children.clone();
                    while matches!(all.first(), Some(Node::Text(t)) if blank_text(t)) {
                        all.remove(0);
                    }
                    while matches!(all.last(), Some(Node::Text(t)) if blank_text(t)) {
                        all.pop();
                    }
                    vec![all]
                } else {
                    rts
                };
                cur.levels.push(RubyLevel { units, spanning, container: Some(k.style.clone()) });
            }
            Kind::Blank => match (neighbour(i, -1), neighbour(i, 1)) {
                // Краевой пробел контейнера (п.4).
                (None, _) | (_, None) => {}
                // Межуровневый: база → аннотация (п.5).
                (Some(p), Some(n)) if base_kind(p) && ann_kind(n) => {}
                // Пробел у явного `<rbc>` — между двумя контейнерами баз, то
                // есть межсегментный (п.6): свой сегмент, иначе аннотация
                // после следующей базы спарилась бы с ним (эталон
                // `ruby-box-generation-001`: `<rbc><rb><span> </span></rb></rbc>`).
                (Some(p @ (Kind::Rb | Kind::Rbc)), Some(n @ (Kind::Rb | Kind::Rbc)))
                    if p == Kind::Rbc || n == Kind::Rbc =>
                {
                    flush_run(&mut run, &mut cur);
                    close_segment(&mut cur, &mut out);
                    loose_level = false;
                    sealed = false;
                    out.push(RubySegment { bases: vec![vec![node.clone()]], levels: Vec::new() });
                }
                // Межбазовый (п.6): своя единица, спаривается по порядку.
                (Some(Kind::Rb | Kind::Rbc), Some(Kind::Rb | Kind::Rbc)) => {
                    base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                    flush_run(&mut run, &mut cur);
                    cur.bases.push(vec![node.clone()]);
                }
                // Пробел внутри анонимной базы.
                (Some(p), Some(n)) if base_kind(p) && base_kind(n) => {
                    base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                    run.push(node.clone());
                }
                // Межаннотационный (п.6) — только между двумя `<rt>` контейнера.
                (Some(Kind::Rt), Some(Kind::Rt)) if loose_level => {
                    cur.levels.last_mut().expect("уровень открыт").units.push(vec![node.clone()]);
                }
                // Аннотация → строчное содержимое: пробел открывает анонимную
                // базу следующего сегмента вместе с этим содержимым (§2.2
                // п.3: анонимная база оборачивает ПОДРЯД идущие строчные
                // коробки, пробел — тоже строчный текст). Эталон
                // `ruby-box-generation-001` так и пишет:
                // `<rb><span> <span>l</span> </span></rb>`.
                (Some(p), Some(Kind::Text)) if ann_kind(p) => {
                    base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                    run.push(node.clone());
                }
                // Межсегментный (п.6): аннотация → база — свой сегмент.
                (Some(p), Some(n)) if ann_kind(p) && base_kind(n) => {
                    close_segment(&mut cur, &mut out);
                    loose_level = false;
                    sealed = false;
                    out.push(RubySegment { bases: vec![vec![node.clone()]], levels: Vec::new() });
                }
                _ => {}
            },
        }
    }
    flush_run(&mut run, &mut cur);
    close_segment(&mut cur, &mut out);
    out
}
