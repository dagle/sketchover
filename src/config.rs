use std::collections::HashMap;

use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::draw::DrawKind;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short, long)]
    size: Option<f32>,

    #[clap(short, long)]
    color: Option<String>,

    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ' ')]
    palette: Option<Vec<String>>,

    #[clap(short, long)]
    distance: Option<bool>,

    // #[clap(long, value_enum)]
    // unit: Option<Unit>,
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
    pub key_map: HashMap<String, Command>,
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
}

impl Default for Config {
    fn default() -> Self {
        let mut key_map = HashMap::new();
        key_map.insert("c".to_owned(), Command::Clear);
        key_map.insert("u".to_owned(), Command::Undo);
        key_map.insert("n".to_owned(), Command::NextColor);
        key_map.insert("N".to_owned(), Command::PrevColor);
        key_map.insert("t".to_owned(), Command::NextTool);
        key_map.insert("T".to_owned(), Command::PrevTool);
        key_map.insert("d".to_owned(), Command::ToggleDistance);
        key_map.insert("+".to_owned(), Command::IncreaseSize);
        key_map.insert("-".to_owned(), Command::DecreaseSize);
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
            key_map,
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
        Ok(cfg)
    }
}

// let cfg: MyConfig = confy::load("my-app-name", None)?;
