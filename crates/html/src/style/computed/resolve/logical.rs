//! Логические свойства на физические поля: logical() и resolve_logical() с учётом направления письма родителя и ячейки таблицы.

use super::*;

impl Computed {
    /// Место под логические значения — заводится по первому обращению: у
    /// подавляющего большинства узлов их нет вовсе.
    pub(crate) fn logical(&mut self) -> &mut Logical {
        self.logical.get_or_insert_with(Default::default)
    }

    pub fn resolve_logical(&mut self, parent_vertical: Option<bool>, is_cell: bool) {
        let Some(logical) = self.logical.take() else {
            return;
        };
        // ОСИ НЕ ПЕРЕСТАВЛЯЮТСЯ, и это не упрощение, а следствие устройства
        // вертикального письма: оно рисуется ПОВОРОТОМ блока, то есть внутри
        // повёрнутого поддерева физические ширина и высота уже поменялись
        // местами. Перевод логических сторон «как в спецификации» переставил
        // бы их второй раз — замерено: writing-modes 191 → 189. Переставлять
        // здесь можно будет только вместе с отказом от поворота.
        // ПОВТОРНО ЗАМЕРЕНО И ОТКАЧЕНО (2026-08-09), уже на новом
        // вертикальном письме: перестановка ЦЕЛИКОМ — writing-modes 200 → 201,
        // flexbox 352 → 349; перестановка ТОЛЬКО размеров — те же числа.
        // Теряются `css-flexbox/gap-001-lr`, `gap-007-lr`, `gap-007-rl`
        // (получает `gap-002-rl`): размер там задан логическим свойством, и
        // после перестановки он ложится поперёк уже повёрнутой раскладки.
        // ОТКАТ ПРОТУХ (проверено 30.08 по свежему своду): из трёх названных
        // выше потерь `gap-007-lr` (0.99) и `gap-007-rl` (0.56) красные и без
        // перестановки, а «приобретение» `gap-002-rl` зелено само.
        // ЗАМЕРЕНО И ОТКАЧЕНО: исключать отсюда ячейку таблицы, чтобы вернуть
        // `table-cell-align-005` и `table-cell-valign-003`. Полный свод CSS3:
        // те же 4 потери — на этом шаге у ячейки ещё нет `display` (табличность
        // движок держит по ТЕГУ, а `Display::TableCell` приходит только из
        // авторского CSS).
        //
        // Переставляем только у УНАСЛЕДОВАВШЕГО письмо: у ортогонального узла
        // (письмо объявлено на нём самом, родитель горизонтален) перестановку
        // ниже по течению делает табличный и блочный код, и вторая здесь
        // складывалась с ней в поворот на месте.
        // РАЗМЕРЫ отображаются по СВОЕМУ письму, а не по письму родителя.
        // Таблица Abstract-Physical Mapping (css-writing-modes-4,
        // Overview.bs:1790-1827) даёт `block-size` -> width и
        // `inline-size` -> height ВСЕМ вертикальным письмам, а колонку
        // выбирает used-значение `writing-mode` САМОГО элемента: письма
        // родителя в таблице нет. Blink делает ровно это одним предикатом —
        // `computed_style.h:1270` `LogicalWidth() = IsHorizontalWritingMode()
        // ? Width() : Height()` (и так же Logical{Min,Max}{Width,Height},
        // строки 1275-1287).
        // Нашей поворотной модели это не мешает: коробки физические на всех
        // уровнях, вертикальный контейнер кладёт детей `flex_row`
        // (`render.rs:14461`), поэтому у ортогонального узла (письмо
        // объявлено на нём, родитель горизонтален) физическая ширина — его
        // БЛОЧНАЯ ось, а высота — СТРОЧНАЯ, ровно как у унаследовавшего
        // письмо. Стороны так считаются давно — `side_vertical` ниже.
        // Ячейка таблицы — единственное исключение: у неё логический
        // inline-size перекладывает в высоту сам табличный код
        // (`render.rs:16790-16800` по флагу `width_from_inline`), и второй
        // перевод здесь сложился бы с ним в поворот на месте
        // (`table-cell-align-005`, `table-cell-valign-003` — замеренный
        // откат в шапке функции).
        let vertical = self.vertical == Some(true) && (parent_vertical == Some(true) || !is_cell);
        let rtl = self.rtl == Some(true);
        // Стороны (поля/отступы/края) переставляются ПО-НАСТОЯЩЕМУ: блочный
        // поток вертикального письма собирается транспонированным рядом
        // (`flex_row(_reverse)`), а не поворотом — `margin-block` абзаца в
        // vertical-rl обязан лечь горизонтально (wm-propagation-body-*).
        // Размеры остаются без перестановки (замерено дважды, см. выше).
        let side_vertical = self.vertical == Some(true);
        let side_rl = self.vertical_rl == Some(true);
        let side_sw_lr = self.sideways == Some(true) && !side_rl;
        // Логическое значение ПЕРЕКРЫВАЕТ физическое, а не только заполняет
        // пустое. Так вело себя прежнее разложение при разборе (оно писало
        // прямо в физическое поле), и порядок объявлений в правиле от этого
        // сохранялся. Заполнение только пустого меняло исход там, где заданы
        // оба (замерено на `grid-self-alignment-baseline-with-grid-002`).
        let set = |slot: &mut Option<Len>, val: Option<Len>| {
            if val.is_some() {
                *slot = val;
            }
        };
        // Размеры: строчная ось горизонтальна при обычном письме и
        // вертикальна при вертикальном.
        let set_ci = |slot: &mut Option<f32>, val: Option<f32>| {
            if val.is_some() {
                *slot = val;
            }
        };
        // `contain-intrinsic-*-size` переставляется по СВОЕМУ письму, а не по
        // общему гейту `vertical` (тот требует ещё и вертикального родителя).
        // Причина: читается пара через `contains_width()`/`contains_height()`,
        // а те смотрят ТОЛЬКО на `self.vertical`. У ортогонального узла
        // (письмо объявлено на нём, родитель горизонтален) условия расходились,
        // и `contain-intrinsic-inline-size` приезжал поперёк — так падали
        // `contain-intrinsic-size-logical-002` и
        // `grid-lanes-contain-intrinsic-size-logical-001`. Размеры и стороны
        // ниже остаются на прежнем гейте: их перестановку у ортогонального
        // узла делает код ниже по течению (замер описан выше по функции).
        if side_vertical {
            set_ci(&mut self.contain_intrinsic.1, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.0, logical.ci_block);
        } else {
            set_ci(&mut self.contain_intrinsic.0, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.1, logical.ci_block);
        }
        if vertical {
            set(&mut self.height, logical.inline_size);
            set(&mut self.width, logical.block_size);
            set(&mut self.min_height, logical.min_inline);
            set(&mut self.min_width, logical.min_block);
            set(&mut self.max_height, logical.max_inline);
            set(&mut self.max_width, logical.max_block);
        } else {
            if self.vertical == Some(true) && logical.inline_size.is_some() {
                self.width_from_inline = true;
            }
            set(&mut self.width, logical.inline_size);
            set(&mut self.height, logical.block_size);
            set(&mut self.min_width, logical.min_inline);
            set(&mut self.min_height, logical.min_block);
            set(&mut self.max_width, logical.max_inline);
            set(&mut self.max_height, logical.max_block);
        }
        // Стороны. Начало строчной оси: слева (обычное письмо), справа (оно же
        // справа налево) или сверху (вертикальное). Начало оси блока: сверху,
        // а в вертикальном — справа при `vertical-rl` и слева при `-lr`.
        // Логическая сторона ложится на физическую, только если объявлена
        // ПОЗЖЕ её (порядок каскада): UA `padding-inline-start: 40px` у `ul`
        // против авторского `padding-top: 0` в `vertical-rl`
        // (`line-box-direction-vrl-019`, `block-flow-direction-vrl-021`).
        let phys_seq = self.side_seq;
        let sides = [
            (&logical.padding, 0u8, phys_seq.padding),
            (&logical.margin, 1, phys_seq.margin),
            (&logical.inset, 2, phys_seq.inset),
        ];
        for (from, which, pseq) in sides {
            let to = match which {
                0 => &mut self.padding,
                1 => &mut self.margin,
                _ => &mut self.inset,
            };
            let (i_start, i_end, b_start, b_end) = if side_vertical {
                // Начало оси блока: `vertical-rl` — ПРАВЫЙ край (поток блоков
                // идёт справа налево), `vertical-lr` — левый.
                let (bs, be) = if side_rl { (1u8, 3u8) } else { (3, 1) };
                // `sideways-lr`: строка идёт СНИЗУ ВВЕРХ (css-writing-modes-4
                // §3) — начало строчной оси у нижнего края, а не верхнего.
                let flip = side_sw_lr != rtl;
                let (is, ie) = if flip { (2u8, 0u8) } else { (0, 2) };
                (is, ie, bs, be)
            } else if rtl {
                (1u8, 3u8, 0u8, 2u8)
            } else {
                (3u8, 1u8, 0u8, 2u8)
            };
            for (side, val, lseq) in [
                (i_start, from.inline_start, from.seq[0]),
                (i_end, from.inline_end, from.seq[1]),
                (b_start, from.block_start, from.seq[2]),
                (b_end, from.block_end, from.seq[3]),
            ] {
                let slot = match side {
                    0 => &mut to.top,
                    1 => &mut to.right,
                    2 => &mut to.bottom,
                    _ => &mut to.left,
                };
                if lseq >= pseq[side as usize] {
                    set(slot, val);
                }
            }
        }
        // Логические кромки: раскладываются той же картой сторон — сырое
        // значение прогоняется через обычный разбор кромки на настоящую
        // физическую сторону.
        let (i_start, i_end, b_start, b_end) = if side_vertical {
            let (bs, be) = if side_rl { (1u8, 3u8) } else { (3, 1) };
            let flip = side_sw_lr != rtl;
            let (is, ie) = if flip { (2u8, 0u8) } else { (0, 2) };
            (is, ie, bs, be)
        } else if rtl {
            (1u8, 3u8, 0u8, 2u8)
        } else {
            (3u8, 1u8, 0u8, 2u8)
        };
        let borders = logical.border.clone();
        for (raw, side) in [
            (&borders[0], b_start),
            (&borders[1], i_end),
            (&borders[2], b_end),
            (&borders[3], i_start),
        ] {
            if let Some(v) = raw {
                self.apply_border_shorthand(v, Some(side as usize));
            }
        }
    }
}
