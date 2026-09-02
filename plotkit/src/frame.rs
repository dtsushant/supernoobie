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

/// Something that can put itself at a point.
///
/// A [`Shape`] moves. Other things do better by being rebuilt: a wave spanning
/// the whole window has no endpoints to shift, so moving it means changing
/// where its `x` is measured from, not translating the samples — which would
/// leave a bare strip at one edge.
pub trait Placeable {
    fn placed(self, at: Cx) -> Shape;
}

impl Placeable for Shape {
    fn placed(self, at: Cx) -> Shape {
        self.at(at)
    }
}

/// Where on the window a pinned caption sits.
///
/// Nine positions, the ones a caption is ever wanted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Middle,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    /// The point it hangs from, and how much of the text hangs back past it —
    /// `0` for left/top, `0.5` for centred, `1` for right/bottom.
    fn spot(self, w: f64, h: f64) -> (f64, f64, f64, f64) {
        use Anchor::*;
        let (x, fx) = match self {
            TopLeft | Left | BottomLeft => (0.0, 0.0),
            Top | Middle | Bottom => (w / 2.0, 0.5),
            TopRight | Right | BottomRight => (w, 1.0),
        };
        let (y, fy) = match self {
            TopLeft | Top | TopRight => (0.0, 0.0),
            Left | Middle | Right => (h / 2.0, 0.5),
            BottomLeft | Bottom | BottomRight => (h, 1.0),
        };
        (x, y, fx, fy)
    }
}

#[derive(Clone)]
pub struct Frame {
    items: Vec<(Shape, Style)>,
    next_colour: usize,
    /// Text at world positions — labels that travel with the drawing.
    labels: Vec<(Cx, String, u32, i32)>,
    /// Text at window positions — captions that do not.
    pins: Vec<(Anchor, f64, f64, String, u32, i32)>,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    pub fn new() -> Self {
        Frame { items: Vec::new(), next_colour: 0, labels: Vec::new(), pins: Vec::new() }
    }

    /// Add a shape, taking the next palette colour. Returns its style so it
    /// can be adjusted:
    ///
    /// ```text
    /// f.add(Shape::circle(z, 1.0)).color(0xE0A44A).width(3);
    /// ```
    pub fn add(&mut self, s: impl Into<Shape>) -> StyleRef<'_> {
        let s = s.into();
        let st = Style { colour: PALETTE[self.next_colour % PALETTE.len()], ..Style::default() };
        self.next_colour += 1;
        self.items.push((s, st));
        let k = self.items.len() - 1;
        StyleRef { f: self, k }
    }

    /// Add a shape **at a position** — `add(s.at(z))`, said the way you would
    /// say it.
    ///
    /// Every shape in `shapes` is built about its own origin, so this is the
    /// usual way to use one.
    ///
    /// ```text
    /// f.place(face::smiley(1.0), Cx::new(-3.0, 2.0)).color(0x6FCF97);
    /// ```
    /// `place` takes anything that knows how to put itself somewhere — a
    /// [`Shape`], which moves; or something like a wave, which is better off
    /// being *rebuilt* about the new point than shifted.
    ///
    /// The bound is on the method, so nothing needs importing at the call
    /// site. That was the whole trouble with the traits this replaced.
    pub fn place(&mut self, thing: impl Placeable, at: Cx) -> StyleRef<'_> {
        self.add(thing.placed(at))
    }

    /// Text at a world position, so it moves with whatever it labels.
    ///
    /// Use this for something that names a *part of the drawing* — it should
    /// travel with what it names, and go off the edge when that does.
    pub fn label(&mut self, at: Cx, text: impl Into<String>, colour: u32, scale: i32) {
        self.labels.push((at, text.into(), colour, scale));
    }

    /// Text pinned to the **window**, not the world.
    ///
    /// It does not move when the view is panned and does not resize when it is
    /// zoomed, because it is not part of the drawing — it is written on the
    /// glass in front of it. Titles, readouts and lists of controls belong
    /// here; a [`Frame::label`] naming a curve does not.
    ///
    /// `dx` and `dy` are pixels in from the anchor, `scale` multiplies the
    /// 5×7 font.
    ///
    /// ```no_run
    /// # use plotkit::{Frame, frame::Anchor};
    /// # let mut f = Frame::new();
    /// f.pin(Anchor::TopLeft, 14.0, 12.0, "walking right", 0x9AA7B4, 2);
    /// f.pin(Anchor::Bottom, 0.0, -18.0, "wheel zooms", 0x5A6774, 1);
    /// ```
    pub fn pin(&mut self, at: Anchor, dx: f64, dy: f64, text: impl Into<String>, colour: u32, scale: i32) {
        self.pins.push((at, dx, dy, text.into(), colour, scale.max(1)));
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
        self.pins.extend(other.pins);
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
        for (at, dx, dy, text, colour, scale) in &self.pins {
            // Straight to the canvas. The view is deliberately not consulted:
            // that is what makes a pin stay put.
            let (ax, ay, fx, fy) = at.spot(c.w as f64, c.h as f64);
            let tw = Canvas::text_w(text, *scale) as f64;
            let th = 7.0 * *scale as f64;
            c.text((ax + dx - tw * fx) as i32, (ay + dy - th * fy) as i32, text, *colour, *scale);
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

    /// ★ The whole point of a pin: it does not move when the view does. A
    /// caption that slid away as you panned, or grew as you zoomed, would be
    /// part of the drawing — and a title is not part of the drawing.
    #[test]
    fn a_pin_stays_put_however_the_view_moves() {
        let ink = |v: &View| {
            let mut c = Canvas::new(240, 120);
            c.clear(0);
            let mut f = Frame::new();
            f.pin(Anchor::TopLeft, 10.0, 8.0, "HELLO", 0xFFFFFF, 2);
            f.draw(&mut c, v);
            c.buf.clone()
        };
        let base = ink(&View::centred(240, 120, 40.0));
        assert!(base.iter().any(|&p| p != 0), "the pin should have drawn something");

        // Zoomed right in, zoomed right out, and panned a long way: identical.
        assert_eq!(base, ink(&View::centred(240, 120, 4000.0)));
        assert_eq!(base, ink(&View::centred(240, 120, 0.05)));
        assert_eq!(base, ink(&View::centred(240, 120, 40.0).with_origin(-900.0, 700.0)));
    }

    /// A label, by contrast, is part of the drawing and must move with it —
    /// otherwise nothing could name a curve.
    #[test]
    fn a_label_does_move_with_the_view() {
        let ink = |v: &View| {
            let mut c = Canvas::new(240, 120);
            c.clear(0);
            let mut f = Frame::new();
            f.label(Cx::new(1.0, 0.0), "HELLO", 0xFFFFFF, 2);
            f.draw(&mut c, v);
            c.buf.iter().position(|&p| p != 0)
        };
        assert_ne!(ink(&View::centred(240, 120, 40.0)), ink(&View::centred(240, 120, 40.0).with_origin(30.0, 60.0)));
    }

    /// The nine anchors land in nine different places, and each one keeps its
    /// text inside the window rather than half off the edge.
    #[test]
    fn every_anchor_lands_somewhere_different_and_stays_on_screen() {
        use Anchor::*;
        let v = View::centred(300, 200, 30.0);
        let mut seen = Vec::new();
        for a in [TopLeft, Top, TopRight, Left, Middle, Right, BottomLeft, Bottom, BottomRight] {
            let mut c = Canvas::new(300, 200);
            c.clear(0);
            let mut f = Frame::new();
            // Nudged in from the edge by the same amount the applications use.
            let (dx, dy) = match a {
                TopLeft | Left | BottomLeft => (8.0, 0.0),
                TopRight | Right | BottomRight => (-8.0, 0.0),
                _ => (0.0, 0.0),
            };
            let (dx, dy) = (dx, dy + if matches!(a, TopLeft | Top | TopRight) { 8.0 } else if matches!(a, BottomLeft | Bottom | BottomRight) { -14.0 } else { 0.0 });
            f.pin(a, dx, dy, "XX", 0xFFFFFF, 2);
            f.draw(&mut c, &v);
            let lit: Vec<usize> = c.buf.iter().enumerate().filter(|(_, &p)| p != 0).map(|(i, _)| i).collect();
            assert!(!lit.is_empty(), "{a:?} drew nothing — it fell off the window");
            seen.push(lit[0]);
        }
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 9, "two anchors landed in the same place");
    }

    /// Bigger text is bigger. The size is the caller's to choose, and nothing
    /// about the view changes it.
    #[test]
    fn a_pin_can_be_resized() {
        let ink = |scale: i32| {
            let mut c = Canvas::new(300, 120);
            c.clear(0);
            let mut f = Frame::new();
            f.pin(Anchor::TopLeft, 6.0, 6.0, "ABC", 0xFFFFFF, scale);
            f.draw(&mut c, &View::centred(300, 120, 30.0));
            c.buf.iter().filter(|&&p| p != 0).count()
        };
        assert!(ink(3) > ink(1) * 3, "scale 3 should be much more ink than scale 1");
    }

    #[test]
    fn merging_carries_pins_too() {
        let mut a = Frame::new();
        let mut b = Frame::new();
        b.pin(Anchor::Middle, 0.0, 0.0, "X", 0xFFFFFF, 2);
        a.merge(b);
        let mut c = Canvas::new(60, 60);
        c.clear(0);
        a.draw(&mut c, &View::centred(60, 60, 10.0));
        assert!(painted(&c) > 0, "the pin was lost in the merge");
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
