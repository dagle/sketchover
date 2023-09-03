use clap::ValueEnum;
use font_kit::font::Font;
use raqote::{DrawOptions, DrawTarget, Point, Source, StrokeStyle};
use serde::{Deserialize, Serialize};
use smithay_client_toolkit::seat::keyboard::Modifiers;

mod fk {
    pub use font_kit::canvas::{Canvas, Format, RasterizationOptions};
    pub use font_kit::font::Font;
    pub use font_kit::hinting::HintingOptions;
    pub use pathfinder_geometry::vector::{vec2f, vec2i};
}

pub struct Draw {
    pub start: (f64, f64),
    pub style: StrokeStyle,
    pub color: raqote::SolidSource,
    pub distance: bool,
    pub action: DrawAction,
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawKind {
    Pen,
    Line,
    Rect,
    Circle,
}

pub enum DrawAction {
    Pen(Vec<(f64, f64)>),
    Line(f64, f64),
    Rect(f64, f64),
    Circle(f64, f64),
}

impl Draw {
    pub fn add_motion(&mut self, motion: Option<(f64, f64)>, modifier: &Modifiers) {
        match self.action {
            DrawAction::Pen(ref mut pen) => {
                if let Some(motion) = motion {
                    pen.push(motion);
                }
            }
            DrawAction::Line(x, y) => {
                let motion = motion.unwrap_or((x, y));
                if modifier.shift {
                    let x_length = f64::abs(motion.0 - self.start.0);
                    let y_length = f64::abs(motion.1 - self.start.1);
                    if x_length > y_length {
                        self.action = DrawAction::Line(motion.0, self.start.1);
                    } else {
                        self.action = DrawAction::Line(self.start.0, motion.1);
                    }
                } else {
                    self.action = DrawAction::Line(motion.0, motion.1);
                }
            }
            DrawAction::Rect(x, y) => {
                let (x_dist, y_dist) = match motion {
                    Some(motion) => (motion.0 - self.start.0, motion.1 - self.start.1),
                    None => (x, y),
                };
                if modifier.shift {
                    let x_length = f64::abs(x_dist);
                    let y_length = f64::abs(y_dist);
                    let x_sign = f64::signum(x_dist);
                    let y_sign = f64::signum(y_dist);
                    if x_length > y_length {
                        self.action = DrawAction::Rect(x_dist, x_length * y_sign);
                    } else {
                        self.action = DrawAction::Rect(y_length * x_sign, y_dist);
                    }
                } else {
                    self.action = DrawAction::Rect(x_dist, y_dist);
                }
            }
            DrawAction::Circle(_, _) => {
                if let Some((x, y)) = motion {
                    self.action = DrawAction::Circle(x, y);
                }
            }
        }
    }
    pub fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        let mut pb = raqote::PathBuilder::new();
        pb.move_to(self.start.0 as f32, self.start.1 as f32);
        match self.action {
            DrawAction::Pen(ref pen) => {
                for stroke in pen {
                    pb.line_to(stroke.0 as f32, stroke.1 as f32);
                    pb.move_to(stroke.0 as f32, stroke.1 as f32);
                }
                pb.close();
            }
            DrawAction::Line(x, y) => {
                pb.line_to(x as f32, y as f32);
            }
            DrawAction::Rect(w, h) => {
                pb.rect(self.start.0 as f32, self.start.1 as f32, w as f32, h as f32);
            }
            DrawAction::Circle(x, y) => {
                let r = f64::sqrt(f64::powi(self.start.0 - x, 2) + f64::powi(self.start.1 - y, 2));
                let start_x = (self.start.0 + x) / 2.0;
                let start_y = (self.start.1 + y) / 2.0;
                pb.arc(
                    start_x as f32,
                    start_y as f32,
                    (r / 2.0) as f32,
                    0.,
                    2. * std::f32::consts::PI,
                );
            }
        }
        dt.stroke(
            &pb.finish(),
            &raqote::Source::Solid(self.color),
            &self.style,
            &raqote::DrawOptions::new(),
        );
    }
    pub fn draw_size(
        &self,
        dt: &mut DrawTarget<&mut [u32]>,
        font: &Font,
        point_size: f32,
        src: &Source,
        options: &DrawOptions,
    ) {
        let (x, y) = match self.action {
            DrawAction::Pen(_) => return,
            DrawAction::Line(x, y) => (x, y),
            DrawAction::Rect(x, y) => (x, y),
            DrawAction::Circle(x, y) => (x, y),
        };
        let point = match self.action {
            DrawAction::Pen(_) => return,
            DrawAction::Line(x, y) => raqote::Point::new((x - 15.) as f32, (y - 15.) as f32),
            DrawAction::Rect(x, y) => raqote::Point::new(
                (self.start.0 + x + 15.) as f32,
                (self.start.1 + y + 15.) as f32,
            ),
            // TODO:
            DrawAction::Circle(x, y) => raqote::Point::new(
                (self.start.0 + x + 15.) as f32,
                (self.start.1 + y + 15.) as f32,
            ),
        };
        draw_text(
            dt,
            font,
            point_size,
            &format!(
                "({:.2}, {:.2})",
                f64::abs(self.start.0 - x),
                f64::abs(self.start.1 - y)
            ),
            // self.constraint_label(),
            point,
            src,
            options,
        );
    }
}

pub fn draw_text(
    dt: &mut DrawTarget<&mut [u32]>,
    font: &Font,
    point_size: f32,
    text: &str,
    start: Point,
    src: &Source,
    options: &DrawOptions,
) {
    let mut start = fk::vec2f(start.x, start.y);
    let mut ids = Vec::new();
    let mut positions = Vec::new();
    for c in text.chars() {
        let id = font.glyph_for_char(c).unwrap();
        ids.push(id);
        positions.push(Point::new(start.x(), start.y()));
        start += font.advance(id).unwrap() * point_size / 12. / 96.;
    }
    dt.draw_glyphs(font, point_size, &ids, &positions, src, options);
}
