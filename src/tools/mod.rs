use raqote::Point;
use serde::{Deserialize, Serialize};

use clap::ValueEnum;
use smithay_client_toolkit::seat::keyboard::Modifiers;

#[derive(Serialize, Deserialize)]
pub struct Pen {
    parts: Vec<(f64, f64)>,
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawKind {
    Pen,
    Line,
    Rect,
    Circle,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Tool {
    Pen(Vec<(f64, f64)>),
    Line { start: (f64, f64), stop: (f64, f64) },
    Rect { start: (f64, f64), stop: (f64, f64) },
    Circle { start: (f64, f64), stop: (f64, f64) },
}

pub trait Tool2 {
    /// When the mouse is moved, the currently active (if there is one) tool
    /// will be updated,
    fn update(&mut self, motion: (f64, f64));
    /// convert the tool to a set of paths
    fn draw(&self) -> raqote::PathBuilder;

    /// A tool sould be able to have a set of modifiers width
    /// it. Atm these are hardcoded against keys but in the future
    /// these should be mapped against a config value.
    fn modifier(&mut self, modifier: &Modifiers) {
        return;
    }
    /// For drawing the size of the figure
    /// The function returns the size (width, height) and a position
    /// to put the text
    fn draw_size(&self) -> Option<((f64, f64), Point)> {
        None
    }
}

pub struct Pen2(Vec<(f64, f64)>);

// we need a way to create a new pen from an identifier
impl Tool2 for Pen2 {
    fn update(&mut self, motion: (f64, f64)) {
        self.0.push(motion);
    }

    fn draw(&self) -> raqote::PathBuilder {
        let mut pb = raqote::PathBuilder::new();

        pb.move_to(self.0[0].0 as f32, self.0[0].1 as f32);
        for stroke in self.0.iter() {
            pb.line_to(stroke.0 as f32, stroke.1 as f32);
            pb.move_to(stroke.0 as f32, stroke.1 as f32);
        }
        pb.close();
        pb
    }
}

pub struct Line2 {
    start: (f64, f64),
    stop: (f64, f64),
    real_stop: Option<(f64, f64)>,
    straight: bool,
}

impl Tool2 for Line2 {
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

    fn draw(&self) -> raqote::PathBuilder {
        let mut pb = raqote::PathBuilder::new();
        pb.move_to(self.start.0 as f32, self.start.1 as f32);
        pb.line_to(self.stop.0 as f32, self.stop.1 as f32);
        pb
    }
}

pub struct Rect2 {
    start: (f64, f64),
    stop: (f64, f64),
    square: bool,
    fill: bool,
}

impl Tool2 for Rect2 {
    fn update(&mut self, motion: (f64, f64)) {
        self.stop = motion;
    }

    fn draw(&self) -> raqote::PathBuilder {
        let mut pb = raqote::PathBuilder::new();

        pb.move_to(self.start.0 as f32, self.start.1 as f32);
        pb.rect(
            self.start.0 as f32,
            self.start.1 as f32,
            self.stop.0 as f32,
            self.stop.1 as f32,
        );
        pb
    }
}

pub struct Circle2 {
    start: (f64, f64),
    stop: (f64, f64),
    // elipse: bool,
}

impl Tool2 for Circle2 {
    fn update(&mut self, motion: (f64, f64)) {
        self.stop = motion;
    }

    fn draw(&self) -> raqote::PathBuilder {
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
        pb
    }
}

// Line { start: (f64, f64), stop: (f64, f64) },
// Rect { start: (f64, f64), stop: (f64, f64) },
// Circle { start: (f64, f64), stop: (f64, f64) },

impl Tool {
    pub fn new(dk: DrawKind, pos: (f64, f64)) -> Self {
        match dk {
            DrawKind::Pen => Tool::Pen(vec![pos]),
            DrawKind::Line => Tool::Line {
                start: pos,
                stop: pos,
            },
            DrawKind::Rect => Tool::Rect {
                start: pos,
                stop: (5.0, 5.0),
            },
            DrawKind::Circle => Tool::Circle {
                start: pos,
                stop: pos,
            },
        }
    }

    pub fn add_motion(&mut self, motion: Option<(f64, f64)>, modifier: &Modifiers) {
        match self {
            Tool::Pen(ref mut pen) => {
                if let Some(motion) = motion {
                    pen.push(motion);
                }
            }
            Tool::Line { start, stop } => {
                let motion = motion.unwrap_or(*stop);
                if modifier.shift {
                    let x_length = f64::abs(motion.0 - start.0);
                    let y_length = f64::abs(motion.1 - start.1);
                    if x_length > y_length {
                        *stop = (motion.0, start.1);
                    } else {
                        *stop = (start.0, motion.1);
                    }
                } else {
                    *stop = motion
                }
            }
            Tool::Rect { start, stop } => {
                let (x_dist, y_dist) = match motion {
                    Some(motion) => (motion.0 - start.0, motion.1 - start.1),
                    None => *stop,
                };
                if modifier.shift {
                    let x_length = f64::abs(x_dist);
                    let y_length = f64::abs(y_dist);
                    let x_sign = f64::signum(x_dist);
                    let y_sign = f64::signum(y_dist);
                    if x_length > y_length {
                        *stop = (x_dist, x_length * y_sign)
                    } else {
                        *stop = (y_length * x_sign, y_dist);
                    }
                } else {
                    *stop = (x_dist, y_dist);
                }
            }
            Tool::Circle { start: _, stop } => {
                if let Some(motion) = motion {
                    *stop = motion;
                }
            }
        }
    }

    pub fn draw(&self) -> raqote::PathBuilder {
        let mut pb = raqote::PathBuilder::new();

        match self {
            Tool::Pen(ref pen) => {
                pb.move_to(pen[0].0 as f32, pen[0].1 as f32);
                for stroke in pen {
                    pb.line_to(stroke.0 as f32, stroke.1 as f32);
                    pb.move_to(stroke.0 as f32, stroke.1 as f32);
                }
                pb.close();
            }
            Tool::Line { start, stop } => {
                pb.move_to(start.0 as f32, start.1 as f32);
                pb.line_to(stop.0 as f32, stop.1 as f32);
            }
            Tool::Rect { start, stop } => {
                pb.move_to(start.0 as f32, start.1 as f32);
                pb.rect(start.0 as f32, start.1 as f32, stop.0 as f32, stop.1 as f32);
            }
            Tool::Circle { start, stop } => {
                let r = f64::sqrt(f64::powi(start.0 - stop.0, 2) + f64::powi(start.1 - stop.1, 2));
                let start_x = (start.0 + stop.0) / 2.0;
                let start_y = (start.1 + stop.1) / 2.0;
                pb.arc(
                    start_x as f32,
                    start_y as f32,
                    (r / 2.0) as f32,
                    0.,
                    2. * std::f32::consts::PI,
                );
            }
        }
        pb.close();
        pb
    }
    pub fn draw_size(&self) -> Option<((f64, f64), Point)> {
        match self {
            Tool::Pen(_) => None,
            Tool::Line { start: _, stop } => Some((
                *stop,
                Point::new((stop.0 - 15.0) as f32, (stop.1 - 15.0) as f32),
            )),
            Tool::Rect { start, stop } => Some((
                *stop,
                Point::new(
                    (start.0 + stop.0 + 15.0) as f32,
                    (start.1 + stop.1 + 15.0) as f32,
                ),
            )),
            Tool::Circle { start, stop } => Some((
                *stop,
                Point::new(
                    (start.0 + stop.0 + 15.0) as f32,
                    (start.1 + stop.1 + 15.0) as f32,
                ),
            )),
        }
    }
}
