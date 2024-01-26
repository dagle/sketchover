use raqote::DrawTarget;
use smithay_client_toolkit::seat::keyboard::Modifiers;

use crate::tools::draw::draw::Draw;
use crate::tools::Tool;

pub struct Line {
    draw: Draw,
    start: (f64, f64),
    stop: (f64, f64),
    real_stop: Option<(f64, f64)>,
    straight: bool,
}

impl Line {
    // pub fn new(pos: (f64, f64), draw: Draw) -> Self {
    pub fn new(pos: (f64, f64), draw: Draw) -> Self {
        Line {
            draw,
            start: pos,
            stop: (pos.0 + 20.0, pos.1 + 20.0),
            real_stop: None,
            straight: false,
        }
    }
}

impl Tool for Line {
    fn update(&mut self, motion: (f64, f64)) {
        if self.straight {
            self.real_stop = Some(motion);
            let x_length = f64::abs(motion.0 - self.start.0);
            let y_length = f64::abs(motion.1 - self.start.1);
            if x_length > y_length {
                self.stop = (motion.0, self.start.1);
            } else {
                self.stop = (self.start.0, motion.1);
            }
        } else {
            self.stop = motion;
        }
    }

    // Fix this later, modifier shouldn't work like this
    // it should be a setter of some sort
    fn modifier(&mut self, modifier: &Modifiers) {
        // don't hardcode this
        if modifier.shift {
            self.straight = true;
            self.update(self.stop);
        } else {
            self.straight = true;
            if let Some(stop) = self.real_stop {
                self.update(stop);
            }
        }
    }

    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        let mut pb = raqote::PathBuilder::new();
        pb.move_to(self.start.0 as f32, self.start.1 as f32);
        pb.line_to(self.stop.0 as f32, self.stop.1 as f32);

        pb.close();

        dt.stroke(
            &pb.finish(),
            &raqote::Source::Solid(self.draw.color),
            &self.draw.style,
            &raqote::DrawOptions::new(),
        );
    }
}
