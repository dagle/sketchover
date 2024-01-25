// Draw is a datastucture most (or all) draw tools want to contain.
// Where draw defines serializeable/deserializeable traits so we
// can easily save data to disk

use core::fmt;

use raqote::{LineCap, LineJoin, SolidSource, StrokeStyle};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};

#[derive(Clone, PartialEq, Debug)]
pub struct Draw {
    pub style: StrokeStyle,
    pub color: SolidSource,
    // pub distance: bool,
}

impl Default for Draw {
    fn default() -> Self {
        let style = StrokeStyle::default();
        let solid = SolidSource {
            r: 255,
            b: 0,
            g: 0,
            a: 0,
        };
        Draw {
            style,
            color: solid,
        }
    }
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

pub fn diff(pos: (f64, f64), motion: (f64, f64)) -> (f64, f64) {
    (motion.0 - pos.0, motion.1 - pos.1)
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

#[cfg(test)]
mod tests {
    use raqote::{LineCap, LineJoin, SolidSource, StrokeStyle};

    use super::{Draw, StrokeStyleSerialize};

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
