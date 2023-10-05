use core::fmt;

use clap::ValueEnum;
use font_kit::font::Font;
use hex_color::{HexColor, ParseHexColorError};
use raqote::{DrawOptions, DrawTarget, Point, Source, StrokeStyle, LineCap, LineJoin, SolidSource};
use serde::{Deserialize, Serialize, ser::SerializeStruct, de::{Visitor, SeqAccess, self, MapAccess}};
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

#[derive(Clone, PartialEq, Debug)]
pub struct StrokeStyleSerialize {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
    pub dash_array: Vec<f32>,
    pub dash_offset: f32,
}

impl Serialize for StrokeStyleSerialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut state = serializer.serialize_struct("Draw", 6)?;
        state.serialize_field("width", &self.width)?;
        match self.cap {
            LineCap::Round => state.serialize_field("cap", "round")?,
            LineCap::Square => state.serialize_field("cap", "square")?,
            LineCap::Butt => state.serialize_field("cap", "butt")?,
        }
        match self.join {
            LineJoin::Round => state.serialize_field("join", "round")?,
            LineJoin::Miter => state.serialize_field("join", "miter")?,
            LineJoin::Bevel => state.serialize_field("join", "bevel")?,
        }
        state.serialize_field("miter_limit", &self.miter_limit)?;
        state.serialize_field("dash_array", &self.dash_array)?;
        state.serialize_field("dash_offset", &self.dash_offset)?;
        state.end()
    }
}

fn read_cap(str: &str) -> Result<LineCap, String> {
    match str {
        "round" => Ok(LineCap::Round),
        "square" => Ok(LineCap::Square),
        "butt" => Ok(LineCap::Butt),
        x => return Err(format!("{x} is not a cap")),
    }
}
fn read_join(str: &str) -> Result<LineJoin, String> {
    // de::Error::invalid_value(unexp, exp)
    match str {
        "round" => Ok(LineJoin::Round),
        "milter" => Ok(LineJoin::Miter),
        "bevel" => Ok(LineJoin::Bevel),
        x => return Err(format!("{x} is not a join")),
    }
}

impl<'de> Deserialize<'de> for StrokeStyleSerialize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Width,
            Cap,
            Join,
            Miter_limit,
            Dash_array,
            Dash_offset,
        }

        struct StyleVisitor;

        impl<'de> Visitor<'de> for StyleVisitor {
            type Value = StrokeStyleSerialize;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Draw")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<StrokeStyleSerialize, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let width = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let cap_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let cap = read_cap(&cap_str).map_err(de::Error::custom)?;

                let join_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let join = read_join(&join_str).map_err(de::Error::custom)?;

                let miter_limit = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let dash_array = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let dash_offset = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &self))?;

                Ok(StrokeStyleSerialize {
                    width, cap, join, miter_limit, dash_array, dash_offset
                })
            }

            fn visit_map<V>(self, mut map: V) -> Result<StrokeStyleSerialize, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut width = None;
                let mut cap = None;
                let mut join = None;
                let mut miter_limit = None;
                let mut dash_array = None;
                let mut dash_offset = None;
                while let Some(f) = map.next_key()? {
                    match f {
                        Field::Width => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            width = Some(map.next_value()?);
                        }
                        Field::Cap => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            let cap_str: String = map.next_value()?;
                            cap = Some(read_cap(&cap_str).map_err(de::Error::custom)?);
                        }
                        Field::Join => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            let join_str: String = map.next_value()?;
                            join = Some(read_join(&join_str).map_err(de::Error::custom)?);
                        }
                        Field::Miter_limit => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            miter_limit = Some(map.next_value()?);
                        }
                        Field::Dash_array => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            dash_array = Some(map.next_value()?);
                        }
                        Field::Dash_offset => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            dash_offset = Some(map.next_value()?);
                        }
                    }
                }
                // let start = start.ok_or_else(|| de::Error::missing_field("key"))?;
                let width = width.ok_or_else(|| de::Error::missing_field("width"))?;
                let cap = cap.ok_or_else(|| de::Error::missing_field("cap"))?;
                let join = join.ok_or_else(|| de::Error::missing_field("join"))?;
                let miter_limit = miter_limit.ok_or_else(|| de::Error::missing_field("miter_limit"))?;
                let dash_array = dash_array.ok_or_else(|| de::Error::missing_field("dash_array"))?;
                let dash_offset = dash_offset.ok_or_else(|| de::Error::missing_field("dash_offset"))?;
                Ok(StrokeStyleSerialize {
                    width, cap, join, miter_limit, dash_array, dash_offset
                })
            }
        }
        const FIELDS: &'static [&'static str] = &["width", "cap", "join", "miter_limit", "dash_array", "dash_offset"];
        deserializer.deserialize_struct("StrokeStyle", FIELDS, StyleVisitor)
    }
}

impl From<&StrokeStyle> for StrokeStyleSerialize {
    fn from(value: &StrokeStyle) -> Self {
        StrokeStyleSerialize {
            width: value.width,
            cap: value.cap,
            join: value.join,
            miter_limit: value.miter_limit,
            dash_array: value.dash_array.clone(),
            dash_offset: value.dash_offset,
        }
    }
}

impl Into<StrokeStyle> for StrokeStyleSerialize {
    fn into(self) -> StrokeStyle {
        StrokeStyle { 
            width: self.width, 
            cap: self.cap,
            join: self.join,
            miter_limit: self.miter_limit,
            dash_array: self.dash_array,
            dash_offset: self.dash_offset, 
        }
    }
}


impl Serialize for Draw {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut state = serializer.serialize_struct("Draw", 5)?;
        state.serialize_field("start", &self.start)?;
        let style: StrokeStyleSerialize = (&self.style).into();
        state.serialize_field("style", &style)?;
        let colorstr = format!("#{:02x}{:02x}{:02x}{:02x}", 
            self.color.r, self.color.g, self.color.b, self.color.a);
        state.serialize_field("color", &colorstr)?;
        state.serialize_field("distance", &self.distance)?;
        state.serialize_field("action", &self.action)?;
        state.end()
    }
}

// TODO: remove dedup this
fn parse_solid(str: &str) -> Result<SolidSource, ParseHexColorError> {
    let hex = HexColor::parse(str)?;
    Ok(SolidSource {
        r: hex.r,
        g: hex.g,
        b: hex.b,
        a: hex.a,
    })
}

impl<'de> Deserialize<'de> for Draw {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Start,
            Style,
            Color,
            Distance,
            Action,
        }

        struct DrawVisitor;

        impl<'de> Visitor<'de> for DrawVisitor {
            type Value = Draw;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Draw")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Draw, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let start = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let style: StrokeStyleSerialize = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let color_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let distance= seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let action = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &self))?;
                let color = parse_solid(&color_str).map_err(de::Error::custom)?;

                Ok(Draw { start, style: style.into(), color, distance, action })
            }

            fn visit_map<V>(self, mut map: V) -> Result<Draw, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut start = None;
                let mut style: Option<StrokeStyleSerialize> = None;
                let mut color = None;
                let mut distance = None;
                let mut action = None;
                while let Some(f) = map.next_key()? {
                    match f {
                        Field::Start => {
                            if start.is_some() {
                                return Err(de::Error::duplicate_field("start"));
                            }
                            start = Some(map.next_value()?);
                        }
                        Field::Style => {
                            if start.is_some() {
                                return Err(de::Error::duplicate_field("style"));
                            }
                            style = Some(map.next_value()?);
                        }
                        Field::Color => {
                            if start.is_some() {
                                return Err(de::Error::duplicate_field("color"));
                            }
                            let color_str: String = map.next_value()?;
                            color = Some(parse_solid(&color_str).map_err(de::Error::custom)?);
                        }
                        Field::Distance => {
                            if start.is_some() {
                                return Err(de::Error::duplicate_field("distance"));
                            }
                            distance = Some(map.next_value()?);
                        }
                        Field::Action => {
                            if start.is_some() {
                                return Err(de::Error::duplicate_field("action"));
                            }
                            action = Some(map.next_value()?);
                        }
                    }
                }
                let start = start.ok_or_else(|| de::Error::missing_field("start"))?;
                let style = style.ok_or_else(|| de::Error::missing_field("style"))?;
                let color = color.ok_or_else(|| de::Error::missing_field("color"))?;
                let distance = distance.ok_or_else(|| de::Error::missing_field("distance"))?;
                let action = action.ok_or_else(|| de::Error::missing_field("action"))?;
                Ok(Draw { start, style: style.into(), color, distance, action })
            }
        }

        const FIELDS: &'static [&'static str] = &["start", "style", "color", "distance", "action"];
        deserializer.deserialize_struct("Draw", FIELDS, DrawVisitor)
    }
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawKind {
    Pen,
    Line,
    Rect,
    Circle,
}

#[derive(Serialize, Deserialize)]
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

// This function is buggy and should be fixed upstream
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
