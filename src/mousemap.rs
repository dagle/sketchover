use core::fmt;
use std::hash::Hash;

use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize,
};
use smithay_client_toolkit::seat::keyboard::Modifiers;

#[derive(Hash, Serialize, Deserialize, Debug)]
pub enum MouseEvent {
    BtnLeft,
    BtnRight,
    BtnMiddle,
    BtnSide,
    BtnExtra,
    BtnForward,
    BtnBack,
    BtnTask,
    Raw(u32),
}

impl Into<u32> for &MouseEvent {
    fn into(self) -> u32 {
        match self {
            MouseEvent::BtnLeft => 0x110,
            MouseEvent::BtnRight => 0x111,
            MouseEvent::BtnMiddle => 0x112,
            MouseEvent::BtnSide => 0x113,
            MouseEvent::BtnExtra => 0x114,
            MouseEvent::BtnForward => 0x115,
            MouseEvent::BtnBack => 0x116,
            MouseEvent::BtnTask => 0x117,
            MouseEvent::Raw(raw) => *raw,
        }
    }
}
impl From<u32> for MouseEvent {
    fn from(value: u32) -> Self {
        match value {
            0x110 => MouseEvent::BtnLeft,
            0x111 => MouseEvent::BtnRight,
            0x112 => MouseEvent::BtnMiddle,
            0x113 => MouseEvent::BtnSide,
            0x114 => MouseEvent::BtnExtra,
            0x115 => MouseEvent::BtnForward,
            0x116 => MouseEvent::BtnBack,
            0x117 => MouseEvent::BtnTask,
            raw => MouseEvent::Raw(raw),
        }
    }
}

impl PartialEq for MouseEvent {
    fn eq(&self, other: &Self) -> bool {
        let r1: u32 = self.into();
        let r2: u32 = other.into();
        r1 == r2
    }
}

impl Eq for MouseEvent {}

#[derive(Debug)]
pub struct MouseMap {
    pub event: Mouse,
    pub modifier: Modifiers,
}

enum ScrollMotion {
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

#[derive(Hash, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub enum Mouse {
    // DescreteScroll(i32, ScrollMotion)
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    Button(MouseEvent),
}

impl MouseMap {
    pub fn new(event: Mouse, modifier: Modifiers) -> Self {
        MouseMap { event, modifier }
    }
}

impl PartialEq for MouseMap {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event
            && self.modifier.ctrl == other.modifier.ctrl
            && self.modifier.alt == other.modifier.alt
            && self.modifier.shift == other.modifier.shift
            && self.modifier.logo == other.modifier.logo
    }
}

impl Hash for MouseMap {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.event.hash(state);
        self.modifier.ctrl.hash(state);
        self.modifier.alt.hash(state);
        self.modifier.shift.hash(state);
        self.modifier.logo.hash(state);
    }
}

impl Serialize for MouseMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Mouse", 2)?;
        state.serialize_field("event", &self.event)?;
        let mut mods = Vec::new();
        if self.modifier.ctrl {
            mods.push("ctrl");
        }
        if self.modifier.alt {
            mods.push("alt");
        }
        if self.modifier.shift {
            mods.push("shift");
        }
        if self.modifier.logo {
            mods.push("logo");
        }
        state.serialize_field("modifier", &mods)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for MouseMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Event,
            Modifier,
        }

        struct KeyMapVisitor;

        impl<'de> Visitor<'de> for KeyMapVisitor {
            type Value = MouseMap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Mouse")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<MouseMap, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let event = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let v: Vec<String> = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let modifier = to_modifier(&v).map_err(de::Error::custom)?;
                Ok(MouseMap { event, modifier })
            }

            fn visit_map<V>(self, mut map: V) -> Result<MouseMap, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut event = None;
                let mut modifier = None;
                while let Some(f) = map.next_key()? {
                    match f {
                        Field::Event => {
                            if event.is_some() {
                                return Err(de::Error::duplicate_field("event"));
                            }
                            event = Some(map.next_value()?);
                        }
                        Field::Modifier => {
                            if modifier.is_some() {
                                return Err(de::Error::duplicate_field("modifier"));
                            }
                            let v: Vec<String> = map.next_value()?;
                            modifier = Some(to_modifier(&v).map_err(de::Error::custom)?);
                        }
                    }
                }
                let event = event.ok_or_else(|| de::Error::missing_field("key"))?;
                let modifier = modifier.ok_or_else(|| de::Error::missing_field("modifier"))?;
                Ok(MouseMap { event, modifier })
            }
        }

        const FIELDS: &'static [&'static str] = &["key", "modifier"];
        deserializer.deserialize_struct("Duration", FIELDS, KeyMapVisitor)
    }
}

impl Eq for MouseMap {}

fn to_modifier(slice: &[String]) -> Result<Modifiers, String> {
    let mut modifiers = Modifiers::default();
    for m in slice {
        match m.as_ref() {
            "ctrl" => modifiers.ctrl = true,
            "alt" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            "logo" => modifiers.logo = true,
            x => return Err(format!("{x} is not a modifier")),
        }
    }
    Ok(modifiers)
}
