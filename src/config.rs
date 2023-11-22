use std::collections::HashMap;

use clap::Parser;
use serde::{Deserialize, Serialize};
use smithay_client_toolkit::seat::keyboard::Modifiers;

use crate::{
    keymap::KeyMap,
    mousemap::{Mouse, MouseEvent, MouseMap},
    tools::DrawKind,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The size of the pen used for drawing
    #[clap(short, long)]
    size: Option<f32>,

    /// Colors in the palette, first value will be the starting color
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ' ')]
    palette: Option<Vec<String>>,

    #[clap(short, long)]
    distance: Option<bool>,

    // #[clap(long, value_enum)]
    // unit: Option<Unit>,
    /// Foreground color
    #[clap(short, long)]
    background: Option<String>,

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
    pub scroll_treshold: Option<f64>,

    /// Save on exit
    #[clap(long)]
    pub save_on_exit: Option<bool>,

    /// Delete the save file after resuming
    #[clap(long)]
    pub delete_save_on_resume: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub size: f32,
    pub palette: Vec<String>,
    pub distance: bool,
    pub scroll_margine: f64,
    pub background: String,
    pub text_color: String,
    pub tools: Vec<DrawKind>,
    pub font: Option<String>,
    pub font_size: f32,
    pub paused: bool,
    pub scroll_threshold: f64,
    pub save_on_exit: bool,
    pub delete_save_on_resume: bool,
    // pub scroll_treshold: f64,
    pub key_map: HashMap<KeyMap, Command>,
    pub mouse_map: HashMap<MouseMap, Command>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Command {
    Clear,
    Undo,
    NextColor,
    PrevColor,
    SetColor(usize),
    NextTool,
    PrevTool,
    SetTool(usize),
    ToggleDistance,
    IncreaseSize(f32),
    DecreaseSize(f32),
    TogglePause,
    Execute(String),
    Save,
    Combo(Vec<Command>),
    Nop,
    DrawStart(usize, usize),
}

impl Command {
    pub fn draw_command(&self) -> bool {
        matches!(self, Command::DrawStart(_, _))
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
        key_map.insert(
            KeyMap::new("q", Modifiers::default()),
            Command::DrawStart(0, 0),
        );
        key_map.insert(KeyMap::new("s", Modifiers::default()), Command::Save);

        let mut mouse_map = HashMap::new();
        mouse_map.insert(
            MouseMap::new(Mouse::Button(MouseEvent::BtnLeft), Modifiers::default()),
            Command::DrawStart(0, 0),
        );
        mouse_map.insert(
            MouseMap::new(Mouse::Button(MouseEvent::BtnRight), Modifiers::default()),
            Command::DrawStart(0, 1),
        );
        mouse_map.insert(
            MouseMap::new(Mouse::ScrollUp, Modifiers::default()),
            Command::IncreaseSize(0.2),
        );
        mouse_map.insert(
            MouseMap::new(Mouse::ScrollDown, Modifiers::default()),
            Command::DecreaseSize(0.2),
        );
        mouse_map.insert(
            MouseMap::new(Mouse::Button(MouseEvent::BtnSide), Modifiers::default()),
            Command::PrevTool,
        );
        mouse_map.insert(
            MouseMap::new(Mouse::Button(MouseEvent::BtnExtra), Modifiers::default()),
            Command::NextTool,
        );

        Config {
            size: 1.,
            palette: vec![
                "#FF0000FF".to_owned(),
                "#00FF00FF".to_owned(),
                "#0000FFFF".to_owned(),
            ],
            distance: false,
            background: String::from("#FFFFFF40"),
            text_color: String::from("#FFFFFFFF"),
            tools: vec![DrawKind::Pen, DrawKind::Line, DrawKind::Rect],
            font: None,
            scroll_margine: 3.0,
            font_size: 12.,
            paused: false,
            key_map,
            mouse_map,
            scroll_threshold: 5.,
            save_on_exit: false,
            delete_save_on_resume: true,
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
        overwrite!(cfg.palette, args.palette);
        overwrite!(cfg.distance, args.distance);
        overwrite!(cfg.background, args.background);
        overwrite!(cfg.text_color, args.text_color);
        overwrite!(cfg.save_on_exit, args.save_on_exit);
        overwrite!(cfg.delete_save_on_resume, args.delete_save_on_resume);
        // overwrite!(cfg.starting_tool, args.starting_tool);
        if let Some(font) = args.font {
            cfg.font = Some(font);
        }
        overwrite!(cfg.font_size, args.font_size);
        overwrite!(cfg.paused, args.paused);
        Ok(cfg)
    }
}
