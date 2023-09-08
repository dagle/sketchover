use core::fmt;
use std::{collections::HashMap, default, hash::Hash};

use clap::Parser;
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize,
};
use smithay_client_toolkit::seat::keyboard::Modifiers;

use crate::draw::DrawKind;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The size of the pen used for drawing
    #[clap(short, long)]
    size: Option<f32>,

    #[clap(short, long)]
    color: Option<String>,

    /// Colors in the palette other than the current color
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ' ')]
    palette: Option<Vec<String>>,

    #[clap(short, long)]
    distance: Option<bool>,

    // #[clap(long, value_enum)]
    // unit: Option<Unit>,
    /// Foreground color
    #[clap(short, long)]
    foreground: Option<String>,

    #[clap(long)]
    text_color: Option<String>,

    #[clap(short = 't', long, value_enum)]
    starting_tool: Option<DrawKind>,

    #[clap(long)]
    font: Option<String>,

    #[clap(long)]
    font_size: Option<f32>,

    /// Should sketchover start in paused state
    #[clap(long)]
    paused: Option<bool>,

    /// How senisitve should scrolling be
    #[clap(long)]
    pub scroll_margine: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub size: f32,
    pub color: String,
    pub palette: Vec<String>,
    pub distance: bool,
    pub scroll_margine: f64,
    pub foreground: String,
    pub text_color: String,
    pub starting_tool: DrawKind,
    pub font: Option<String>,
    pub font_size: f32,
    pub paused: bool,
    pub key_map: HashMap<KeyMap, Command>,
    pub mouse_map: HashMap<MouseMap, Command>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Clear,
    Undo,
    NextColor,
    PrevColor,
    NextTool,
    PrevTool,
    ToggleDistance,
    IncreaseSize(f32),
    DecreaseSize(f32),
    TogglePause,
    Execute(String),
    Save,
    Combo(Vec<Command>),
    Nop,
    // draw action
    // DrawStart,
    // AltDrawStart,
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Key {
    pub key: String,
    pub modifier: Vec<String>,
}

#[derive(Hash, Serialize, Deserialize, Eq, PartialEq)]
pub struct MouseMap {
    pub event: Mouse,
    // pub modifier: Modifiers
}

#[derive(Hash, Serialize, Deserialize, Eq, PartialEq)]
pub enum Mouse {
    HorizontalScroll,
    VerticalScroll,
    Button(u32),
}

#[derive(Debug)]
pub struct KeyMap {
    pub key: u32,
    pub modifier: Modifiers,
}

impl KeyMap {
    fn new(key: &str, modifier: Modifiers) -> Self {
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
        deserializer.deserialize_struct("Duration", FIELDS, KeyMapVisitor)
    }
}

impl Eq for KeyMap {}

pub fn to_modifier(slice: &[String]) -> Result<Modifiers, String> {
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

impl From<Key> for KeyMap {
    fn from(key: Key) -> Self {
        let mut modifiers = Modifiers::default();
        for m in key.modifier {
            match m.as_ref() {
                "ctrl" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "logo" => modifiers.logo = true,
                _ => {
                    panic!("Unknown modifier key: {m}");
                }
            }
        }
        let keysym =
            xkbcommon::xkb::keysym_from_name(&key.key, xkbcommon::xkb::KEYSYM_CASE_INSENSITIVE);
        KeyMap {
            key: keysym,
            modifier: modifiers,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut key_map = HashMap::new();
        key_map.insert(KeyMap::new("c", Modifiers::default()), Command::Clear);
        key_map.insert(KeyMap::new("u", Modifiers::default()), Command::Undo);
        key_map.insert(KeyMap::new("n", Modifiers::default()), Command::NextColor);
        key_map.insert(KeyMap::new("N", Modifiers::default()), Command::PrevColor);
        key_map.insert(KeyMap::new("t", Modifiers::default()), Command::NextTool);
        key_map.insert(KeyMap::new("T", Modifiers::default()), Command::PrevTool);
        key_map.insert(
            KeyMap::new("d", Modifiers::default()),
            Command::ToggleDistance,
        );
        key_map.insert(
            KeyMap::new("plus", Modifiers::default()),
            Command::IncreaseSize(1.),
        );
        key_map.insert(
            KeyMap::new("minus", Modifiers::default()),
            Command::DecreaseSize(1.),
        );
        key_map.insert(KeyMap::new("p", Modifiers::default()), Command::TogglePause);
        key_map.insert(
            KeyMap::new("x", Modifiers::default()),
            Command::Execute("grim -g \"$(slurp)\"".to_owned()),
        );
        key_map.insert(
            KeyMap::new("m", Modifiers::default()),
            Command::Combo(vec![Command::Clear, Command::IncreaseSize(1.)]),
        );
        let mut mouse_map = HashMap::new();
        mouse_map.insert(
            MouseMap {
                event: Mouse::Button(2),
            },
            Command::PrevTool,
        );

        Config {
            size: 1.,
            color: String::from("#FF0000FF"),
            palette: vec!["#00FF00FF".to_owned(), "#0000FFFF".to_owned()],
            distance: false,
            foreground: String::from("#00000000"),
            text_color: String::from("#FFFFFFFF"),
            starting_tool: DrawKind::Pen,
            font: None,
            scroll_margine: 3.0,
            font_size: 12.,
            paused: false,
            key_map,
            mouse_map,
        }
    }
}

macro_rules! overwrite {
    ($var:expr, $replace:expr) => {
        if let Some(replace) = $replace {
            $var = replace;
        }
    };
}

impl Config {
    pub fn load(args: Args) -> Result<Config, confy::ConfyError> {
        let mut cfg: Config = confy::load("sketchover", None)?;
        // TODO: This is beyond horrible but I'm not writing this lib right now
        overwrite!(cfg.size, args.size);
        overwrite!(cfg.color, args.color);
        overwrite!(cfg.palette, args.palette);
        overwrite!(cfg.distance, args.distance);
        overwrite!(cfg.foreground, args.foreground);
        overwrite!(cfg.text_color, args.text_color);
        overwrite!(cfg.starting_tool, args.starting_tool);
        if let Some(font) = args.font {
            cfg.font = Some(font);
        }
        overwrite!(cfg.font_size, args.font_size);
        overwrite!(cfg.paused, args.paused);
        Ok(cfg)
    }
}

// let cfg: MyConfig = confy::load("my-app-name", None)?;
