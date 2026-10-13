//! The app's icon (docs/mock/logo.svg): two breakwaters, cyan and gold, with
//! a light on each tip, reaching out from a dark tile and leaving a mouth
//! with calm blue water inside. Drawn here at any size so no image file is
//! carried.

type Rgb = [u8; 3];

/// The tile's colour and its rim (the logo's `#16181d` and `#2c3039`).
const TILE: Rgb = [0x16, 0x18, 0x1d];
const RIM: Rgb = [0x2c, 0x30, 0x39];
const CYAN: Rgb = [0x6f, 0xd0, 0xd0];
const GOLD: Rgb = [0xe8, 0xc8, 0x7a];
const BLUE: Rgb = [0x7a, 0xb8, 0xf5];

/// One cubic Bézier in the logo's 16-unit box.
type Cubic = [[f32; 2]; 4];

/// The breakwaters: `M2.7 13.3C2.7 8.4 3.5 5.6 5.7 4.6` and its mirror.
const CYAN_WALL: Cubic = [[2.7, 13.3], [2.7, 8.4], [3.5, 5.6], [5.7, 4.6]];
const GOLD_WALL: Cubic = [[13.3, 13.3], [13.3, 8.4], [12.5, 5.6], [10.3, 4.6]];
/// The wave: `M5.4 10.2c.9-.75 1.75-.75 2.6 0s1.75.75 2.6 0` (two cubics).
const WAVE: [Cubic; 2] = [[[5.4, 10.2], [6.3, 9.45], [7.15, 9.45], [8.0, 10.2]], [[8.0, 10.2], [8.85, 10.95], [9.7, 10.95], [10.6, 10.2]]];

/// The icon as RGBA, `size` × `size`.
pub fn pixels(size: u32) -> Vec<u8> {
    let n = size as usize;
    let unit = size as f32 / 16.0;
    // Thicker at small sizes so the walls survive 16 px.
    let boost = if size <= 16 {
        1.35
    } else if size <= 32 {
        1.15
    } else {
        1.0
    };
    let wall = 1.5 * unit * boost;
    let wave = 1.3 * unit * boost;
    let cyan = curve(&[CYAN_WALL], unit);
    let gold = curve(&[GOLD_WALL], unit);
    let blue = curve(&WAVE, unit);
    let lights = [([5.7 * unit, 4.6 * unit], CYAN), ([10.3 * unit, 4.6 * unit], GOLD)];
    let light_r = 1.1 * unit * boost;
    let radius = 3.4 * unit;
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let at = [x as f32 + 0.5, y as f32 + 0.5];
            let tile = rounded(at, size as f32, radius);
            if tile <= 0.0 {
                continue;
            }
            let mut rgb = TILE.map(f32::from);
            // The rim: a half-unit band inside the edge.
            let inside = rounded(at, size as f32 - 0.5 * unit, radius - 0.25 * unit);
            blend(&mut rgb, RIM, 1.0 - inside);
            for (line, width, color) in [(&cyan, wall, CYAN), (&gold, wall, GOLD), (&blue, wave, BLUE)] {
                blend(&mut rgb, color, (width / 2.0 - distance(at, line) + 0.5).clamp(0.0, 1.0));
            }
            for (c, color) in lights {
                let d = ((at[0] - c[0]).powi(2) + (at[1] - c[1]).powi(2)).sqrt();
                blend(&mut rgb, color, (light_r - d + 0.5).clamp(0.0, 1.0));
            }
            let i = (y * n + x) * 4;
            out[i..i + 3].copy_from_slice(&rgb.map(|v| v.round() as u8));
            out[i + 3] = (tile * 255.0).round() as u8;
        }
    }
    out
}

fn blend(rgb: &mut [f32; 3], color: Rgb, cover: f32) {
    for k in 0..3 {
        rgb[k] += (f32::from(color[k]) - rgb[k]) * cover;
    }
}

/// The cubics as a run of points, in pixels.
fn curve(segs: &[Cubic], unit: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    for seg in segs {
        for k in 0..=24 {
            let t = k as f32 / 24.0;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            let x: f32 = (0..4).map(|j| w[j] * seg[j][0]).sum();
            let y: f32 = (0..4).map(|j| w[j] * seg[j][1]).sum();
            out.push([x * unit, y * unit]);
        }
    }
    out
}

/// How far `p` is from the run of points (its segments).
fn distance(p: [f32; 2], line: &[[f32; 2]]) -> f32 {
    line.windows(2)
        .map(|s| {
            let (a, b) = (s[0], s[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = dx * dx + dy * dy;
            let t = if len == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len).clamp(0.0, 1.0) };
            let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
            ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
        })
        .fold(f32::MAX, f32::min)
}

/// How much of the pixel at `p` is inside the rounded tile, 0 to 1.
fn rounded(p: [f32; 2], size: f32, r: f32) -> f32 {
    let q = |v: f32| (v - size / 2.0).abs() - (size / 2.0 - r);
    let (qx, qy) = (q(p[0]), q(p[1]));
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
    (0.5 - outside).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_has_both_walls_the_lights_and_the_wave() {
        let px = pixels(128);
        assert_eq!(px.len(), 128 * 128 * 4);
        let at = |ux: f32, uy: f32| {
            let (x, y) = ((ux * 8.0) as usize, (uy * 8.0) as usize);
            &px[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4]
        };
        assert_eq!(at(0.1, 0.1)[3], 0, "the corner is cut round");
        assert_eq!(at(8.0, 2.0), &[0x16, 0x18, 0x1d, 255], "the tile");
        let cyan = at(5.7, 4.6);
        assert!(cyan[1] > 0xb0 && cyan[0] < 0x90, "the cyan light: {cyan:?}");
        let gold = at(10.3, 4.6);
        assert!(gold[0] > 0xc0 && gold[2] < 0x90, "the gold light: {gold:?}");
        let blue = at(6.3, 9.7);
        assert!(blue[2] > 0xc0 && blue[0] < 0x90, "the wave: {blue:?}");
        assert_eq!(pixels(16).len(), 16 * 16 * 4);
    }
}
