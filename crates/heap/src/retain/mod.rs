//! Граф удержания кучи без сборщика мусора: консервативный скан живых блоков
//! (как у Boehm GC и DMD) даёт рёбра «блок → блок», корни — стек потока и
//! статики модуля. По графу считаются кратчайший путь от корня до каждого
//! блока и дерево доминаторов с удержанным размером (как анализ кучи Gecko:
//! `ComputeShortestPaths` и `DominatorTree`, и панель Memory Chrome).
//!
//! Граф строится по запросу и ничего не стоит, пока его не попросили: список
//! живых блоков и чтение их памяти даёт вызывающий (`Memory`), здесь только
//! алгоритмы без зависимостей от платформы.
//!
//! Нумерация узлов: блоки `0..blocks` в порядке адресов, затем виртуальные
//! узлы — общий корень, по узлу на каждую область корней и узел «без корня»,
//! через который в граф входят блоки, недостижимые из областей (их держит
//! чужой стек, память ОС или они утекли в цикле `Rc`).

mod analyze;
mod scan;
mod unrooted;
mod write;

#[cfg(test)]
mod tests;

/// Живой блок кучи: адрес и запрошенный размер.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub address: usize,
    pub bytes: usize,
}

/// Область корней: диапазон памяти вне кучи, слова которого держат блоки
/// (стек потока, секция статик модуля, TLS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub name: String,
    pub start: usize,
    pub bytes: usize,
}

/// Чтение памяти блока или области словами указателя.
pub trait Memory {
    /// Выровненные слова диапазона `[address, address + bytes)`; хвост меньше
    /// слова не читается.
    fn words(&self, address: usize, bytes: usize) -> &[usize];
}

/// Нет ребра или родителя.
pub const NONE: u32 = u32::MAX;

/// Блок вошёл в граф через узел «без корня» как исток (никто его не держит).
pub const FLAG_UNHELD: u8 = 1;
/// Блок вошёл через узел «без корня» как представитель цикла, который никто
/// снаружи не держит (утечка цикла `Rc`).
pub const FLAG_CYCLE: u8 = 2;

/// Граф удержания и его анализ.
#[derive(Debug, Default)]
pub struct Graph {
    pub blocks: Vec<Block>,
    pub regions: Vec<Region>,
    /// Начало рёбер узла `i` — `edge_start[i]`, конец — `edge_start[i + 1]`.
    pub edge_start: Vec<u32>,
    pub edge_to: Vec<u32>,
    /// Смещение слова-указателя внутри источника ребра, байты.
    pub edge_offset: Vec<u32>,
    /// Непосредственный доминатор узла; у общего корня — он сам.
    pub idom: Vec<u32>,
    /// Байты, которые освободились бы вместе с узлом (поддерево доминаторов).
    pub retained: Vec<u64>,
    /// Предок на кратчайшем пути от корня и смещение его слова.
    pub parent: Vec<u32>,
    pub parent_offset: Vec<u32>,
    pub flags: Vec<u8>,
}

impl Graph {
    /// Индекс общего корня.
    pub fn root(&self) -> u32 {
        self.blocks.len() as u32
    }
    /// Индекс узла области корней `region`.
    pub fn region(&self, region: usize) -> u32 {
        (self.blocks.len() + 1 + region) as u32
    }
    /// Индекс узла «без корня».
    pub fn unrooted(&self) -> u32 {
        (self.blocks.len() + 1 + self.regions.len()) as u32
    }
    /// Число узлов вместе с виртуальными.
    pub fn nodes(&self) -> usize {
        self.blocks.len() + 2 + self.regions.len()
    }
    /// Рёбра узла: (цель, смещение).
    pub fn edges(&self, node: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        let range = self.edge_start[node as usize] as usize..self.edge_start[node as usize + 1] as usize;
        self.edge_to[range.clone()]
            .iter()
            .copied()
            .zip(self.edge_offset[range].iter().copied())
    }
    /// Собственный размер узла (у виртуальных — 0).
    pub fn bytes(&self, node: u32) -> u64 {
        self.blocks.get(node as usize).map_or(0, |block| block.bytes as u64)
    }
    /// Блок, внутрь которого указывает `address`.
    pub fn block_at(&self, address: usize) -> Option<u32> {
        scan::find(&self.blocks, address)
    }

    /// Скан и анализ: рёбра, вход недостижимого через «без корня», кратчайшие
    /// пути и доминаторы. Блоки сортируются по адресу; пересекающиеся и
    /// нулевые отбрасываются.
    pub fn build(blocks: Vec<Block>, regions: Vec<Region>, memory: &impl Memory) -> Self {
        let mut graph = Self::scan(blocks, regions, memory);
        graph.analyze();
        graph
    }

    /// Только рёбра (для замера цены скана отдельно от анализа).
    pub fn scan(blocks: Vec<Block>, regions: Vec<Region>, memory: &impl Memory) -> Self {
        scan::scan(blocks, regions, memory)
    }

    /// Вход недостижимого, кратчайшие пути, доминаторы и удержанный размер.
    pub fn analyze(&mut self) {
        unrooted::connect(self);
        analyze::shortest_paths(self);
        analyze::dominators(self);
    }

    /// Двоичный файл графа для `scripts/memprof_retain.py`.
    pub fn write(&self, out: &mut impl std::io::Write, header: &str) -> std::io::Result<()> {
        write::write(self, out, header)
    }
}
