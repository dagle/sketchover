use core::fmt;

use font_kit::font::Font;
use hex_color::{HexColor, ParseHexColorError};
use raqote::{DrawOptions, DrawTarget, LineCap, LineJoin, Point, SolidSource, Source, StrokeStyle};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use smithay_client_toolkit::seat::keyboard::Modifiers;

use crate::tools::Tool;

mod fk {
    pub use font_kit::canvas::{Canvas, Format, RasterizationOptions};
    pub use font_kit::font::Font;
    pub use font_kit::hinting::HintingOptions;
    pub use pathfinder_geometry::vector::{vec2f, vec2i};
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
        S: serde::Serializer,
    {
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
        x => Err(format!("{x} is not a cap")),
    }
}
fn read_join(str: &str) -> Result<LineJoin, String> {
    match str {
        "round" => Ok(LineJoin::Round),
        "miter" => Ok(LineJoin::Miter),
        "bevel" => Ok(LineJoin::Bevel),
        x => Err(format!("{x} is not a join")),
    }
}

impl<'de> Deserialize<'de> for StrokeStyleSerialize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        #[allow(non_camel_case_types)]
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
                    width,
                    cap,
                    join,
                    miter_limit,
                    dash_array,
                    dash_offset,
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
                                return Err(de::Error::duplicate_field("width"));
                            }
                            width = Some(map.next_value()?);
                        }
                        Field::Cap => {
                            if cap.is_some() {
                                return Err(de::Error::duplicate_field("cap"));
                            }
                            let cap_str: String = map.next_value()?;
                            cap = Some(read_cap(&cap_str).map_err(de::Error::custom)?);
                        }
                        Field::Join => {
                            if join.is_some() {
                                return Err(de::Error::duplicate_field("join"));
                            }
                            let join_str: String = map.next_value()?;
                            join = Some(read_join(&join_str).map_err(de::Error::custom)?);
                        }
                        Field::Miter_limit => {
                            if miter_limit.is_some() {
                                return Err(de::Error::duplicate_field("miter_limit"));
                            }
                            miter_limit = Some(map.next_value()?);
                        }
                        Field::Dash_array => {
                            if dash_array.is_some() {
                                return Err(de::Error::duplicate_field("dash_array"));
                            }
                            dash_array = Some(map.next_value()?);
                        }
                        Field::Dash_offset => {
                            if dash_offset.is_some() {
                                return Err(de::Error::duplicate_field("dash_offset"));
                            }
                            dash_offset = Some(map.next_value()?);
                        }
                    }
                }
                // let start = start.ok_or_else(|| de::Error::missing_field("key"))?;
                let width = width.ok_or_else(|| de::Error::missing_field("width"))?;
                let cap = cap.ok_or_else(|| de::Error::missing_field("cap"))?;
                let join = join.ok_or_else(|| de::Error::missing_field("join"))?;
                let miter_limit =
                    miter_limit.ok_or_else(|| de::Error::missing_field("miter_limit"))?;
                let dash_array =
                    dash_array.ok_or_else(|| de::Error::missing_field("dash_array"))?;
                let dash_offset =
                    dash_offset.ok_or_else(|| de::Error::missing_field("dash_offset"))?;
                Ok(StrokeStyleSerialize {
                    width,
                    cap,
                    join,
                    miter_limit,
                    dash_array,
                    dash_offset,
                })
            }
        }
        const FIELDS: &[&str] = &[
            "width",
            "cap",
            "join",
            "miter_limit",
            "dash_array",
            "dash_offset",
        ];
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

impl From<StrokeStyleSerialize> for StrokeStyle {
    fn from(val: StrokeStyleSerialize) -> Self {
        StrokeStyle {
            width: val.width,
            cap: val.cap,
            join: val.join,
            miter_limit: val.miter_limit,
            dash_array: val.dash_array,
            dash_offset: val.dash_offset,
        }
    }
}

impl Serialize for Draw {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Draw", 5)?;
        // state.serialize_field("start", &self.start)?;
        let style: StrokeStyleSerialize = (&self.style).into();
        state.serialize_field("style", &style)?;
        let colorstr = format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            self.color.r, self.color.g, self.color.b, self.color.a
        );
        state.serialize_field("color", &colorstr)?;
        state.serialize_field("distance", &self.distance)?;
        state.serialize_field("action", &self.tool)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Draw {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
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
                let distance = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let action = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &self))?;
                let color = parse_solid(&color_str).map_err(de::Error::custom)?;

                Ok(Draw {
                    // start,
                    style: style.into(),
                    color,
                    distance,
                    tool: action,
                })
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
                            if style.is_some() {
                                return Err(de::Error::duplicate_field("style"));
                            }
                            style = Some(map.next_value()?);
                        }
                        Field::Color => {
                            if color.is_some() {
                                return Err(de::Error::duplicate_field("color"));
                            }
                            let color_str: String = map.next_value()?;
                            color = Some(parse_solid(&color_str).map_err(de::Error::custom)?);
                        }
                        Field::Distance => {
                            if distance.is_some() {
                                return Err(de::Error::duplicate_field("distance"));
                            }
                            distance = Some(map.next_value()?);
                        }
                        Field::Action => {
                            if action.is_some() {
                                return Err(de::Error::duplicate_field("action"));
                            }
                            action = Some(map.next_value()?);
                        }
                    }
                }
                // let start = start.ok_or_else(|| de::Error::missing_field("start"))?;
                let style = style.ok_or_else(|| de::Error::missing_field("style"))?;
                let color = color.ok_or_else(|| de::Error::missing_field("color"))?;
                let distance = distance.ok_or_else(|| de::Error::missing_field("distance"))?;
                let action = action.ok_or_else(|| de::Error::missing_field("action"))?;
                Ok(Draw {
                    // start,
                    style: style.into(),
                    color,
                    distance,
                    tool: action,
                })
            }
        }

        // const FIELDS: &[&str] = &["start", "style", "color", "distance", "action"];
        const FIELDS: &[&str] = &["style", "color", "distance", "action"];
        deserializer.deserialize_struct("Draw", FIELDS, DrawVisitor)
    }
}

impl Draw {
    pub fn add_motion(&mut self, motion: Option<(f64, f64)>, modifier: &Modifiers) {
        self.tool.add_motion(motion, modifier)
    }
    pub fn draw(&self, dt: &mut DrawTarget<&mut [u32]>) {
        // self.tool.draw(dt);
        let mut pb = raqote::PathBuilder::new();

        match self.tool {
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
        // if let Some(((x, y), point)) = self.tool.draw_size() {
        //     draw_text(
        //         dt,
        //         font,
        //         point_size,
        //         &format!(
        //             "({:.2}, {:.2})",
        //             f64::abs(self.start.0 - x),
        //             f64::abs(self.start.1 - y)
        //         ),
        //         point,
        //         src,
        //         options,
        //     );
        // }
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

#[cfg(test)]
mod tests {
    use raqote::{LineCap, LineJoin, SolidSource, StrokeStyle};

    use super::{Draw, StrokeStyleSerialize, Tool};

    #[test]
    fn draw_serialize_deserialize() {
        let style = StrokeStyle {
            width: 3.0,
            cap: LineCap::Round,
            join: LineJoin::Round,
            miter_limit: 0.0,
            dash_array: Vec::new(),
            dash_offset: 0.0,
        };
        let pre = Draw {
            // start: (0.0, 0.0),
            style,
            color: SolidSource {
                r: 200,
                g: 140,
                b: 80,
                a: 0,
            },
            distance: false,
            tool: Tool::Pen(vec![(23.0, 48.0), (48.0, 93.9)]),
        };
        let j = serde_json::to_string(&pre).unwrap();
        let post: Draw = serde_json::from_str(&j).unwrap();
        assert!(pre == post)
    }

    #[test]
    fn stroke_serialize_deserialize() {
        let pre = StrokeStyleSerialize {
            width: 3.0,
            cap: LineCap::Round,
            join: LineJoin::Miter,
            miter_limit: 0.0,
            dash_array: Vec::new(),
            dash_offset: 0.0,
        };
        let j = serde_json::to_string(&pre).unwrap();
        let post: StrokeStyleSerialize = serde_json::from_str(&j).unwrap();
        assert!(pre == post)
    }
}
