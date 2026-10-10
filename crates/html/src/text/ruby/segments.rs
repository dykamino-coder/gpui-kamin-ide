//! Segments for ruby; split out to keep the owning module within 250 lines.

use super::ruby_role;
use super::segment_state::{collect_bases, kind_of};
use super::{Kind, RubyLevel, RubySegment, RubyUnit, base_starts, close_segment, flush_run, fresh};
use crate::dom::Node;
use crate::text::text_box::blank_text;

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
pub(super) fn ruby_segments(children: &[Node]) -> Vec<RubySegment> {
    // Вид — по РОЛИ (тег или `display: ruby*`, `ruby_role`): `span.rt
    // { display: ruby-text }` — аннотация (`rt-display-001`), `span#rbc
    // { display: ruby-base-container }` — контейнер баз (`rbc-rtc-basic-001`).
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
                collect_bases(k, &mut cur);
            }
            Kind::Rt => {
                flush_run(&mut run, &mut cur);
                if !loose_level {
                    cur.levels.push(RubyLevel {
                        units: Vec::new(),
                        spanning: false,
                        container: None,
                    });
                    loose_level = true;
                }
                cur.levels
                    .last_mut()
                    .expect("уровень только что открыт")
                    .units
                    .push(vec![node.clone()]);
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
                cur.levels.push(RubyLevel {
                    units,
                    spanning,
                    container: Some(k.style.clone()),
                });
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
                    out.push(RubySegment {
                        bases: vec![vec![node.clone()]],
                        levels: Vec::new(),
                    });
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
                    cur.levels
                        .last_mut()
                        .expect("уровень открыт")
                        .units
                        .push(vec![node.clone()]);
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
                    out.push(RubySegment {
                        bases: vec![vec![node.clone()]],
                        levels: Vec::new(),
                    });
                }
                _ => {}
            },
        }
    }
    flush_run(&mut run, &mut cur);
    close_segment(&mut cur, &mut out);
    out
}
