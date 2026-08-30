use bevy::{
    math::IVec2,
    platform::collections::{HashMap, HashSet},
};

use crate::rle::Pattern;

const MAX_LEVEL: u32 = 30;
const MAX_AREA: u64 = 200_000_000;
const CORE_TILE: i32 = 64;
const CORE_MIN_TILE_CELLS: usize = 16;

enum Node {
    Leaf8([u8; 8]),
    Leaf2([bool; 4]),
    Inner { level: u32, children: [usize; 4] },
}

pub fn parse_macrocell(text: &str) -> Result<Pattern, String> {
    let mut nodes: Vec<Node> = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with('[') {
            continue;
        }
        if l.starts_with(['.', '*', '$']) {
            let mut rows = [0u8; 8];
            let (mut x, mut y) = (0usize, 0usize);
            for c in l.chars() {
                match c {
                    '.' => x += 1,
                    '*' => {
                        if x < 8 && y < 8 {
                            rows[y] |= 1 << x;
                        }
                        x += 1;
                    }
                    '$' => {
                        y += 1;
                        x = 0;
                    }
                    _ => {}
                }
            }
            nodes.push(Node::Leaf8(rows));
            continue;
        }
        let nums: Vec<usize> = l
            .split_whitespace()
            .map(|t| t.parse::<usize>())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("line {}: expected numbers: {l}", line_no + 1))?;
        if nums.len() != 5 {
            return Err(format!("line {}: malformed node: {l}", line_no + 1));
        }
        let level = nums[0] as u32;
        if level == 1 {
            nodes.push(Node::Leaf2([
                nums[1] != 0,
                nums[2] != 0,
                nums[3] != 0,
                nums[4] != 0,
            ]));
            continue;
        }
        if level > MAX_LEVEL {
            return Err(format!("line {}: level too large ({level})", line_no + 1));
        }
        let children = [nums[1], nums[2], nums[3], nums[4]];
        if children.iter().any(|c| *c > nodes.len()) {
            return Err(format!("line {}: forward node reference: {l}", line_no + 1));
        }
        nodes.push(Node::Inner { level, children });
    }
    if nodes.is_empty() {
        return Err("no nodes".into());
    }
    let mut cells = Vec::new();
    collect(&nodes, nodes.len(), IVec2::ZERO, &mut cells);
    if cells.is_empty() {
        return Err("no live cells".into());
    }
    if bounding_area(&cells) > MAX_AREA {
        cells = crop_to_core(cells);
    }
    let area = bounding_area(&cells);
    if area > MAX_AREA {
        return Err(format!("pattern too large ({area} cell area)"));
    }
    Pattern::from_alive_cells(cells).ok_or_else(|| "no live cells".into())
}

fn bounding_area(cells: &[IVec2]) -> u64 {
    let (min, max) = cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| {
        (lo.min(*c), hi.max(*c))
    });
    let size = (max - min + IVec2::ONE).as_u64vec2();
    size.x * size.y
}

fn crop_to_core(cells: Vec<IVec2>) -> Vec<IVec2> {
    let mut counts: HashMap<IVec2, usize> = HashMap::default();
    for c in &cells {
        *counts
            .entry(c.div_euclid(IVec2::splat(CORE_TILE)))
            .or_default() += 1;
    }
    let dense: HashSet<IVec2> = counts
        .iter()
        .filter(|(_, n)| **n >= CORE_MIN_TILE_CELLS)
        .map(|(t, _)| *t)
        .collect();
    let mut visited: HashSet<IVec2> = HashSet::default();
    let mut best: Option<(usize, IVec2, IVec2)> = None;
    for start in &dense {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![*start];
        visited.insert(*start);
        let (mut total, mut lo, mut hi) = (0usize, *start, *start);
        while let Some(t) = stack.pop() {
            total += counts[&t];
            lo = lo.min(t);
            hi = hi.max(t);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let n = t + IVec2::new(dx, dy);
                    if dense.contains(&n) && visited.insert(n) {
                        stack.push(n);
                    }
                }
            }
        }
        if best.is_none_or(|(b, _, _)| total > b) {
            best = Some((total, lo, hi));
        }
    }
    let Some((_, lo, hi)) = best else {
        return cells;
    };
    let min = (lo - IVec2::ONE) * CORE_TILE;
    let max = (hi + IVec2::splat(2)) * CORE_TILE;
    cells
        .into_iter()
        .filter(|c| c.x >= min.x && c.y >= min.y && c.x < max.x && c.y < max.y)
        .collect()
}

fn collect(nodes: &[Node], idx: usize, origin: IVec2, out: &mut Vec<IVec2>) {
    if idx == 0 {
        return;
    }
    match &nodes[idx - 1] {
        Node::Leaf8(rows) => {
            for (y, row) in rows.iter().enumerate() {
                let mut r = *row;
                while r != 0 {
                    let x = r.trailing_zeros();
                    out.push(origin + IVec2::new(x as i32, y as i32));
                    r &= r - 1;
                }
            }
        }
        Node::Leaf2(states) => {
            for (k, alive) in states.iter().enumerate() {
                if *alive {
                    out.push(origin + IVec2::new((k % 2) as i32, (k / 2) as i32));
                }
            }
        }
        Node::Inner { level, children } => {
            let half = 1i32 << (level - 1);
            for (k, child) in children.iter().enumerate() {
                let offset = IVec2::new((k % 2) as i32 * half, (k / 2) as i32 * half);
                collect(nodes, *child, origin + offset, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rle::parse_rle;

    #[test]
    fn two_state_leaf() {
        let p = parse_macrocell("[M2] (golly 2.0)\n#R B3/S23\n.*$..*$***$\n").unwrap();
        assert_eq!(p, parse_rle("x = 3, y = 3\nbob$2bo$3o!").unwrap());
    }

    #[test]
    fn multistate_tree() {
        let p = parse_macrocell("[M2]\n#R B3/S23\n1 0 1 0 0\n1 0 0 1 1\n2 1 2 0 0\n").unwrap();
        assert_eq!(p, parse_rle("x = 3, y = 2\no$b2o!").unwrap());
    }

    #[test]
    fn rejects_forward_reference() {
        assert!(parse_macrocell("2 0 0 0 5\n").is_err());
    }
}
