//! # Frames — a layer of styled shapes
//!
//! A [`Frame`] is a **layer**, not a time step. Things drawn together, in
//! order, with styles.
//!
//! Animation is then a plain function `f(t) -> Frame`, which means a still
//! picture is just `t = 0` and scrubbing comes free without inventing a second
//! concept. That is the whole reason to prefer "layer" over "time step" here.
//!
//! ```no_run
//! # use plotkit::{frame::Frame, shape::Shape, complex::Cx};
//! fn scene(t: f64) -> Frame {
//!     let mut f = Frame::new();
//!     f.add(Shape::circle(Cx::ZERO, 1.0));
//!     let r = Cx::expi(t);
//!     f.add(Shape::unit_square().map(move |z| r * z)).color(0xE0A44A);
//!     f
//! }
//! ```

use crate::complex::Cx;
use crate::plot;
use crate::raster::Canvas;
use crate::shape::Shape;
use crate::view::View;

/// Colours a frame cycles through when nothing says otherwise. Chosen to stay
/// apart from each other on a dark ground.
pub const PALETTE: [u32; 6] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0xE0704A];

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub colour: u32,
    /// Line thickness in pixels — a constant on screen at any zoom.
    pub width: i32,
    /// Radius of a point mark, in pixels.
    pub dot: f64,
    /// Draw the vertices of a path as well as its edges.
    pub show_vertices: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style { colour: PALETTE[0], width: 2, dot: 5.0, show_vertices: false }
    }
}

#[derive(Clone)]
pub struct Frame {
    items: Vec<(Shape, Style)>,
    next_colour: usize,
    /// Text pinned to world positions — labels that travel with the drawing.
    labels: Vec<(Cx, String, u32, i32)>,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    pub fn new() -> Self {
        Frame { items: Vec::new(), next_colour: 0, labels: Vec::new() }
    }

    /// Add a shape, taking the next palette colour. Returns its style so it
    /// can be adjusted:
    ///
    /// ```text
    /// f.add(Shape::circle(z, 1.0)).color(0xE0A44A).width(3);
    /// ```
    pub fn add(&mut self, s: Shape) -> StyleRef<'_> {
        let st = Style { colour: PALETTE[self.next_colour % PALETTE.len()], ..Style::default() };
        self.next_colour += 1;
        self.items.push((s, st));
        let k = self.items.len() - 1;
        StyleRef { f: self, k }
    }

    /// Text at a world position, so it moves with whatever it labels.
    pub fn label(&mut self, at: Cx, text: impl Into<String>, colour: u32, scale: i32) {
        self.labels.push((at, text.into(), colour, scale));
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Merge another frame in, keeping its styles. Useful for building a scene
    /// out of parts that each know their own colours.
    pub fn merge(&mut self, other: Frame) {
        self.items.extend(other.items);
        self.labels.extend(other.labels);
    }

    /// The box everything in the frame fits inside, as `(bottom-left,
    /// top-right)`, or `None` if there is nothing to measure.
    ///
    /// A view has to be supplied because some shapes only exist relative to
    /// one — `Graph` is sampled across whatever is on screen, `Implicit` is
    /// marched over it. For those the answer is the view you gave, which is
    /// the honest result: they fill whatever they are shown in.
    pub fn bounds(&self, v: &View) -> Option<(Cx, Cx)> {
        let (lo, hi) = plot::bounds(v);
        let (mut min, mut max) = (Cx::new(f64::MAX, f64::MAX), Cx::new(f64::MIN, f64::MIN));
        let mut any = false;
        for (shape, _) in &self.items {
            for p in shape.polylines(lo, hi, v.w as usize).into_iter().flatten() {
                if !p.re.is_finite() || !p.im.is_finite() {
                    continue;
                }
                any = true;
                min = Cx::new(min.re.min(p.re), min.im.min(p.im));
                max = Cx::new(max.re.max(p.re), max.im.max(p.im));
            }
        }
        any.then_some((min, max))
    }

    /// Render onto a canvas through a view.
    pub fn draw(&self, c: &mut Canvas, v: &View) {
        let (lo, hi) = plot::bounds(v);
        for (shape, st) in &self.items {
            let dot_world = st.dot / v.scale.max(1e-9);
            for run in shape.polylines(lo, hi, v.w as usize) {
                match run.len() {
                    0 => {}
                    1 => v.disc(c, run[0], dot_world, st.colour),
                    _ => {
                        if shape.is_points() {
                            for p in &run {
                                v.disc(c, *p, dot_world, st.colour);
                            }
                        } else {
                            for w in run.windows(2) {
                                v.line(c, w[0], w[1], st.width, st.colour);
                            }
                            if st.show_vertices {
                                for p in &run {
                                    v.disc(c, *p, dot_world, st.colour);
                                }
                            }
                        }
                    }
                }
            }
        }
        for (at, text, colour, scale) in &self.labels {
            v.text_mid(c, *at, text, *colour, *scale);
        }
    }
}

/// Returned by [`Frame::add`] so a shape can be styled where it is added.
pub struct StyleRef<'a> {
    f: &'a mut Frame,
    k: usize,
}

impl StyleRef<'_> {
    pub fn color(self, c: u32) -> Self {
        self.f.items[self.k].1.colour = c;
        self
    }
    pub fn width(self, w: i32) -> Self {
        self.f.items[self.k].1.width = w;
        self
    }
    pub fn dot(self, r: f64) -> Self {
        self.f.items[self.k].1.dot = r;
        self
    }
    pub fn vertices(self, on: bool) -> Self {
        self.f.items[self.k].1.show_vertices = on;
        self
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn painted(c: &Canvas) -> usize {
        c.buf.iter().filter(|&&p| p != 0).count()
    }

    #[test]
    fn a_frame_draws_what_it_was_given() {
        let v = View::centred(200, 200, 40.0);
        let mut c = Canvas::new(200, 200);
        c.clear(0);
        let mut f = Frame::new();
        f.add(Shape::polygon(vec![Cx::ZERO, Cx::new(1.0, 2.0)]));
        assert_eq!(f.len(), 1);
        f.draw(&mut c, &v);
        assert!(painted(&c) > 40, "only {} pixels", painted(&c));
    }

    /// The line lands where the numbers say, not somewhere else.
    #[test]
    fn geometry_lands_where_it_should() {
        let v = View::centred(200, 200, 40.0);
        let mut c = Canvas::new(200, 200);
        c.clear(0);
        let mut f = Frame::new();
        f.add(Shape::polygon(vec![Cx::ZERO, Cx::new(1.0, 2.0)]));
        f.draw(&mut c, &v);
        let near = |w: Cx| {
            let (x, y) = v.to_screen(w);
            (-2..=2).any(|dy| {
                (-2..=2).any(|dx| {
                    let (px, py) = (x + dx, y + dy);
                    px >= 0 && py >= 0 && px < 200 && py < 200 && c.buf[(py * 200 + px) as usize] != 0
                })
            })
        };
        assert!(near(Cx::ZERO));
        assert!(near(Cx::new(1.0, 2.0)));
        assert!(!near(Cx::new(-2.0, 2.0)));
    }

    /// Shapes take different palette colours unless told otherwise, so a
    /// scene of bare `add` calls is still readable.
    #[test]
    fn colours_cycle_and_can_be_pinned() {
        let v = View::centred(120, 120, 25.0);

        let mut a = Canvas::new(120, 120);
        a.clear(0);
        let mut f = Frame::new();
        f.add(Shape::polygon(vec![Cx::new(-1.0, 0.0), Cx::new(1.0, 0.0)]));
        f.add(Shape::polygon(vec![Cx::new(0.0, -1.0), Cx::new(0.0, 1.0)]));
        f.draw(&mut a, &v);
        let mut shades: Vec<u32> = a.buf.iter().copied().filter(|&p| p != 0).collect();
        shades.sort_unstable();
        shades.dedup();
        assert!(shades.len() >= 2, "the two shapes should differ in colour");

        let mut b = Canvas::new(120, 120);
        b.clear(0);
        let mut g = Frame::new();
        g.add(Shape::polygon(vec![Cx::new(-1.0, 0.0), Cx::new(1.0, 0.0)])).color(0xFF0000);
        g.add(Shape::polygon(vec![Cx::new(0.0, -1.0), Cx::new(0.0, 1.0)])).color(0xFF0000);
        g.draw(&mut b, &v);
        let mut pinned: Vec<u32> = b.buf.iter().copied().filter(|&p| p != 0).collect();
        pinned.sort_unstable();
        pinned.dedup();
        assert_eq!(pinned, vec![0xFF0000]);
    }

    /// ★ Animation is `f(t) -> Frame`, so a still is just `t = 0` and there
    /// is no second concept to learn.
    #[test]
    fn a_scene_function_is_all_animation_needs() {
        let scene = |t: f64| {
            let mut f = Frame::new();
            let r = Cx::expi(t);
            f.add(Shape::point(Cx::new(1.0, 0.0)).map(move |z| r * z));
            f
        };
        let v = View::centred(100, 100, 30.0);
        let at = |t: f64| {
            let mut c = Canvas::new(100, 100);
            c.clear(0);
            scene(t).draw(&mut c, &v);
            c.buf.iter().position(|&p| p != 0).unwrap()
        };
        assert_ne!(at(0.0), at(std::f64::consts::FRAC_PI_2), "the frame should move with t");
        assert_eq!(at(0.0), at(std::f64::consts::TAU), "and come back after a full turn");
    }

    #[test]
    fn merging_keeps_both_sets_of_styles() {
        let mut a = Frame::new();
        a.add(Shape::point(Cx::ZERO)).color(0x111111);
        let mut b = Frame::new();
        b.add(Shape::point(Cx::new(1.0, 1.0))).color(0x222222);
        a.merge(b);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn an_empty_frame_draws_nothing_and_does_not_panic() {
        let v = View::centred(60, 60, 10.0);
        let mut c = Canvas::new(60, 60);
        c.clear(0);
        Frame::new().draw(&mut c, &v);
        assert_eq!(painted(&c), 0);
    }
}
