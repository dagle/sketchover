use raqote::DrawTarget;

use crate::tools::draw::draw::Draw;
use crate::tools::Tool;
use serde::Serialize;

#[derive(Serialize)]
pub struct Pen {
    draw: Draw,
    lines: Vec<(f64, f64)>,
}

impl Pen {
    pub fn new(_: (f64, f64), draw: Draw) -> Self {
        Pen {
            draw,
            lines: Vec::new(),
        }
    }
}

// we need a way to create a new pen from an identifier
impl Tool for Pen {
    fn update(&mut self, motion: (f64, f64)) {
        self.lines.push(motion);
    }

    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        let mut pb = raqote::PathBuilder::new();

        if self.lines.is_empty() {
            return;
        }

        pb.move_to(self.lines[0].0 as f32, self.lines[0].1 as f32);
        for stroke in self.lines.iter() {
            pb.line_to(stroke.0 as f32, stroke.1 as f32);
            pb.move_to(stroke.0 as f32, stroke.1 as f32);
        }
        pb.close();

        dt.stroke(
            &pb.finish(),
            &raqote::Source::Solid(self.draw.color),
            &self.draw.style,
            &raqote::DrawOptions::new(),
        );
    }
}
