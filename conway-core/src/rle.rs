use bevy::math::{IVec2, UVec2};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<bool>,
}

impl Pattern {
    pub fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells: vec![false; (width * height) as usize],
        }
    }

    pub fn size(&self) -> UVec2 {
        UVec2::new(self.width, self.height)
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.cells[(y * self.width + x) as usize]
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, alive: bool) {
        if x < self.width && y < self.height {
            self.cells[(y * self.width + x) as usize] = alive;
        }
    }

    pub fn alive_count(&self) -> u32 {
        self.cells.iter().filter(|c| **c).count() as u32
    }

    pub fn alive_cells(&self) -> impl Iterator<Item = UVec2> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, alive)| **alive)
            .map(|(i, _)| UVec2::new(i as u32 % self.width, i as u32 / self.width))
    }

    pub fn flip_h(&self) -> Pattern {
        let mut out = Pattern::empty(self.width, self.height);
        for c in self.alive_cells() {
            out.set(self.width - 1 - c.x, c.y, true);
        }
        out
    }

    pub fn flip_v(&self) -> Pattern {
        let mut out = Pattern::empty(self.width, self.height);
        for c in self.alive_cells() {
            out.set(c.x, self.height - 1 - c.y, true);
        }
        out
    }

    pub fn rotate_cw(&self) -> Pattern {
        let mut out = Pattern::empty(self.height, self.width);
        for c in self.alive_cells() {
            out.set(self.height - 1 - c.y, c.x, true);
        }
        out
    }

    pub fn from_alive_cells(cells: impl IntoIterator<Item = IVec2>) -> Option<Pattern> {
        let cells: Vec<IVec2> = cells.into_iter().collect();
        let min_x = cells.iter().map(|c| c.x).min()?;
        let min_y = cells.iter().map(|c| c.y).min()?;
        let max_x = cells.iter().map(|c| c.x).max()?;
        let max_y = cells.iter().map(|c| c.y).max()?;
        let mut out = Pattern::empty((max_x - min_x + 1) as u32, (max_y - min_y + 1) as u32);
        for c in cells {
            out.set((c.x - min_x) as u32, (c.y - min_y) as u32, true);
        }
        Some(out)
    }

    pub fn normalized(&self) -> Option<Pattern> {
        Pattern::from_alive_cells(self.alive_cells().map(|c| c.as_ivec2()))
    }
}

pub fn parse_rle(text: &str) -> Result<Pattern, String> {
    let mut width = 0u32;
    let mut height = 0u32;
    let mut header_done = false;
    let mut body = String::new();

    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if !header_done {
            for part in l.split(',') {
                let mut kv = part.splitn(2, '=');
                let key = kv.next().unwrap_or("").trim().to_ascii_lowercase();
                let value = kv.next().unwrap_or("").trim();
                match key.as_str() {
                    "x" => {
                        width = value
                            .parse()
                            .map_err(|_| format!("x 값 파싱 실패: {value}"))?
                    }
                    "y" => {
                        height = value
                            .parse()
                            .map_err(|_| format!("y 값 파싱 실패: {value}"))?
                    }
                    _ => {}
                }
            }
            header_done = true;
        } else {
            body.push_str(l);
        }
    }

    if width == 0 || height == 0 {
        return Err("헤더(x = .., y = ..)를 찾을 수 없습니다".into());
    }

    let mut pattern = Pattern::empty(width, height);
    let (mut x, mut y) = (0u32, 0u32);
    let mut run = 0u32;

    'outer: for c in body.chars() {
        match c {
            '0'..='9' => run = run * 10 + c.to_digit(10).unwrap(),
            '!' => break 'outer,
            '$' => {
                y += run.max(1);
                x = 0;
                run = 0;
            }
            'b' | 'B' | '.' => {
                x += run.max(1);
                run = 0;
            }
            c if c.is_ascii_alphabetic() => {
                let n = run.max(1);
                for i in 0..n {
                    pattern.set(x + i, y, true);
                }
                x += n;
                run = 0;
            }
            _ => {}
        }
    }

    Ok(pattern)
}

pub fn rle_name(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix("#N")
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glider_parses() {
        let p = parse_rle("x = 3, y = 3\nbob$2bo$3o!").unwrap();
        assert_eq!(p.alive_count(), 5);
        assert!(p.get(1, 0) && p.get(2, 1) && p.get(0, 2) && p.get(1, 2) && p.get(2, 2));
    }

    #[test]
    fn normalize_trims() {
        let p = parse_rle("x = 5, y = 5\n$2b2o$2b2o!").unwrap();
        let n = p.normalized().unwrap();
        assert_eq!((n.width, n.height), (2, 2));
        assert_eq!(n.alive_count(), 4);
    }

    fn cpu_step(cells: &std::collections::HashSet<IVec2>) -> std::collections::HashSet<IVec2> {
        let mut counts: std::collections::HashMap<IVec2, u32> = Default::default();
        for c in cells {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx != 0 || dy != 0 {
                        *counts.entry(*c + IVec2::new(dx, dy)).or_default() += 1;
                    }
                }
            }
        }
        counts
            .into_iter()
            .filter(|(c, n)| *n == 3 || (*n == 2 && cells.contains(c)))
            .map(|(c, _)| c)
            .collect()
    }

    fn evolve(p: &Pattern, n: usize) -> (Pattern, IVec2) {
        let mut cells: std::collections::HashSet<IVec2> =
            p.alive_cells().map(|c| c.as_ivec2()).collect();
        for _ in 0..n {
            cells = cpu_step(&cells);
        }
        let min = IVec2::new(
            cells.iter().map(|c| c.x).min().unwrap(),
            cells.iter().map(|c| c.y).min().unwrap(),
        );
        (Pattern::from_alive_cells(cells).unwrap(), min)
    }

    #[test]
    fn glider_moves_south_east() {
        let g = parse_rle("x = 3, y = 3\nbob$2bo$3o!").unwrap();
        let (after, shift) = evolve(&g, 4);
        assert_eq!(after, g);
        assert_eq!(shift, IVec2::new(1, 1));
    }

    #[test]
    fn lwss_moves_west() {
        let s = parse_rle("x = 5, y = 4\nbo2bo$o4b$o3bo$4o!").unwrap();
        let (after, shift) = evolve(&s, 4);
        assert_eq!(after, s);
        assert_eq!(shift, IVec2::new(-2, 0));
    }

    #[test]
    fn rotate_flip_roundtrip() {
        let p = parse_rle("x = 3, y = 3\nbob$2bo$3o!").unwrap();
        let r = p.rotate_cw().rotate_cw().rotate_cw().rotate_cw();
        assert_eq!(p, r);
        assert_eq!(p, p.flip_h().flip_h());
        assert_eq!(p, p.flip_v().flip_v());
    }
}
