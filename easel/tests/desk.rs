//! The studio as it actually looks: toolbar, drawing, and the row panel with
//! its sliders. For the eye; the claims are unit tests.

use easel::bar::Bar;
use easel::panel::Panel;
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

#[test]
fn desk() {
    let (w, h) = (1400, 820);
    let mut b = Board::new();

    // --- the written half -------------------------------------------------
    b.sheet.script.add("r = 3.2");
    b.sheet.script.add("n = 7");
    b.sheet.script.add("circle(0, r)");
    b.sheet.script.add("ngon(0, r, n)");
    b.sheet.script.add("color(0xE585AC)");
    b.sheet.script.add("param(r*exp(i*t) + 0.9*exp(7i*t + i*time), 0, tau)");
    b.sheet.script.rows.push(easel::Row::new("implicit(x*x + y*y, 4)").off());

    // --- and a hand-drawn one over it -------------------------------------
    b.nib = Nib::Quill { slow: 0.16, fast: 0.02, pace: 0.16 };
    b.taper = 0.15;
    b.colour = 0xE3E9EF;
    let hand: Vec<Cx> = (0..=90)
        .map(|k| {
            let s = k as f64 / 90.0;
            Cx::new(-5.4 + s * 3.0, 2.4 + 0.8 * (s * TAU).sin() + 0.02 * (s * 40.0).sin())
        })
        .collect();
    draw(&mut b, &hand);
    b.selected = vec![b.sheet.len() - 1];
    b.editing = Some(1);
    b.clock = 1.1;

    let mut c = Canvas::new(w, h);
    c.clear(0x0B1017);
    b.frame().draw(&mut c, &View::centred(w, h, 58.0).with_origin(w as f64 * 0.42, h as f64 * 0.5));
    for ring in b.selection() {
        let mut f = plotkit::Frame::new();
        f.add(ring).color(0x6FCF97).width(1);
        f.draw(&mut c, &View::centred(w, h, 58.0).with_origin(w as f64 * 0.42, h as f64 * 0.5));
    }

    let mut furniture = plotkit::Frame::new();
    Bar::new().paint(&mut furniture, &b, h as i32);
    Panel::new(w as i32, &b).paint(&mut furniture, &b, h as i32);
    furniture.draw(&mut c, &View::centred(w, h, 60.0));

    let out = std::env::temp_dir().join("desk.png");
    c.write_png(out.to_str().expect("a path")).expect("wrote it");
    println!("look at {}", out.display());
}
