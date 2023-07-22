use hex_color::{HexColor, ParseHexColorError};
use raqote::{SolidSource, Transform};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
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
use xkbcommon::xkb::keysyms;

use clap::{Parser, ValueEnum};

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
    #[clap(default_value_t = String::from("#00000000"))]
    foreground: String,

    #[clap(short = 't', long)]
    #[clap(default_value_t = DrawKind::Pen, value_enum)]
    starting_tool: DrawKind,
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
    println!("palette: {:?}", config.palette);

    // All Wayland apps start by connecting the compositor (server).
    let conn = Connection::connect_to_env().unwrap();

    // Enumerate the list of globals to get the protocols the server implements.
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    // The compositor (not to be confused with the server which is commonly called the compositor) allows
    // configuring surfaces to be presented.
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    // This app uses the wlr layer shell, which may not be available with every compositor.
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");
    // Since we are not using the GPU in this example, we use wl_shm to allow software rendering to a buffer
    // we share with the compositor process.
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    // A layer surface is created from a surface.
    let surface = compositor.create_surface(&qh);

    // And then we create the layer shell.
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("simple_layer"), None);
    // Configure the layer surface, providing things like the anchor on screen, desired size and the keyboard
    // interactivity
    layer.set_anchor(Anchor::TOP);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_size(1280, 1024);

    // In order for the layer surface to be mapped, we need to perform an initial commit with no attached\
    // buffer. For more info, see WaylandSurface::commit
    //
    // The compositor will respond with an initial configure that we can then use to present to the layer
    // surface with the correct options.
    layer.commit();

    // We don't know how large the window will be yet, so lets assume the minimum size we suggested for the
    // initial memory allocation.
    let pool = SlotPool::new(1280 * 1024 * 4, &shm).expect("Failed to create pool");

    let fgcolor = parse_solid(&config.foreground).expect("Couldn't parse foreground color");
    let draw_color = parse_solid(&config.color).expect("Couldn't parse draw color");
    let mut palette = Vec::new();

    palette.push(draw_color);
    for c in config.palette {
        let color = parse_solid(&c).expect("Couldn't parse palette color");
        palette.push(color);
    }
    // let draw_color = parse_solid(&config.color).expect("Couldn't parse draw color");

    // let fgcolor = config.foreground.try_into().expect("apa");

    let mut simple_layer = SimpleLayer {
        // Seats and outputs may be hotplugged at runtime, therefore we need to setup a registry state to
        // listen for seats and outputs.
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,

        clear: true,
        exit: false,
        drawing: false,
        first_configure: true,
        pool,
        width: 1280,
        height: 1024,
        layer,
        keyboard: None,
        keyboard_focus: false,
        pointer: None,
        draws: Vec::new(),
        fgcolor,
        modifiers: Modifiers::default(),
        draw_color,
        palette,
        palette_index: 0,
        // fgcolor,
        kind: config.starting_tool,
    };

    // We don't draw immediately, the configure will notify us when to first draw.
    loop {
        event_queue.blocking_dispatch(&mut simple_layer).unwrap();

        if simple_layer.exit {
            break;
        }
    }
}

struct SimpleLayer {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,

    clear: bool,
    exit: bool,
    drawing: bool,
    first_configure: bool,
    pool: SlotPool,
    width: u32,
    height: u32,
    layer: LayerSurface,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: bool,
    pointer: Option<wl_pointer::WlPointer>,
    fgcolor: raqote::SolidSource,
    draw_color: raqote::SolidSource,
    palette_index: usize,
    palette: Vec<raqote::SolidSource>,
    draws: Vec<Draw>,
    kind: DrawKind,
    modifiers: Modifiers,
}

struct Draw {
    start: (f64, f64),
    size: u32,
    color: raqote::SolidSource,
    action: DrawAction,
    // draw style (solid, dotted, etc)
    //
    // lines: Vec<(f64, f64)>,
}

impl Draw {
    fn add_motion(&mut self, motion: (f64, f64)) {
        match self.action {
            DrawAction::Pen(ref mut pen) => pen.push(motion),
            DrawAction::Line(_, _) => self.action = DrawAction::Line(motion.0, motion.1),
            DrawAction::Box(_, _) => {
                self.action = DrawAction::Box(motion.0 - self.start.0, motion.1 - self.start.1)
            }
            DrawAction::Circle(_, _) => self.action = DrawAction::Circle(motion.0, motion.1),
        }
    }
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum DrawKind {
    Pen,
    Line,
    Box,
    Circle,
}

enum DrawAction {
    Pen(Vec<(f64, f64)>),
    Line(f64, f64),
    Box(f64, f64),
    Circle(f64, f64),
}

// struct Pen {
//     strokes: Vec<(f64, f64)>,
// }

// impl Draw {
//     fn new() -> Self {
//         Draw { lines: Vec::new() }
//     }
// }

impl CompositorHandler for SimpleLayer {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
    }
}

impl OutputHandler for SimpleLayer {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for SimpleLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 1280;
            self.height = 1024;
        } else {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }

        // Initiate the first draw.
        if self.first_configure {
            self.first_configure = false;
            self.draw(qh);
        }
    }
}

impl SeatHandler for SimpleLayer {
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

impl KeyboardHandler for SimpleLayer {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        keysyms: &[u32],
    ) {
        if self.layer.wl_surface() == surface {
            self.keyboard_focus = true;
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if self.layer.wl_surface() == surface {
            self.keyboard_focus = false;
        }
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
                self.clear = true;
                self.draws = Vec::new();
            }
            keysyms::KEY_u => {
                self.draws.pop();
            }
            keysyms::KEY_n => {
                self.next_color();
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
        event: KeyEvent,
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
        self.modifiers = modifiers
    }
}

impl PointerHandler for SimpleLayer {
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
            if &event.surface != self.layer.wl_surface() {
                continue;
            }
            match event.kind {
                Enter { .. } => {}
                Leave { .. } => {}
                Motion { .. } => {
                    if self.drawing {
                        if let Some(last) = self.draws.last_mut() {
                            last.add_motion(event.position);
                        }
                    }
                }
                Press { button, .. } => {
                    self.drawing = true;

                    let action = match self.kind {
                        DrawKind::Pen => DrawAction::Pen(Vec::new()),
                        DrawKind::Line => DrawAction::Line(event.position.0, event.position.1),
                        DrawKind::Box => DrawAction::Box(5.0, 5.0),
                        DrawKind::Circle => DrawAction::Circle(event.position.0, event.position.1),
                    };
                    let draw = Draw {
                        start: event.position,
                        size: 5,
                        color: self.draw_color,
                        action,
                    };
                    self.draws.push(draw);
                }
                Release { button, .. } => {
                    self.drawing = false;
                }
                Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    println!("Scroll H:{horizontal:?}, V:{vertical:?}");
                }
            }
        }
    }
}

impl ShmHandler for SimpleLayer {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SimpleLayer {
    pub fn next_color(&mut self) {
        let len = self.palette.len();
        self.palette_index = (self.palette_index + 1) % len;

        self.draw_color = self.palette[self.palette_index];
    }
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        let width = self.width;
        let height = self.height;
        let stride = self.width as i32 * 4;

        // let mut pool = self.pool;
        let (buffer, canvas) = self
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
        // if self.clear {
        dt.clear(self.fgcolor);
        self.clear = false;
        // }

        for draw in self.draws.iter() {
            let mut pb = raqote::PathBuilder::new();
            pb.move_to(draw.start.0 as f32, draw.start.1 as f32);
            match draw.action {
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
                DrawAction::Box(w, h) => {
                    pb.rect(draw.start.0 as f32, draw.start.1 as f32, w as f32, h as f32);
                }
                DrawAction::Circle(_, _) => {
                    pb.arc(
                        draw.start.0 as f32,
                        draw.start.1 as f32,
                        20.,
                        0. * std::f32::consts::PI,
                        4. * std::f32::consts::PI,
                    );
                }
            }
            dt.stroke(
                &pb.finish(),
                &raqote::Source::Solid(draw.color),
                &raqote::StrokeStyle::default(),
                &raqote::DrawOptions::new(),
            );
        }

        // Damage the entire window
        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);

        // Request our next frame
        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());

        // Attach and commit to present.
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("buffer attach");
        self.layer.commit();

        // TODO save and reuse buffer when the window size is unchanged.  This is especially
        // useful if you do damage tracking, since you don't need to redraw the undamaged parts
        // of the canvas.
    }
}

delegate_compositor!(SimpleLayer);
delegate_output!(SimpleLayer);
delegate_shm!(SimpleLayer);

delegate_seat!(SimpleLayer);
delegate_keyboard!(SimpleLayer);
delegate_pointer!(SimpleLayer);

delegate_layer!(SimpleLayer);

delegate_registry!(SimpleLayer);

impl ProvidesRegistryState for SimpleLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
