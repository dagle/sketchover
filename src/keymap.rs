use core::fmt;
use std::hash::Hash;

use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize,
};
use smithay_client_toolkit::seat::keyboard::Modifiers;

#[derive(Debug)]
pub struct KeyMap {
    pub key: u32,
    pub modifier: Modifiers,
}

impl KeyMap {
    pub fn new(key: &str, modifier: Modifiers) -> Self {
        KeyMap {
            key: xkbcommon::xkb::keysym_from_name(key, xkbcommon::xkb::KEYSYM_NO_FLAGS),
            modifier,
        }
    }
}

impl PartialEq for KeyMap {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.modifier.ctrl == other.modifier.ctrl
            && self.modifier.alt == other.modifier.alt
            && self.modifier.shift == other.modifier.shift
            && self.modifier.logo == other.modifier.logo
    }
}

impl Eq for KeyMap {}

impl Hash for KeyMap {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.modifier.ctrl.hash(state);
        self.modifier.alt.hash(state);
        self.modifier.shift.hash(state);
        self.modifier.logo.hash(state);
    }
}

impl Serialize for KeyMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Key", 2)?;
        let key_str = xkbcommon::xkb::keysym_get_name(self.key);
        state.serialize_field("key", &key_str)?;
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

impl<'de> Deserialize<'de> for KeyMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Key,
            Modifier,
        }

        struct KeyMapVisitor;

        impl<'de> Visitor<'de> for KeyMapVisitor {
            type Value = KeyMap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Key")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<KeyMap, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let k: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let key = xkbcommon::xkb::keysym_from_name(&k, xkbcommon::xkb::KEYSYM_NO_FLAGS);
                let v: Vec<String> = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let modifier = to_modifier(&v).map_err(de::Error::custom)?;
                Ok(KeyMap { key, modifier })
            }

            fn visit_map<V>(self, mut map: V) -> Result<KeyMap, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut key = None;
                let mut modifier = None;
                while let Some(f) = map.next_key()? {
                    match f {
                        Field::Key => {
                            if key.is_some() {
                                return Err(de::Error::duplicate_field("key"));
                            }
                            let k: String = map.next_value()?;
                            key = Some(xkbcommon::xkb::keysym_from_name(
                                &k,
                                xkbcommon::xkb::KEYSYM_NO_FLAGS,
                            ));
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
                let key = key.ok_or_else(|| de::Error::missing_field("key"))?;
                let modifier = modifier.ok_or_else(|| de::Error::missing_field("modifier"))?;
                Ok(KeyMap { key, modifier })
            }
        }

        const FIELDS: &'static [&'static str] = &["key", "modifier"];
        deserializer.deserialize_struct("KeyMap", FIELDS, KeyMapVisitor)
    }
}

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
