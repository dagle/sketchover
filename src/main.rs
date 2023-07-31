mod draw;

use draw::{Draw, DrawKind};
use font_kit::{font::Font, source::SystemSource};
// use cursor_icon::CursorIcon;
use hex_color::{HexColor, ParseHexColorError};
use raqote::{DrawOptions, DrawTarget, Point, SolidSource, Source, StrokeStyle};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputInfo, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemedPointer},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

mod fk {
    pub use font_kit::canvas::{Canvas, Format, RasterizationOptions};
    pub use font_kit::font::Font;
    pub use font_kit::hinting::HintingOptions;
    pub use pathfinder_geometry::vector::{vec2f, vec2i};
}
use xkbcommon::xkb::keysyms;

use clap::{Parser, ValueEnum};

use crate::draw::DrawAction;

// const CURSORS: &[CursorIcon] = &[CursorIcon::Default, CursorIcon::Crosshair];

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Config {
    #[clap(short, long)]
    #[clap(default_value_t = 5)]
    size: u32,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("#FF0000FF"))]
    color: String,

    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ' ')]
    #[arg(default_values_t = ["#00FF00FF #0000FFFF".to_string()])]
    palette: Vec<String>,

    #[clap(short, long)]
    #[clap(default_value_t = false)]
    distance: bool,

    #[clap(long)]
    #[clap(default_value_t = Unit::Pixel, value_enum)]
    unit: Unit,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("#00000000"))]
    foreground: String,

    #[clap(long)]
    #[clap(default_value_t = String::from("#FFFFFFFF"))]
    text_color: String,

    #[clap(short = 't', long)]
    #[clap(default_value_t = DrawKind::Pen, value_enum)]
    starting_tool: DrawKind,

    #[clap(long)]
    font: Option<String>,

    #[clap(long)]
    #[clap(default_value_t = 12.)]
    font_size: f32,
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum Unit {
    Pixel,
    Cm,
}

fn parse_solid(str: &str) -> Result<SolidSource, ParseHexColorError> {
    let hex = HexColor::parse(str)?;
    Ok(SolidSource {
        r: hex.r,
        g: hex.g,
        b: hex.b,
        a: hex.a,
    })
}

fn main() {
    env_logger::init();
    let config = Config::parse();

    let conn = Connection::connect_to_env().expect("Couldn't connect wayland compositor");

    let (globals, mut event_queue) =
        registry_queue_init(&conn).expect("Couldn't create an event queue");
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);

    let output_state = OutputState::new(&globals, &qh);

    let seat_state = SeatState::new(&globals, &qh);

    // We don't need this one atm bu we will the future to set the set the cursor icon
    let compositor_state =
        CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("Layer shell is not available");

    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    let fgcolor = parse_solid(&config.foreground).expect("Couldn't parse foreground color");
    let draw_color = parse_solid(&config.color).expect("Couldn't parse draw color");
    let mut palette = Vec::new();

    palette.push(draw_color);
    for c in config.palette {
        let color = parse_solid(&c).expect("Couldn't parse palette color");
        palette.push(color);
    }

    let font = match config.font {
        Some(name) => SystemSource::new().select_by_postscript_name(&name),
        None => SystemSource::new().select_best_match(
            &[font_kit::family_name::FamilyName::Serif],
            &font_kit::properties::Properties::new(),
        ),
    }
    .expect("No suitable font found")
    .load()
    .expect("Couldn't load font");

    let font_color = parse_solid(&config.text_color).expect("Couldn't parse font color");

    let font_size = config.font_size;

    let mut sketch_over = SketchOver {
        registry_state,
        seat_state,
        output_state,
        compositor_state,
        layer_shell,
        shm,

        exit: false,
        drawing: false,
        distance: config.distance,
        keyboard: None,
        pointer: None,
        fgcolor,
        modifiers: Modifiers::default(),
        palette,
        palette_index: 0,
        current_style: StrokeStyle::default(),
        kind: config.starting_tool,
        themed_pointer: None,
        current_output: None,
        outputs: Vec::new(),
        font,
        font_color,
        font_size,
    };

    loop {
        match event_queue.blocking_dispatch(&mut sketch_over) {
            Err(e) => {
                eprintln!("Dispatch failed: {}", e);
                break;
            }
            _ => {}
        }

        if sketch_over.exit {
            break;
        }
    }
}

struct SketchOver {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm: Shm,
    outputs: Vec<OutPut>,

    exit: bool,
    drawing: bool,
    distance: bool,
    layer_shell: LayerShell,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    fgcolor: raqote::SolidSource,
    palette_index: usize,
    palette: Vec<raqote::SolidSource>,
    current_output: Option<usize>,
    kind: DrawKind,
    modifiers: Modifiers,
    current_style: StrokeStyle,
    themed_pointer: Option<ThemedPointer>,
    font: font_kit::loaders::freetype::Font,
    font_color: raqote::SolidSource,
    font_size: f32,
}

struct OutPut {
    width: u32,
    height: u32,
    info: OutputInfo,
    pool: SlotPool,
    layer: LayerSurface,
    configured: bool,
    draws: Vec<Draw>,
}

impl CompositorHandler for SketchOver {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh, surface);
    }
}

impl OutputHandler for SketchOver {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let surface = self.compositor_state.create_surface(&qh);
        let layer = self.layer_shell.create_layer_surface(
            &qh,
            surface,
            Layer::Top,
            Some("sketchover"),
            Some(&output),
        );

        let Some(info) = self.output_state.info(&output) else {
            eprintln!("Can't get screen info");
            self.exit = true;
            return;
        };

        let Some(logical) = info.logical_size else {
            eprintln!("Can't get logical info about the screen");
            self.exit = true;
            return;
        };

        let width = logical.0 as u32;
        let height = logical.1 as u32;

        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.set_exclusive_zone(-1);
        layer.set_size(width, height);

        layer.commit();

        let pool = SlotPool::new(width as usize * height as usize * 4, &self.shm)
            .expect("Failed to create pool");

        let output = OutPut {
            width,
            height,
            info,
            pool,
            layer,
            configured: false,
            draws: Vec::new(),
        };
        self.outputs.push(output);
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        // let output = self
        //     .outputs
        //     .iter_mut()
        //     .find(|x| x.layer.wl_surface() == &surface);
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for SketchOver {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if let Some(output) = self.outputs.iter_mut().find(|x| &x.layer == layer) {
            if output.configured {
                return;
            }
            if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            } else {
                output.width = configure.new_size.0;
                output.height = configure.new_size.1;
            }

            output.configured = true;
            self.draw(qh, layer.wl_surface());
        }
    }
}

impl SeatHandler for SketchOver {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .expect("Failed to create keyboard");
            self.keyboard = Some(keyboard);
        }

        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }

        // TODO: add this when 0.18 is out, I don't want to support the old API
        // if capability == Capability::Pointer && self.themed_pointer.is_none() {
        //     println!("Set pointer capability");
        //     let surface = self.compositor_state.create_surface(qh);
        //     let themed_pointer = self
        //         .seat_state
        //         .get_pointer_with_theme(
        //             qh,
        //             &seat,
        //             self.shm_state.wl_shm(),
        //             surface,
        //             ThemeSpec::default(),
        //         )
        //         .expect("Failed to create pointer");
        //     self.themed_pointer.replace(themed_pointer);
        // }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            self.keyboard.take().unwrap().release();
        }

        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for SketchOver {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _keysyms: &[u32],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            keysyms::KEY_Escape => {
                self.exit = true;
            }

            keysyms::KEY_c => {
                if let Some(idx) = self.current_output {
                    let output = &mut self.outputs[idx];
                    output.draws = Vec::new();
                }
            }
            keysyms::KEY_u => {
                if let Some(idx) = self.current_output {
                    let output = &mut self.outputs[idx];
                    output.draws.pop();
                }
            }
            keysyms::KEY_n => {
                self.next_color();
            }
            keysyms::KEY_N => {
                self.prev_color();
            }
            keysyms::KEY_t => {
                self.next_tool();
            }
            keysyms::KEY_T => {
                self.prev_tool();
            }
            keysyms::KEY_d => {
                self.distance = !self.distance;
            }
            keysyms::KEY_plus => {
                self.increase_size();
            }
            keysyms::KEY_minus => {
                self.decrease_size();
            }
            _ => {}
        }
        if event.keysym == keysyms::KEY_Escape {
            self.exit = true;
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
    ) {
        self.modifiers = modifiers;
        if let Some(idx) = self.current_output {
            let output = &mut self.outputs[idx];
            if self.drawing {
                if let Some(last) = output.draws.last_mut() {
                    last.add_motion(None, &self.modifiers);
                }
            }
        }
    }
}

impl PointerHandler for SketchOver {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            // Ignore events for other surfaces
            let output = self.output(&event.surface);
            let Some(output) = output else {
                continue;
            };
            match event.kind {
                Enter { .. } => {
                    // TODO: add this when 0.18 is out, I don't want to support the old API
                    // if let Some(themed_pointer) = self.themed_pointer.as_mut() {
                    //     themed_pointer.set_cursor(conn, cursor_icon);
                    // }
                    self.current_output = Some(output);
                }
                Leave { .. } => {}
                Motion { .. } => {
                    if let Some(output) = self
                        .outputs
                        .iter_mut()
                        .find(|x| x.layer.wl_surface() == &event.surface)
                    {
                        if self.drawing {
                            if let Some(last) = output.draws.last_mut() {
                                last.add_motion(Some(event.position), &self.modifiers);
                            }
                        }
                    }
                }
                Press { .. } => {
                    self.drawing = true;

                    if let Some(output) = self
                        .outputs
                        .iter_mut()
                        .find(|x| x.layer.wl_surface() == &event.surface)
                    {
                        let action = match self.kind {
                            DrawKind::Pen => DrawAction::Pen(Vec::new()),
                            DrawKind::Line => DrawAction::Line(event.position.0, event.position.1),
                            DrawKind::Rect => DrawAction::Rect(5.0, 5.0),
                            DrawKind::Circle => DrawAction::Circle(5.0, 5.0),
                        };
                        let draw = Draw {
                            start: event.position,
                            style: self.current_style.clone(),
                            color: self.palette[self.palette_index],
                            distance: self.distance,
                            action,
                        };
                        output.draws.push(draw);
                    }
                }
                Release { .. } => {
                    self.drawing = false;
                }
                Axis { .. } => {}
            }
        }
    }
}

impl ShmHandler for SketchOver {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SketchOver {
    pub fn next_color(&mut self) {
        let len = self.palette.len();
        self.palette_index = (self.palette_index + 1) % len;
    }

    pub fn prev_color(&mut self) {
        let len = self.palette.len();
        self.palette_index = if self.palette_index == 0 && len > 0 {
            len - 1
        } else {
            self.palette_index - 1
        };
    }

    pub fn next_tool(&mut self) {
        self.kind = match self.kind {
            DrawKind::Pen => DrawKind::Line,
            DrawKind::Line => DrawKind::Rect,
            DrawKind::Rect => DrawKind::Circle,
            DrawKind::Circle => DrawKind::Pen,
        };
    }

    pub fn prev_tool(&mut self) {
        self.kind = match self.kind {
            DrawKind::Pen => DrawKind::Circle,
            DrawKind::Line => DrawKind::Pen,
            DrawKind::Rect => DrawKind::Line,
            DrawKind::Circle => DrawKind::Rect,
        };
    }

    pub fn increase_size(&mut self) {
        self.current_style.width += 1.;
    }

    pub fn decrease_size(&mut self) {
        self.current_style.width -= 1.;
        if self.current_style.width < 0. {
            self.current_style.width = 1.;
        }
    }

    pub fn output(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        for (idx, output) in self.outputs.iter().enumerate() {
            if output.layer.wl_surface() == surface {
                return Some(idx);
            }
        }
        None
    }

    pub fn draw(&mut self, qh: &QueueHandle<Self>, surface: &wl_surface::WlSurface) {
        if let Some(output) = self
            .outputs
            .iter_mut()
            .find(|x| x.layer.wl_surface() == surface)
        {
            let width = output.width;
            let height = output.height;
            let stride = output.width as i32 * 4;

            let (buffer, canvas) = output
                .pool
                .create_buffer(
                    width as i32,
                    height as i32,
                    stride,
                    wl_shm::Format::Argb8888,
                )
                .expect("create buffer");

            let mut dt = raqote::DrawTarget::from_backing(
                width as i32,
                height as i32,
                bytemuck::cast_slice_mut(canvas),
            );
            dt.clear(self.fgcolor);

            for draw in output.draws.iter() {
                draw.draw(&mut dt);

                if draw.distance {
                    draw.draw_size(
                        &mut dt,
                        &self.font,
                        self.font_size,
                        &self.font_color.into(),
                        &raqote::DrawOptions::default(),
                    );
                }
            }

            // Damage the entire window
            output
                .layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);

            // Request our next frame
            output
                .layer
                .wl_surface()
                .frame(qh, output.layer.wl_surface().clone());

            // Attach and commit to present.
            buffer
                .attach_to(output.layer.wl_surface())
                .expect("Can't attach the buffer");
            output.layer.commit();
        }

        // TODO save and reuse buffer when the window size is unchanged.  This is especially
        // useful if you do damage tracking, since you don't need to redraw the undamaged parts
        // of the canvas.
    }
}

delegate_registry!(SketchOver);

delegate_compositor!(SketchOver);
delegate_output!(SketchOver);
delegate_shm!(SketchOver);

delegate_seat!(SketchOver);
delegate_keyboard!(SketchOver);
delegate_pointer!(SketchOver);

delegate_layer!(SketchOver);

impl ProvidesRegistryState for SketchOver {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
