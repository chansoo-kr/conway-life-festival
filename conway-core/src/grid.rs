use bevy::{platform::collections::HashMap, prelude::*, render::extract_resource::ExtractResource};

use crate::rle::Pattern;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub size: UVec2,
    pub planes: u32,
}

impl GridSize {
    pub fn new(size: UVec2, planes: u32) -> Self {
        assert!(
            size.x.is_multiple_of(32),
            "그리드 가로 크기는 32의 배수여야 합니다"
        );
        assert!((1..=2).contains(&planes));
        Self { size, planes }
    }

    #[inline]
    pub fn words_x(&self) -> u32 {
        self.size.x / 32
    }

    #[inline]
    pub fn plane_words(&self) -> u32 {
        self.words_x() * self.size.y
    }

    #[inline]
    pub fn total_words(&self) -> usize {
        (self.plane_words() * self.planes) as usize
    }

    #[inline]
    pub fn contains(&self, cell: IVec2) -> bool {
        cell.x >= 0 && cell.y >= 0 && (cell.x as u32) < self.size.x && (cell.y as u32) < self.size.y
    }

    #[inline]
    pub fn word_index(&self, plane: u32, x: u32, y: u32) -> u32 {
        plane * self.plane_words() + y * self.words_x() + x / 32
    }

    pub fn centered_origin(&self, pattern: &Pattern) -> IVec2 {
        IVec2::new(
            (self.size.x as i32 - pattern.width as i32) / 2,
            (self.size.y as i32 - pattern.height as i32) / 2,
        )
    }
}

pub fn pack_words(grid: &GridSize, placements: &[(&Pattern, IVec2, u32)]) -> Vec<u32> {
    let mut words = vec![0u32; grid.total_words()];
    for (pattern, origin, plane) in placements {
        for c in pattern.alive_cells() {
            let g = *origin + c.as_ivec2();
            if grid.contains(g) {
                let idx = grid.word_index(*plane, g.x as u32, g.y as u32) as usize;
                words[idx] |= 1 << (g.x as u32 % 32);
            }
        }
    }
    words
}

pub fn words_to_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

pub fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn plane_slice<'a>(words: &'a [u32], grid: &GridSize, plane: u32) -> &'a [u32] {
    let n = grid.plane_words() as usize;
    let start = plane as usize * n;
    words.get(start..start + n).unwrap_or(&[])
}

pub fn count_plane(words: &[u32], grid: &GridSize, plane: u32) -> u32 {
    plane_slice(words, grid, plane)
        .iter()
        .map(|w| w.count_ones())
        .sum()
}

pub fn alive_cells_in(words: &[u32], grid: &GridSize, plane: u32) -> Vec<IVec2> {
    let wx = grid.words_x();
    let mut out = Vec::new();
    for (i, mut w) in plane_slice(words, grid, plane).iter().copied().enumerate() {
        if w == 0 {
            continue;
        }
        let base_x = (i as u32 % wx) * 32;
        let y = (i as u32 / wx) as i32;
        while w != 0 {
            out.push(IVec2::new((base_x + w.trailing_zeros()) as i32, y));
            w &= w - 1;
        }
    }
    out
}

#[derive(Resource, Clone, Debug)]
pub struct CpuGrid {
    pub grid: GridSize,
    pub words: Vec<u32>,
}

impl CpuGrid {
    pub fn new(grid: GridSize) -> Self {
        Self {
            grid,
            words: vec![0; grid.total_words()],
        }
    }

    pub fn clear(&mut self) {
        self.words.fill(0);
    }

    #[inline]
    pub fn get(&self, plane: u32, cell: IVec2) -> bool {
        if !self.grid.contains(cell) {
            return false;
        }
        let idx = self.grid.word_index(plane, cell.x as u32, cell.y as u32) as usize;
        (self.words[idx] >> (cell.x as u32 % 32)) & 1 == 1
    }

    #[inline]
    pub fn set(&mut self, plane: u32, cell: IVec2, alive: bool) {
        if !self.grid.contains(cell) {
            return;
        }
        let idx = self.grid.word_index(plane, cell.x as u32, cell.y as u32) as usize;
        let bit = 1u32 << (cell.x as u32 % 32);
        if alive {
            self.words[idx] |= bit;
        } else {
            self.words[idx] &= !bit;
        }
    }

    pub fn count(&self, plane: u32) -> u32 {
        count_plane(&self.words, &self.grid, plane)
    }

    pub fn alive_cells(&self, plane: u32) -> Vec<IVec2> {
        alive_cells_in(&self.words, &self.grid, plane)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WordEdit {
    pub index: u32,
    pub set_mask: u32,
    pub clear_mask: u32,
}

#[derive(Resource, Clone, Default, ExtractResource)]
pub struct PendingEdits {
    edits: HashMap<u32, WordEdit>,
}

impl PendingEdits {
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn len(&self) -> usize {
        self.edits.len()
    }

    pub fn clear(&mut self) {
        self.edits.clear();
    }

    fn word(&mut self, index: u32) -> &mut WordEdit {
        self.edits
            .entry(index)
            .or_insert(WordEdit { index, ..default() })
    }

    pub fn set_cell(&mut self, grid: &GridSize, cell: IVec2, owner: Option<u32>) {
        if !grid.contains(cell) {
            return;
        }
        let (x, y) = (cell.x as u32, cell.y as u32);
        let bit = 1u32 << (x % 32);
        for plane in 0..grid.planes {
            let e = self.word(grid.word_index(plane, x, y));
            if owner == Some(plane) {
                e.set_mask |= bit;
                e.clear_mask &= !bit;
            } else {
                e.clear_mask |= bit;
                e.set_mask &= !bit;
            }
        }
    }

    pub fn stamp(&mut self, grid: &GridSize, pattern: &Pattern, origin: IVec2, plane: u32) {
        for c in pattern.alive_cells() {
            self.set_cell(grid, origin + c.as_ivec2(), Some(plane));
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.edits.len() * 12);
        for e in self.edits.values() {
            bytes.extend_from_slice(&e.index.to_le_bytes());
            bytes.extend_from_slice(&e.set_mask.to_le_bytes());
            bytes.extend_from_slice(&e.clear_mask.to_le_bytes());
        }
        bytes
    }
}
