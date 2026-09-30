//! Squarified treemap layout (Bruls, Huizing, van Wijk).
//!
//! A pure function from values and a rectangle to rectangles. DESIGN-GUI §8
//! asks for exactly that — "keep layout algorithms as free functions taking
//! data and returning geometry" — because then they are testable without a
//! GPU, benchmarkable, and reusable by a future PNG exporter for putting a
//! treemap in a bug report.
//!
//! **Why squarified rather than the obvious slice-and-dice.** The entire point
//! of a treemap is that area is comparable by eye. A naive algorithm produces
//! rectangles hundreds of times longer than they are wide, and the eye reads a
//! long thin sliver as smaller than a square of the same area. An aspect ratio
//! that is not bounded does not merely look worse — it makes the chart lie
//! about the one thing it exists to show.

/// Where one item ended up, in the caller's coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    /// Index into the values that were laid out.
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Tile {
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// How far from square. 1.0 is a square; larger is worse, either way up.
    pub fn aspect(&self) -> f32 {
        if self.width <= 0.0 || self.height <= 0.0 {
            return f32::INFINITY;
        }
        (self.width / self.height).max(self.height / self.width)
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// The rectangle being filled.
#[derive(Debug, Clone, Copy)]
struct Region {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Region {
    fn shorter(&self) -> f32 {
        self.width.min(self.height)
    }
}

/// Lay `values` out in `width` × `height`, filling it completely.
///
/// Values are taken in the order given; the caller sorts. Sorting descending
/// is what makes the result squarified rather than merely tiled, and leaving
/// it to the caller means the returned indices still point at the caller's
/// own ordering.
///
/// Zero and negative values are skipped rather than given a degenerate
/// rectangle — a symbol of no size is not a region of the binary.
pub fn squarify(values: &[u64], width: f32, height: f32) -> Vec<Tile> {
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }

    let live: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter(|(_, value)| **value > 0)
        .map(|(index, value)| (index, *value as f64))
        .collect();
    if live.is_empty() {
        return Vec::new();
    }

    let total: f64 = live.iter().map(|(_, value)| value).sum();
    // Values become areas, so the sum of the areas is the whole rectangle.
    let scale = (width as f64 * height as f64) / total;

    let mut tiles = Vec::with_capacity(live.len());
    let mut region = Region { x: 0.0, y: 0.0, width, height };
    let mut remaining = &live[..];
    let mut row: Vec<(usize, f64)> = Vec::new();

    while !remaining.is_empty() {
        let (index, value) = remaining[0];
        let area = value * scale;

        // Does adding this item to the current row make the worst aspect
        // ratio better or worse? That question is the whole algorithm.
        let without = worst_aspect(&row, region.shorter());
        let mut candidate = row.clone();
        candidate.push((index, area));
        let with = worst_aspect(&candidate, region.shorter());

        if row.is_empty() || with <= without {
            row = candidate;
            remaining = &remaining[1..];
        } else {
            // Adding it would make the row worse, so close the row and start
            // again in what is left.
            region = place_row(&row, region, &mut tiles);
            row.clear();
        }
    }
    if !row.is_empty() {
        place_row(&row, region, &mut tiles);
    }

    tiles
}

/// The worst aspect ratio in a row laid along a side of length `side`.
fn worst_aspect(row: &[(usize, f64)], side: f32) -> f64 {
    if row.is_empty() || side <= 0.0 {
        return f64::INFINITY;
    }
    let total: f64 = row.iter().map(|(_, area)| area).sum();
    if total <= 0.0 {
        return f64::INFINITY;
    }
    let largest = row.iter().map(|(_, area)| *area).fold(0.0f64, f64::max);
    let smallest = row.iter().map(|(_, area)| *area).fold(f64::INFINITY, f64::min);
    let side = side as f64;

    // Bruls et al.'s formula: the worse of the two extremes in the row.
    let a = (side * side * largest) / (total * total);
    let b = (total * total) / (side * side * smallest);
    a.max(b)
}

/// Place a finished row along the shorter side, and return what is left.
fn place_row(row: &[(usize, f64)], region: Region, tiles: &mut Vec<Tile>) -> Region {
    let total: f64 = row.iter().map(|(_, area)| area).sum();
    if total <= 0.0 {
        return region;
    }

    // Rows run along the shorter side, which is what keeps them square-ish.
    let along_width = region.width <= region.height;
    let depth = (total / region.shorter() as f64) as f32;

    // The depth can only ever be what is left, whatever the arithmetic says.
    let depth = depth.min(if along_width { region.height } else { region.width });
    let side = region.shorter();

    let mut offset = 0.0f32;
    for (position, (index, area)) in row.iter().enumerate() {
        // The last tile takes exactly what remains rather than its computed
        // share. Twenty-four rectangles of accumulated float error put a tile
        // 0.012px outside its region — invisible, and still a rectangle
        // painted outside the canvas it belongs to.
        let length = if position + 1 == row.len() {
            (side - offset).max(0.0)
        } else {
            (area / total) as f32 * side
        };
        if along_width {
            tiles.push(Tile {
                index: *index,
                x: region.x + offset,
                y: region.y,
                width: length,
                height: depth,
            });
        } else {
            tiles.push(Tile {
                index: *index,
                x: region.x,
                y: region.y + offset,
                width: depth,
                height: length,
            });
        }
        offset += length;
    }

    if along_width {
        Region {
            x: region.x,
            y: region.y + depth,
            width: region.width,
            height: (region.height - depth).max(0.0),
        }
    } else {
        Region {
            x: region.x + depth,
            y: region.y,
            width: (region.width - depth).max(0.0),
            height: region.height,
        }
    }
}

/// Which tile is under a point.
///
/// A linear scan, which is correct and fast enough for the hundreds of tiles a
/// treemap can usefully show at once. DESIGN-GUI §6.1 notes that a spatial
/// index is wanted "once you exceed a few thousand rectangles" — and a
/// treemap showing a few thousand rectangles has already stopped being
/// readable, so the fix for that is culling, not an index.
pub fn hit(tiles: &[Tile], x: f32, y: f32) -> Option<&Tile> {
    tiles.iter().find(|tile| tile.contains(x, y))
}

/// Drop tiles too small to see, and say how many went.
///
/// Rectangles under a couple of pixels convey nothing and cost everything —
/// they are a smear of borders. Returning the count lets the interface say
/// "and 4,812 smaller" rather than silently showing part of the binary.
pub fn cull(tiles: Vec<Tile>, minimum: f32) -> (Vec<Tile>, usize, f32) {
    let before = tiles.len();
    let mut hidden_area = 0.0;
    let kept: Vec<Tile> = tiles
        .into_iter()
        .filter(|tile| {
            let visible = tile.width >= minimum && tile.height >= minimum;
            if !visible {
                hidden_area += tile.area();
            }
            visible
        })
        .collect();
    let hidden = before - kept.len();
    (kept, hidden, hidden_area)
}
