use raqote::DrawTarget;

use crate::tools::draw::draw::Draw;
use crate::tools::Tool;

pub struct Rect {
    draw: Draw,
    start: (f64, f64),
    stop: (f64, f64),
    square: bool,
    fill: bool,
}

impl Tool for Rect {
    fn update(&mut self, motion: (f64, f64)) {
        self.stop = motion;
    }

    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        let mut pb = raqote::PathBuilder::new();

        pb.move_to(self.start.0 as f32, self.start.1 as f32);
        pb.rect(
            self.start.0 as f32,
            self.start.1 as f32,
            self.stop.0 as f32,
            self.stop.1 as f32,
        );

        dt.stroke(
            &pb.finish(),
            &raqote::Source::Solid(self.draw.color),
            &self.draw.style,
            &raqote::DrawOptions::new(),
        );
    }
}
