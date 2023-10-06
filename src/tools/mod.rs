use raqote::DrawTarget;
use serde::{Serialize, Deserialize};

pub trait Tool {
    /// Name of the tool
    fn identify() -> &'static str; 
    /// Create a new 
    fn draw_start(x: f64, y: f64) -> Self;
    fn add_motion(&mut self, x: f64, y: f64);
    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>);
    fn size(&self) -> Option<(f64, f64)>;
}

#[derive(Serialize, Deserialize)]
pub struct Pen {
    parts: Vec<(f64, f64)>
}

impl Tool for Pen {
    fn identify() -> &'static str {
        "Pen"
    }

    fn draw_start(x: f64, y: f64) -> Self {
        Pen {
            parts: vec![(x,y)]
        }
    }

    fn add_motion(&mut self, x: f64, y: f64) {
        self.parts.push((x,y));

    }

    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        // pb.move_to(self.start.0 as f32, self.start.1 as f32);
        for stroke in pen {
            pb.line_to(stroke.0 as f32, stroke.1 as f32);
            pb.move_to(stroke.0 as f32, stroke.1 as f32);
        }
        pb.close();

        todo!()
    }

    fn size(&self) -> Option<(f64, f64)> {
        todo!()
    }
}
