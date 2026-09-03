//! Draws the board with tokens on it, so a human can see that the geometry is
//! a Ludo board and not merely self-consistent.
use plotkit::{Canvas, Cx, Frame, View};
use shapes::ludo;

#[test]
fn board() {
    let mut f = Frame::new();
    for (shape, colour) in ludo::board() {
        f.add(shape).color(colour).width(1);
    }

    // One token per seat, walked to a different distance, so the four paths
    // can be checked against each other by eye.
    for seat in 0..4 {
        let step = [0usize, 17, 46, 54][seat];
        let at = ludo::place(seat, step);
        f.add(plotkit::Shape::circle(at, 0.34)).color(ludo::SEATS[seat]).fill();
    }
    // And one still waiting, per seat.
    for seat in 0..4 {
        f.add(plotkit::Shape::circle(ludo::waiting(seat, 0), 0.28)).color(ludo::SEATS[seat]).fill();
    }

    // The whole of seat 0's journey, faintly, so a wrong turn is obvious.
    for step in 0..=ludo::FINISH {
        f.add(plotkit::Shape::circle(ludo::place(0, step), 0.09)).color(0x8FBF6A).fill();
    }

    let mut c = Canvas::new(760, 760);
    c.clear(0x0B1017);
    f.draw(&mut c, &View::centred(760, 760, 46.0));
    let out = std::env::temp_dir().join("ludo.png");
    c.write_png(out.to_str().expect("a path")).expect("wrote it");
    let _ = Cx::ZERO;
}
