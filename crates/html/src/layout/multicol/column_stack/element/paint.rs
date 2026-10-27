//! Краска фрагментов колонок: маски колонок, копии детей, сдвиги.

use crate::layout::fragment::types::Frag;
use crate::layout::fragment::{fragment_mask, gap_fragment};
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl ColumnStack {
    // Fragment painting shares the column geometry and the active GPUI context.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_frags(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        bounds: Bounds<Pixels>,
        col_w: f32,
        step: f32,
        parts: Vec<usize>,
        plan_all: Vec<Frag>,
        plan: Vec<Frag>,
    ) {
        let nested_frag = crate::layout::fragment::effect_fragment::active(&self.paint);
        for f in plan {
            let (c, ry) = self.place(f.col);
            // Срез едет вместе со сдвинутым фрагментом (css-break-3 §5.5):
            // маска, оставленная на месте колонки, съедала его целиком
            // (`out-of-flow-in-multicolumn-042/045`, проба `pm3`).
            let rel = self.children[f.kid].rel;
            let x = bounds.origin.x + px(c as f32 * step + rel.0);
            let x = x + px(self.children[f.kid].par.dx);
            let y = bounds.origin.y + px(ry + f.y + rel.1);
            // Маска — устройство `slice`. Фрагмент `clone` самодостаточен:
            // содержимое режет его внутренняя обёртка (`clone_fragment`), а
            // тень/контур обязаны выходить за колонку (`clone-009`).
            // Единственный кусок, оборванный в усечённом хвостовом поле
            // (`fill_pieces`: от ребёнка за разрезом осталось одно поле), —
            // тоже срез: коробка кончается краем колонки.
            let tail_cut = {
                let k = &self.children[f.kid];
                f.from + f.h < k.h - COORD_EPS
                    && k.cuts.iter().any(|&(_, nf)| nf >= k.h - COORD_EPS)
            };
            let split = (parts[f.kid] > 1 || tail_cut) && self.children[f.kid].clone_dec.is_none();
            // Срез `slice` (css-break-3 §4) — поперёк БЛОЧНОЙ оси. Вбок колонка
            // переполнение не режет: css-multicol-1 §8.1 «content that extends
            // outside column boxes visibly overflows and is not clipped to the column
            // box» (Blink режет только `overflow` самой коробки). Маска шириной в
            // колонку прятала жёлтую полосу 180px поверх линеек (`column-rule-002`),
            // правую четверть ребёнка 100px в колонке 75
            // (`relative-child-overflowing-column-gap`) и всё содержимое при стопке
            // шириной 0 (`relative-child-overflowing-container`, колонка 1px). Вылет —
            // на ширину окна (у стопки нулевой ширины своей ширины нет); дальше режет
            // маска предка.
            // Кроме ребёнка с вложенным многоколоночником (`StackChild::nested_cols`):
            // его ширина за колонкой — артефакт плоской копии, а не переполнение
            // (красный потомок `margin-left:100%`
            // в `multicol-fill-balance-nested-000` прячет только маска колонки).
            let win_w = window.viewport_size().width;
            let spill = if self.children[f.kid].nested_cols {
                px(0.)
            } else if win_w > bounds.size.width {
                win_w
            } else {
                bounds.size.width
            };
            let k0 = &self.children[f.kid];
            let (nested_cols, over) = (k0.nested_cols, k0.over);
            // Таблица и вложенная фрагментация задают свои границы. Одна
            // лишь мера размера не обрезает тень на краю без разрыва
            // (css-break-3 §5.4; Blink BoxFragmentPainter::SidesToInclude).
            let table_like = k0.repeat.is_some() || k0.slack.is_some() || nested_frag;
            let kid = &mut self.children[f.kid];
            // Полосы повтора таблицы — каждая своей маской по своей полосе.
            if let Some(r) = kid.repeat.as_mut() {
                let mask_scale = window.scale_factor();
                let band_mask = |top: f32, h: f32| {
                    fragment_mask::snap(
                        Bounds {
                            origin: point(x - spill, y + px(top)),
                            size: size(px(col_w) + spill + spill, px(h)),
                        },
                        mask_scale,
                    )
                };
                r.paint_bands(&f, |el, top, h| {
                    window
                        .with_content_mask(Some(band_mask(top, h)), |window| el.paint(window, cx));
                });
            }
            // Хвост непоследнего фрагмента таблицы — фоном таблицы (`slack`).
            if let Some(bg) = kid.slack
                && plan_all
                    .iter()
                    .any(|g| g.kid == f.kid && g.copy == f.copy + 1)
            {
                let line = if matches!(self.rows, Some(r) if r.wrap) {
                    f.col / self.count
                } else {
                    0
                };
                let line_h = self.lines_plan.borrow().get(line).map_or(0.0, |l| l.1);
                let band = kid
                    .repeat
                    .as_ref()
                    .and_then(|r| r.foot)
                    .map_or(0.0, |b| b.1);
                // Хвост — остаток фрагментаинера без полосы подвала: под
                // подвалом, если тот стоит сразу под последним рядом
                // (`Repeat::foot_top`), иначе над ним.
                let foot_top = kid.repeat.as_ref().and_then(|r| r.foot_top(&f));
                let (tail_top, tail) = match foot_top {
                    Some(top) if top <= f.h + COORD_EPS => (f.h + band, f.foot - band),
                    Some(_) => (f.h, f.foot - band),
                    None => (f.h, line_h - f.y - f.h),
                };
                // Не шире колонки: разложенная ширина копии несёт дробный
                // остаток раскладки, и хвост залезал на край соседней колонки
                // (`multi-line-row-flex-fragmentation-090`: пиксель красного у
                // левого края третьей колонки).
                let w = kid.laid_w.get().min(col_w);
                if tail > COORD_EPS && w > COORD_EPS {
                    // По пикселям устройства, как маска фрагмента: дробная
                    // ширина копии (`33.333px`) иначе оставляла полупрозрачный
                    // край рядом с соседом (`multi-line-row-flex-
                    // fragmentation-090`).
                    let sf = window.scale_factor();
                    window.paint_quad(gpui::fill(
                        fragment_mask::snap(
                            Bounds {
                                origin: point(x, y + px(tail_top)),
                                size: size(px(w), px(tail)),
                            },
                            sf,
                        )
                        .bounds,
                        bg,
                    ));
                }
            }
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            // Маска и режет: копия нарисована во всю свою высоту, видна
            // только полоса своей колонки (css-break-3 §4, вид `slice`).
            if split {
                let line = if matches!(self.rows, Some(r) if r.wrap) {
                    f.col / self.count
                } else {
                    0
                };
                let line_h = self.lines_plan.borrow().get(line).map_or(0.0, |l| l.1);
                let continued = plan_all
                    .iter()
                    .any(|g| g.kid == f.kid && g.copy == f.copy + 1);
                let parent = window.content_mask();
                if let Some(w) = &kid.watched {
                    w.set_paint_outer(Some(parent));
                }
                // Разрез режет только ВНУТРЕННИЕ края фрагментов: чернильное
                // переполнение над первым фрагментом и под последним (тень,
                // контур) видно (css-break-3 §5.4 «slice»: края без разрыва
                // рисуются как у целой коробки; Blink красит переполнение
                // фрагмента без обрезки фрагментаинером). Под последним — до
                // низа колонки: дальше переполнение — поток следующей.
                let (up, down) = if nested_cols || table_like {
                    (0.0, 0.0)
                } else {
                    (
                        if f.copy == 0 && f.from <= COORD_EPS {
                            f32::from(spill)
                        } else {
                            0.0
                        },
                        if continued || f.from + f.h < over - COORD_EPS {
                            0.0
                        } else {
                            (line_h - f.y - f.h).max(0.0)
                        },
                    )
                };
                let clip = Bounds {
                    origin: point(x - spill, y - px(up)),
                    size: size(px(col_w) + spill + spill, px(f.h + up + down)),
                };
                let mask = fragment_mask::snap(clip, window.scale_factor());
                window.with_content_mask(Some(mask), |window| {
                    let cut = f32::from(y) + f.h;
                    let end = if continued && f.foot <= COORD_EPS {
                        cut.max(f32::from(y) + line_h - f.y)
                    } else {
                        cut
                    };
                    let scope = Some(gap_fragment::Scope {
                        parent,
                        mask: window.content_mask(),
                        start: f32::from(y),
                        cut,
                        end,
                    });
                    crate::layout::fragment::effect_fragment::with(
                        &self.paint,
                        Some(crate::layout::fragment::effect_fragment::Scope {
                            parent,
                            fragment: Bounds {
                                origin: point(x, y),
                                size: size(px(col_w), px(f.h)),
                            },
                            clip,
                        }),
                        || gap_fragment::with(&self.paint, scope, || el.paint(window, cx)),
                    );
                });
            } else {
                el.paint(window, cx);
            }
        }
    }
}
