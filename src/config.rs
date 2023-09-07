use std::collections::HashMap;

use clap::Parser;
use serde::{Deserialize, Serialize};

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
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub size: f32,
    pub color: String,
    pub palette: Vec<String>,
    pub distance: bool,
    pub foreground: String,
    pub text_color: String,
    pub starting_tool: DrawKind,
    pub font: Option<String>,
    pub font_size: f32,
    pub paused: bool,
    pub key_map: HashMap<String, Bind>,
    pub key_map2: HashMap<Key, Bind>,
}

#[derive(Serialize, Deserialize)]
pub enum Command {
    Clear,
    Undo,
    NextColor,
    PrevColor,
    NextTool,
    PrevTool,
    ToggleDistance,
    IncreaseSize,
    DecreaseSize,
    TogglePause,
    Execute,
}

// A keybinding or a mouse binding
#[derive(Serialize, Deserialize)]
pub struct Bind {
    pub command: Command,
    pub arg: Option<String>,
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Key {
    pub key: String,
    pub modifier: Vec<String>,
}

macro_rules! noarg {
    ($var:expr) => {
        Bind {
            command: $var,
            // arg: "".to_owned(),
            arg: None,
        }
    };
}

impl Default for Config {
    fn default() -> Self {
        let mut key_map = HashMap::new();
        key_map.insert("c".to_owned(), noarg!(Command::Clear));
        key_map.insert("u".to_owned(), noarg!(Command::Undo));
        key_map.insert("n".to_owned(), noarg!(Command::NextColor));
        key_map.insert("N".to_owned(), noarg!(Command::PrevColor));
        key_map.insert("t".to_owned(), noarg!(Command::NextTool));
        key_map.insert("T".to_owned(), noarg!(Command::PrevTool));
        key_map.insert("d".to_owned(), noarg!(Command::ToggleDistance));
        key_map.insert("plus".to_owned(), noarg!(Command::IncreaseSize));
        key_map.insert("minus".to_owned(), noarg!(Command::DecreaseSize));
        key_map.insert("p".to_owned(), noarg!(Command::TogglePause));
        key_map.insert(
            "x".to_owned(),
            Bind {
                command: Command::Execute,
                arg: Some("grim -g \"$(slurp)\"".to_owned()),
            },
        );
        let mut key_map2 = HashMap::new();
        key_map2.insert(
            Key {
                key: "c".to_owned(),
                modifier: Vec::new(),
            },
            noarg!(Command::Clear),
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
            font_size: 12.,
            paused: false,
            key_map,
            key_map2,
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
