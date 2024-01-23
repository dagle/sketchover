use raqote::DrawTarget;

use crate::tools::draw::draw;
use crate::tools::draw::draw::Draw;
use crate::tools::Tool;

pub struct Circle {
    draw: Draw,
    start: (f64, f64),
    stop: (f64, f64),
    // elipse: bool,
}

impl Tool for Circle {
    fn update(&mut self, motion: (f64, f64)) {
        self.stop = draw::diff(self.start, motion);
    }

    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        let mut pb = raqote::PathBuilder::new();
        let r = f64::sqrt(
            f64::powi(self.start.0 - self.stop.0, 2) + f64::powi(self.start.1 - self.stop.1, 2),
        );
        let start_x = (self.start.0 + self.stop.0) / 2.0;
        let start_y = (self.start.1 + self.stop.1) / 2.0;
        pb.arc(
            start_x as f32,
            start_y as f32,
            (r / 2.0) as f32,
            0.,
            2. * std::f32::consts::PI,
        );

        dt.stroke(
            &pb.finish(),
            &raqote::Source::Solid(self.draw.color),
            &self.draw.style,
            &raqote::DrawOptions::new(),
        );
    }
}
