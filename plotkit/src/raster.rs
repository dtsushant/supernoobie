//! # A software rasteriser
//!
//! Every pixel on screen is written by the CPU, one at a time. There is no
//! GPU here, no shader, no graphics API — just a `Vec<u32>` you fill in and
//! hand to the window.
//!
//! ```text
//! buf[y * width + x] = 0x00RRGGBB
//! ```
//!
//! That is genuinely all a framebuffer is. Everything below — lines, circles,
//! text — is a loop that decides which indices to write.
//!
//! Worth knowing where the ceiling is: at 1200x780 a full clear is ~936,000
//! writes, and a 60 Hz frame budget is 16.7 ms. A modern core manages that
//! comfortably for 2D shapes. It is filling *lit, textured* pixels in 3D that
//! eventually forces a GPU — not the physics, and not this.

/// Colours in `0x00RRGGBB`, the format minifb expects.
pub mod colour {
    pub const BG: u32 = 0x0B1017;
    pub const GRID: u32 = 0x161F29;
    pub const LINE: u32 = 0x22303C;
    pub const INK: u32 = 0xE3E9EF;
    pub const SOFT: u32 = 0x94A1AE;
    pub const FAINT: u32 = 0x6B7987;
    pub const REAL: u32 = 0xE0A44A; // amber  — real axis, mass 1
    pub const IMAG: u32 = 0x4FBCD4; // teal   — imaginary axis, tangent
    pub const MOD: u32 = 0xE585AC; // rose   — modulus, mass 2, markers
    pub const ROPE: u32 = 0x8CA0B3;
    pub const GOOD: u32 = 0x6FCF97;
    pub const WARN: u32 = 0xE0704A;
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub buf: Vec<u32>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Canvas { w: w as i32, h: h as i32, buf: vec![colour::BG; w * h] }
    }

    pub fn clear(&mut self, c: u32) {
        self.buf.fill(c);
    }

    /// The one primitive. Everything else is a loop around this.
    #[inline]
    pub fn px(&mut self, x: i32, y: i32, c: u32) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.buf[(y * self.w + x) as usize] = c;
        }
    }

    /// Blend `c` over what is already there, `a` in 0..=255.
    #[inline]
    pub fn px_blend(&mut self, x: i32, y: i32, c: u32, a: u32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = (y * self.w + x) as usize;
        let d = self.buf[i];
        let mix = |sh: u32| {
            let s = (c >> sh) & 0xFF;
            let t = (d >> sh) & 0xFF;
            ((s * a + t * (255 - a)) / 255) << sh
        };
        self.buf[i] = mix(16) | mix(8) | mix(0);
    }

    /// Bresenham: step along the long axis, and carry an error term that says
    /// when to step the short axis. All integer arithmetic, no division.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u32) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.px(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Thickness by drawing parallel copies offset along the normal.
    /// Crude, but it is three lines and it reads.
    pub fn thick_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: u32) {
        if t <= 1 {
            return self.line(x0, y0, x1, y1, c);
        }
        let (dx, dy) = ((x1 - x0) as f32, (y1 - y0) as f32);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len, dx / len); // the normal = direction * i
        for k in -(t / 2)..=(t / 2) {
            let (ox, oy) = ((nx * k as f32).round() as i32, (ny * k as f32).round() as i32);
            self.line(x0 + ox, y0 + oy, x1 + ox, y1 + oy, c);
        }
    }

    pub fn polyline(&mut self, pts: &[(i32, i32)], t: i32, c: u32) {
        for w in pts.windows(2) {
            self.thick_line(w[0].0, w[0].1, w[1].0, w[1].1, t, c);
        }
    }

    /// Midpoint circle: exploit 8-fold symmetry, so only an eighth is walked.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: u32) {
        let (mut x, mut y, mut d) = (r, 0, 1 - r);
        while x >= y {
            for (a, b) in [
                (x, y), (y, x), (-y, x), (-x, y),
                (-x, -y), (-y, -x), (y, -x), (x, -y),
            ] {
                self.px(cx + a, cy + b, c);
            }
            y += 1;
            if d < 0 {
                d += 2 * y + 1;
            } else {
                x -= 1;
                d += 2 * (y - x) + 1;
            }
        }
    }

    pub fn ring(&mut self, cx: i32, cy: i32, r: i32, t: i32, c: u32) {
        for k in 0..t {
            self.circle(cx, cy, r - k, c);
        }
    }

    pub fn disc(&mut self, cx: i32, cy: i32, r: i32, c: u32) {
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f32).sqrt() as i32;
            for dx in -half..=half {
                self.px(cx + dx, cy + dy, c);
            }
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        self.line(x, y, x + w, y, c);
        self.line(x, y + h, x + w, y + h, c);
        self.line(x, y, x, y + h, c);
        self.line(x + w, y, x + w, y + h, c);
    }

    /// The same picture, as Unicode braille — two pixels wide and four tall
    /// per character, so a terminal shows what the window would.
    ///
    /// A braille cell has eight dots in a 2×4 block and one code point per
    /// subset of them, which is 256 characters starting at `U+2800`. That is
    /// exactly a byte, so the cell is built by setting bits.
    ///
    /// Anything equal to `bg` counts as empty. With `colour`, each cell is
    /// tinted by the first lit pixel in it using 24-bit ANSI.
    pub fn braille(&self, bg: u32, colour: bool) -> String {
        // Which bit each of the eight positions owns. Braille numbers its
        // dots down the left column then down the right, which is why this
        // table is not simply 1, 2, 4, 8 in reading order.
        const DOT: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

        let mut s = String::new();
        let mut cur: Option<u32> = None;
        for cy in (0..self.h).step_by(4) {
            for cx in (0..self.w).step_by(2) {
                let mut bits = 0u8;
                let mut lit: Option<u32> = None;
                for (dy, row) in DOT.iter().enumerate() {
                    for (dx, bit) in row.iter().enumerate() {
                        let (x, y) = (cx + dx as i32, cy + dy as i32);
                        if x >= self.w || y >= self.h {
                            continue;
                        }
                        let p = self.buf[(y * self.w + x) as usize];
                        if p != bg {
                            bits |= bit;
                            lit.get_or_insert(p);
                        }
                    }
                }
                if colour {
                    if let Some(want) = lit {
                        if cur != Some(want) {
                            s.push_str(&format!("\x1b[38;2;{};{};{}m", (want >> 16) & 255, (want >> 8) & 255, want & 255));
                            cur = Some(want);
                        }
                    }
                }
                s.push(char::from_u32(0x2800 + bits as u32).expect("braille block"));
            }
            s.push('\n');
        }
        if colour {
            s.push_str("\x1b[0m");
        }
        s
    }

    /// Draw text with the 5x7 font.
    ///
    /// The table covers ASCII 32..95, so lowercase is folded to uppercase and
    /// a few symbols above `_` are patched in by hand. Anything else becomes
    /// `?` — which is how `|L|` once rendered as `?L?` in the 3-D demo.
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: u32, scale: i32) {
        let mut cx = x;
        for ch in s.chars() {
            let up = ch.to_ascii_uppercase() as usize;
            let glyph = match up {
                32..=95 => FONT[up - 32],
                124 => [0x00, 0x00, 0x7F, 0x00, 0x00], // |
                123 => [0x00, 0x08, 0x36, 0x41, 0x00], // {
                125 => [0x00, 0x41, 0x36, 0x08, 0x00], // }
                126 => [0x08, 0x04, 0x08, 0x10, 0x08], // ~
                _ => FONT[31],                         // ?
            };
            for (col, bits) in glyph.iter().enumerate() {
                for row in 0..7 {
                    if (bits >> row) & 1 == 1 {
                        let px = cx + col as i32 * scale;
                        let py = y + row * scale;
                        if scale == 1 {
                            self.px(px, py, c);
                        } else {
                            self.fill_rect(px, py, scale, scale, c);
                        }
                    }
                }
            }
            cx += 6 * scale;
        }
    }

    pub fn text_w(s: &str, scale: i32) -> i32 {
        s.chars().count() as i32 * 6 * scale
    }

    /// Write the framebuffer out as a PNG, by hand.
    ///
    /// A PNG is: an 8-byte signature, then length-tagged chunks each ending in
    /// a CRC. The pixels live in `IDAT` as a zlib stream — and zlib permits
    /// **stored** (uncompressed) deflate blocks, which means a valid PNG can be
    /// written with no compression code at all. Every scanline is prefixed with
    /// a filter byte; `0` means "no filter".
    ///
    /// That is the whole format. Roughly sixty lines, no dependencies.
    pub fn write_png(&self, path: &str) -> std::io::Result<()> {
        let (w, h) = (self.w as usize, self.h as usize);

        // scanlines: one filter byte, then RGB triples
        let mut raw = Vec::with_capacity((w * 3 + 1) * h);
        for y in 0..h {
            raw.push(0u8); // filter type 0 = none
            for x in 0..w {
                let p = self.buf[y * w + x];
                raw.push((p >> 16) as u8);
                raw.push((p >> 8) as u8);
                raw.push(p as u8);
            }
        }

        // zlib wrapper: 0x78 0x01, stored deflate blocks, adler32
        let mut z = vec![0x78u8, 0x01];
        let mut i = 0usize;
        while i < raw.len() {
            let n = (raw.len() - i).min(65_535);
            z.push(if i + n >= raw.len() { 1 } else { 0 }); // BFINAL, BTYPE=00
            z.extend_from_slice(&(n as u16).to_le_bytes());
            z.extend_from_slice(&(!(n as u16)).to_le_bytes()); // one's complement
            z.extend_from_slice(&raw[i..i + n]);
            i += n;
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour RGB

        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        std::fs::write(path, out)
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// The standard CRC-32, generated bit by bit so no lookup table is needed.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Adler-32: two running sums mod 65521. Weaker than CRC but far cheaper,
/// which is why zlib uses it for the stream checksum.
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// 5x7 bitmap font, ASCII 32..95. One byte per column, bit 0 = top row.
/// 320 bytes for the whole alphabet — this is how every embedded display and
/// every 1980s machine drew text.
#[rustfmt::skip]
const FONT: [[u8; 5]; 64] = [
    [0x00,0x00,0x00,0x00,0x00], // space
    [0x00,0x00,0x5F,0x00,0x00], // !
    [0x00,0x07,0x00,0x07,0x00], // "
    [0x14,0x7F,0x14,0x7F,0x14], // #
    [0x24,0x2A,0x7F,0x2A,0x12], // $
    [0x23,0x13,0x08,0x64,0x62], // %
    [0x36,0x49,0x55,0x22,0x50], // &
    [0x00,0x05,0x03,0x00,0x00], // '
    [0x00,0x1C,0x22,0x41,0x00], // (
    [0x00,0x41,0x22,0x1C,0x00], // )
    [0x14,0x08,0x3E,0x08,0x14], // *
    [0x08,0x08,0x3E,0x08,0x08], // +
    [0x00,0x50,0x30,0x00,0x00], // ,
    [0x08,0x08,0x08,0x08,0x08], // -
    [0x00,0x60,0x60,0x00,0x00], // .
    [0x20,0x10,0x08,0x04,0x02], // /
    [0x3E,0x51,0x49,0x45,0x3E], // 0
    [0x00,0x42,0x7F,0x40,0x00], // 1
    [0x42,0x61,0x51,0x49,0x46], // 2
    [0x21,0x41,0x45,0x4B,0x31], // 3
    [0x18,0x14,0x12,0x7F,0x10], // 4
    [0x27,0x45,0x45,0x45,0x39], // 5
    [0x3C,0x4A,0x49,0x49,0x30], // 6
    [0x01,0x71,0x09,0x05,0x03], // 7
    [0x36,0x49,0x49,0x49,0x36], // 8
    [0x06,0x49,0x49,0x29,0x1E], // 9
    [0x00,0x36,0x36,0x00,0x00], // :
    [0x00,0x56,0x36,0x00,0x00], // ;
    [0x08,0x14,0x22,0x41,0x00], // <
    [0x14,0x14,0x14,0x14,0x14], // =
    [0x00,0x41,0x22,0x14,0x08], // >
    [0x02,0x01,0x51,0x09,0x06], // ?
    [0x32,0x49,0x79,0x41,0x3E], // @
    [0x7E,0x11,0x11,0x11,0x7E], // A
    [0x7F,0x49,0x49,0x49,0x36], // B
    [0x3E,0x41,0x41,0x41,0x22], // C
    [0x7F,0x41,0x41,0x22,0x1C], // D
    [0x7F,0x49,0x49,0x49,0x41], // E
    [0x7F,0x09,0x09,0x09,0x01], // F
    [0x3E,0x41,0x49,0x49,0x7A], // G
    [0x7F,0x08,0x08,0x08,0x7F], // H
    [0x00,0x41,0x7F,0x41,0x00], // I
    [0x20,0x40,0x41,0x3F,0x01], // J
    [0x7F,0x08,0x14,0x22,0x41], // K
    [0x7F,0x40,0x40,0x40,0x40], // L
    [0x7F,0x02,0x0C,0x02,0x7F], // M
    [0x7F,0x04,0x08,0x10,0x7F], // N
    [0x3E,0x41,0x41,0x41,0x3E], // O
    [0x7F,0x09,0x09,0x09,0x06], // P
    [0x3E,0x41,0x51,0x21,0x5E], // Q
    [0x7F,0x09,0x19,0x29,0x46], // R
    [0x46,0x49,0x49,0x49,0x31], // S
    [0x01,0x01,0x7F,0x01,0x01], // T
    [0x3F,0x40,0x40,0x40,0x3F], // U
    [0x1F,0x20,0x40,0x20,0x1F], // V
    [0x3F,0x40,0x38,0x40,0x3F], // W
    [0x63,0x14,0x08,0x14,0x63], // X
    [0x07,0x08,0x70,0x08,0x07], // Y
    [0x61,0x51,0x49,0x45,0x43], // Z
    [0x00,0x7F,0x41,0x41,0x00], // [
    [0x02,0x04,0x08,0x10,0x20], // backslash
    [0x00,0x41,0x41,0x7F,0x00], // ]
    [0x04,0x02,0x01,0x02,0x04], // ^
    [0x40,0x40,0x40,0x40,0x40], // _
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_is_width_times_height() {
        let c = Canvas::new(40, 25);
        assert_eq!(c.buf.len(), 1000);
    }

    /// Writing outside the buffer must be silently dropped, not panic and not
    /// wrap onto the next row.
    #[test]
    fn out_of_bounds_pixels_are_dropped() {
        let mut c = Canvas::new(10, 10);
        c.clear(0);
        c.px(-1, 5, 0xFFFFFF);
        c.px(10, 5, 0xFFFFFF);
        c.px(5, -1, 0xFFFFFF);
        c.px(5, 10, 0xFFFFFF);
        assert!(c.buf.iter().all(|&p| p == 0), "something leaked into the buffer");
    }

    #[test]
    fn a_line_reaches_both_endpoints() {
        let mut c = Canvas::new(60, 40);
        c.clear(0);
        c.line(3, 5, 50, 33, 0xFFFFFF);
        assert_eq!(c.buf[(5 * 60 + 3) as usize], 0xFFFFFF);
        assert_eq!(c.buf[(33 * 60 + 50) as usize], 0xFFFFFF);
    }

    /// Every plotted point of a circle must be within a pixel of the radius.
    #[test]
    fn circle_points_sit_on_the_radius() {
        let mut c = Canvas::new(80, 80);
        c.clear(0);
        let r = 30;
        c.circle(40, 40, r, 0xFFFFFF);
        let mut found = 0;
        for y in 0..80i32 {
            for x in 0..80i32 {
                if c.buf[(y * 80 + x) as usize] != 0 {
                    found += 1;
                    let (dx, dy) = ((x - 40) as f64, (y - 40) as f64);
                    let d = (dx * dx + dy * dy).sqrt();
                    assert!((d - r as f64).abs() < 1.5, "point at distance {d}, want {r}");
                }
            }
        }
        assert!(found > 100, "circle drew only {found} pixels");
    }

    #[test]
    fn text_stays_inside_the_canvas() {
        let mut c = Canvas::new(200, 40);
        c.clear(0);
        c.text(2, 2, "THETA = -1.234 RAD", colour::INK, 2);
        assert!(c.buf.iter().any(|&p| p == colour::INK), "nothing was drawn");
    }

    /// Blending with alpha 0 leaves the destination alone; 255 replaces it.
    #[test]
    fn blend_endpoints_behave() {
        let mut c = Canvas::new(4, 4);
        c.clear(0x000000);
        c.px_blend(1, 1, 0xFFFFFF, 0);
        assert_eq!(c.buf[5], 0x000000);
        c.px_blend(1, 1, 0xFFFFFF, 255);
        assert_eq!(c.buf[5], 0xFFFFFF);
    }
}
