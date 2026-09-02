//! Renders a sheet of marks so a human can look at them. Not an assertion —
//! the claims about widths are unit tests; this is for the eye.
use plotkit::{Canvas, Cx, Frame, View};
use shapes::Stroke;
use std::f64::consts::PI;

#[test]
fn sheet() {
    let mut f = Frame::new();
    let wave = |y: f64, amp: f64, n: usize| -> Vec<Cx> {
        (0..n)
            .map(|k| {
                let x = k as f64 / (n - 1) as f64 * 9.0 - 4.5;
                Cx::new(x, y + amp * (x * 1.1).sin())
            })
            .collect()
    };
    // round, blunt
    f.add(Stroke::new(wave(4.0, 0.7, 90)).round(0.35).shape()).color(0xE3E9EF).fill();
    // round, tapered
    f.add(Stroke::new(wave(2.5, 0.7, 90)).round(0.35).taper(0.25).shape()).color(0x6FCF97).fill();
    // quill: thin where the sine is steepest, because that is where the
    // samples are furthest apart
    f.add(Stroke::new(wave(1.0, 0.7, 90)).quill(0.45, 0.03, 0.13).taper(0.1).shape()).color(0xE0A44A).fill();
    // broad nib at 30 degrees
    f.add(Stroke::new(wave(-0.5, 0.7, 90)).broad(0.5, PI / 6.0).shape()).color(0x4FBCD4).fill();
    // a broad-nib circle: thick and thin twice round, like a pen drawing an O
    let ring: Vec<Cx> =
        (0..120).map(|k| Cx::new(-2.6, -3.2) + Cx::polar(1.5, k as f64 / 119.0 * 2.0 * PI)).collect();
    f.add(Stroke::new(ring).broad(0.42, PI / 4.0).shape()).color(0xE585AC).fill();
    // the same circle with a quill, drawn at a varying pace
    let ring2: Vec<Cx> = (0..120)
        .map(|k| {
            let s = k as f64 / 119.0;
            // 0.12, not more: past 1/2pi the angle goes BACKWARDS and the
            // stroke doubles back on itself. See `Stroke`'s note on that.
            let th = (s + 0.12 * (s * 2.0 * PI).sin()) * 2.0 * PI;
            Cx::new(2.2, -3.2) + Cx::polar(1.5, th)
        })
        .collect();
    f.add(Stroke::new(ring2).quill(0.5, 0.04, 0.14).shape()).color(0x9B7BD4).fill();

    let mut c = Canvas::new(900, 700);
    c.clear(0x0B1017);
    f.draw(&mut c, &View::centred(900, 700, 62.0));
    // The system temp directory, not a hard-coded /tmp: this has to work
    // wherever it is run, and it must not leave anything in the repository.
    let out = std::env::temp_dir().join("ink.png");
    c.write_png(out.to_str().expect("a path")).expect("wrote the sheet");
    println!("look at {}", out.display());
}
