//! The studio as it actually looks: the tree down the left with a figure
//! chosen and its options open, the toolbar across the top, the drawing
//! between. For the eye; the claims are unit tests.

use easel::bar::Bar;
use easel::tree::Tree;
use easel::Board;
use plotkit::{Canvas, Cx, View};
use shapes::Nib;
use std::f64::consts::TAU;

fn draw(b: &mut Board, path: &[Cx]) {
    for z in path {
        b.pointer(*z, true);
    }
    b.pointer(*path.last().expect("a path"), false);
}

fn ring(r: f64, at: Cx) -> Vec<Cx> {
    (0..=60).map(|k| at + Cx::polar(r, k as f64 / 60.0 * TAU)).collect()
}

fn line(from: Cx, to: Cx) -> Vec<Cx> {
    (0..=24).map(|k| from + (to - from).scale(k as f64 / 24.0)).collect()
}

#[test]
fn desk() {
    let (w, h) = (1500, 880);
    let mut b = Board::new();
    b.nib = Nib::Quill { slow: 0.14, fast: 0.02, pace: 0.16 };
    b.taper = 0.12;

    // A figure, grouped.
    b.colour = 0xE0A44A;
    let head = Cx::new(-1.0, 1.6);
    draw(&mut b, &ring(0.42, head));
    draw(&mut b, &line(Cx::new(-1.0, 1.15), Cx::new(-1.0, 0.0)));
    draw(&mut b, &line(Cx::new(-1.0, 0.9), Cx::new(-1.6, 0.4)));
    draw(&mut b, &line(Cx::new(-1.0, 0.9), Cx::new(-0.4, 0.4)));
    b.selected = (0..4).collect();
    b.group();

    // A second figure, which will be folded away.
    b.colour = 0x4FBCD4;
    draw(&mut b, &ring(0.5, Cx::new(2.4, 0.6)));
    draw(&mut b, &ring(0.3, Cx::new(2.4, -0.4)));
    b.selected = vec![4, 5];
    b.group();
    b.fold(b.sheet.marks[4].group);

    // And a loose stroke.
    b.colour = 0xE585AC;
    draw(&mut b, &line(Cx::new(-3.4, -1.6), Cx::new(3.4, -1.6)));

    b.sheet.script.add("r = 2.4");
    b.sheet.script.add("circle(0, r)");
    b.sheet.script.rows.push(easel::Row::new("ngon(0, r, 6)").off());

    // The first figure chosen, so its options are open.
    b.choose_group(1);
    b.give(easel::Action::Walk(Cx::new(1.6, 0.0)), Some(2.0));

    let mut c = Canvas::new(w, h);
    c.clear(0x0B1017);
    let view = View::centred(w, h, 74.0).with_origin(w as f64 * 0.56, h as f64 * 0.55);
    b.frame().draw(&mut c, &view);
    let mut sel = plotkit::Frame::new();
    for ring in b.selection() {
        sel.add(ring).color(0x6FCF97).width(1);
    }
    sel.draw(&mut c, &view);

    let mut furniture = plotkit::Frame::new();
    Bar::new(w as i32).paint(&mut furniture, &b, w as i32);
    Tree::new(&b).paint(&mut furniture, &b, h as i32);
    furniture.draw(&mut c, &View::centred(w, h, 60.0));

    let out = std::env::temp_dir().join("desk.png");
    c.write_png(out.to_str().expect("a path")).expect("wrote it");
}
