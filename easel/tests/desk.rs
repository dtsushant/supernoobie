//! The studio as it looks: open, collapsed, and with the furniture away. The
//! drawing is a big circle on purpose — it should stop at the furniture's edge
//! rather than run under it.

use easel::bar::Bar;
use easel::tree::Tree;
use easel::Board;
use plotkit::{Canvas, Cx, Frame, View};
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

const BAR_DEEP: i32 = 100;

fn a_board() -> Board {
    let mut b = Board::new();
    b.nib = Nib::Quill { slow: 0.14, fast: 0.02, pace: 0.16 };
    b.taper = 0.12;
    b.colour = 0xE0A44A;
    let head = Cx::new(-0.4, 1.6);
    draw(&mut b, &ring(0.42, head));
    draw(&mut b, &line(Cx::new(-0.4, 1.15), Cx::new(-0.4, 0.0)));
    draw(&mut b, &line(Cx::new(-0.4, 0.9), Cx::new(-1.0, 0.4)));
    draw(&mut b, &line(Cx::new(-0.4, 0.9), Cx::new(0.2, 0.4)));
    b.selected = (0..4).collect();
    b.group();
    b.give(easel::Action::Walk(Cx::new(1.6, 0.0)), Some(2.0));

    b.sheet.script.add("r = 4.2");
    b.sheet.script.add("circle(0, r)");
    b.sheet.script.add("cheer = 0");
    b.sheet.script.add("smiley(3.4, 1.5, max(0, 1.0 - 1.4*(time - cheer)))");
    b.sheet.script.rows.push(easel::Row::new("ngon(0, r, 6)").off());
    b.choose_group(1);
    b
}

fn shot(b: &Board, full: bool, w: usize, h: usize) -> Canvas {
    let (left, top) = if full { (0, 0) } else { (Tree::width(b), BAR_DEEP) };
    let mut c = Canvas::new(w, h);
    c.clear(0x0B1017);

    let mut f = Frame::new();
    f.stage(left, top, w as i32 - left, h as i32 - top);
    for (shape, colour) in b.written().shapes {
        f.add(shape).color(colour).width(2);
    }
    for m in &b.sheet.marks {
        f.add(m.at(b.clock)).color(m.colour).fill();
    }
    if !full {
        Bar::new(w as i32).paint(&mut f, b, w as i32);
        Tree::new(b).paint(&mut f, b, h as i32);
    }
    f.draw(&mut c, &View::centred(w, h, 62.0));
    c
}

#[test]
fn desk() {
    let (w, h) = (1500, 560);
    let mut sheet = Canvas::new(w, h * 3);
    sheet.clear(0x05080C);

    let mut b = a_board();
    for (k, (full, label)) in [(false, "open"), (false, "collapsed"), (true, "just the drawing")].into_iter().enumerate()
    {
        if k == 1 {
            b.tree_shut = true;
        }
        let tile = shot(&b, full, w, h);
        for y in 0..h {
            for x in 0..w {
                sheet.px(x as i32, (k * h + y) as i32, tile.buf[y * w + x]);
            }
        }
        sheet.text(8, (k * h + h - 16) as i32, label, 0x46525E, 1);
    }

    let out = std::env::temp_dir().join("desk.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
}
