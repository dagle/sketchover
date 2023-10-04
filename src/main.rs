use std::collections::HashMap;

use font_kit::source::SystemSource;
use hex_color::{HexColor, ParseHexColorError};
use raqote::{SolidSource, StrokeStyle};
use sketchover::config::{Args, Command, Config};
use sketchover::draw::{Draw, DrawAction, DrawKind};
use sketchover::keymap::KeyMap;
use sketchover::mousemap::{Mouse, MouseMap};
use sketchover::pause::{create_screenshot, ScreenCopy};
use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopSignal};
use smithay_client_toolkit::shm::slot::Buffer;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputInfo, OutputState},
    reexports::protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
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
use wayland_client::globals::GlobalList;
use wayland_client::WaylandSource;
use wayland_client::{
    delegate_noop,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use xkbcommon::xkb::keysyms;

use clap::{Parser, ValueEnum};

// const CURSORS: &[CursorIcon] = &[CursorIcon::Default, CursorIcon::Crosshair];

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
    let args = Args::parse();

    let config = Config::load(args).expect("Could not parse config file");

    let conn = Connection::connect_to_env().expect("Couldn't connect wayland compositor");

    let (globals, event_queue) =
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
    let mut palette = Vec::new();

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

    let current_style = raqote::StrokeStyle {
        width: config.size,
        ..StrokeStyle::default()
    };

    let mut event_loop = EventLoop::try_new().expect("couldn't create event-loop");

    let mut sketch_over = SketchOver {
        registry_state,
        seat_state,
        output_state,
        compositor_state,
        layer_shell,
        globals,
        shm,
        conn,

        exit: event_loop.get_signal(),
        drawing: false,
        distance: config.distance,
        keyboard: None,
        pointer: None,
        fgcolor,
        modifiers: Modifiers::default(),
        palette,
        palette_index: 0,
        current_style,
        tool_index: 0,
        tools: config.tools,
        // kind: config.starting_tool,
        themed_pointer: None,
        current_output: None,
        outputs: Vec::new(),
        font,
        font_color,
        font_size,
        start_paused: config.paused,
        key_map: config.key_map,
        mouse_map: config.mouse_map,
        last_pos: None,
    };

    let ws = WaylandSource::new(event_queue).unwrap();

    ws.insert(event_loop.handle()).unwrap();

    event_loop.run(None, &mut sketch_over, |_| {}).unwrap();
}

struct SketchOver {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    globals: GlobalList,
    shm: Shm,
    outputs: Vec<OutPut>,
    conn: Connection,

    exit: LoopSignal,
    drawing: bool,
    last_pos: Option<(f64, f64)>,
    distance: bool,
    layer_shell: LayerShell,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    fgcolor: raqote::SolidSource,
    palette_index: usize,
    palette: Vec<raqote::SolidSource>,
    current_output: Option<usize>,
    tool_index: usize,
    tools: Vec<DrawKind>,
    modifiers: Modifiers,
    current_style: StrokeStyle,
    themed_pointer: Option<ThemedPointer>,
    font: font_kit::loaders::freetype::Font,
    font_color: raqote::SolidSource,
    font_size: f32,
    start_paused: bool,
    key_map: HashMap<KeyMap, Command>,
    mouse_map: HashMap<MouseMap, Command>,
}

pub struct OutPut {
    output: wl_output::WlOutput,
    width: u32,
    height: u32,
    info: OutputInfo,
    pool: SlotPool,
    buffers: Buffers,
    layer: LayerSurface,
    configured: bool,
    draws: Vec<Draw>,
    screencopy: Option<ScreenCopy>,
}

impl OutPut {
    fn toggle_screencopy_output(&mut self, conn: &Connection, globals: &GlobalList, shm: &Shm) {
        self.screencopy = match self.screencopy {
            Some(_) => None,
            None => create_screenshot(conn, globals, shm, &self.output).ok(),
        }
    }
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
        let surface = self.compositor_state.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Top,
            Some("sketchover"),
            Some(&output),
        );

        let Some(info) = self.output_state.info(&output) else {
            log::error!("Can't get screen info for new output");
            self.exit.stop();
            return;
        };

        let Some(logical) = info.logical_size else {
            log::error!("Can't get logical info info for new output");
            self.exit.stop();
            return;
        };

        let width = logical.0 as u32;
        let height = logical.1 as u32;

        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.set_exclusive_zone(-1);
        layer.set_size(width, height);

        layer.commit();

        let mut pool = SlotPool::new(width as usize * height as usize * 4, &self.shm)
            .expect("Failed to create pool");

        let buffers = Buffers::new(&mut pool, width, height, wl_shm::Format::Argb8888);

        let mut output = OutPut {
            output,
            width,
            height,
            info,
            pool,
            layer,
            buffers,
            configured: false,
            draws: Vec::new(),
            screencopy: None,
        };
        if self.start_paused {
            output.toggle_screencopy_output(&self.conn, &self.globals, &self.shm)
        }
        self.outputs.push(output);
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        // if let Some(output) = self.outputs.iter().find(|o| o.output == output) {
        // }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(index) = self.outputs.iter().position(|o| o.output == output) {
            self.outputs.remove(index);
            if self.current_output.map(|i| i == index).unwrap_or(false) {
                self.current_output = None;
            }
        }
    }
}

impl LayerShellHandler for SketchOver {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit.stop();
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
        _: &wl_surface::WlSurface,
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
        _: &wl_surface::WlSurface,
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
        // Esc is hardcoded
        if event.keysym == keysyms::KEY_Escape {
            self.exit.stop();
            return;
        }

        let keymap = KeyMap {
            key: event.keysym,
            modifier: self.modifiers,
        };
        let cmd = self.key_map.get(&keymap).unwrap_or(&Command::Nop);
        self.command(&cmd.clone());
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _event: KeyEvent,
    ) {
        // Is checking that the key isn't a modifier enough?
        // or should we have save a key that triggered it?
        self.drawing = false;
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
            let Some(output) = self.output(&event.surface) else {
                continue;
            };
            match event.kind {
                Enter { .. } => {
                    // TODO: add this when 0.18 is out, I don't want to support the old API
                    // if let Some(themed_pointer) = self.themed_pointer.as_mut() {
                    //     themed_pointer.set_cursor(conn, cursor_icon);
                    // }
                    self.current_output = Some(output);
                    self.last_pos = Some(event.position);
                }
                Leave { .. } => {}
                Motion { .. } => {
                    if let Some(output) = self
                        .outputs
                        .iter_mut()
                        .find(|x| x.layer.wl_surface() == &event.surface)
                    {
                        self.last_pos = Some(event.position);
                        if self.drawing {
                            if let Some(last) = output.draws.last_mut() {
                                last.add_motion(Some(event.position), &self.modifiers);
                            }
                        }
                    }
                }
                Press { button, .. } => {
                    let mouse_map = MouseMap {
                        event: Mouse::Button(button.into()),
                        modifier: self.modifiers,
                    };

                    let cmd = self.mouse_map.get(&mouse_map).unwrap_or(&Command::Nop);
                    self.command(&cmd.clone());
                }
                Release { .. } => {
                    self.drawing = false;
                }
                Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    // TODO: handle descrete and contineous differetly

                    // descrete scrolling should should look for a treshold and
                    // then just wait for a stop.
                    println!("Horizontal: {:?}\nVertical: {:?}", horizontal, vertical);
                    let action = if vertical.absolute > 1. {
                        println!("1");
                        Mouse::ScrollDown
                    } else if vertical.absolute < -1. {
                        println!("2");
                        Mouse::ScrollUp
                        // scroll down
                    } else if horizontal.absolute > 1. {
                        println!("3");
                        Mouse::ScrollLeft
                        // scroll down
                    } else if horizontal.absolute < -1. {
                        println!("4");
                        Mouse::ScrollRight
                        // scroll down
                    } else {
                        println!("bail");
                        return;
                    };
                    let mouse_map = MouseMap {
                        event: action,
                        modifier: self.modifiers,
                    };
                    let cmd = self.mouse_map.get(&mouse_map).unwrap_or(&Command::Nop);
                    self.command(&cmd.clone());
                }
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
        if len == 0 {
            return;
        }
        self.palette_index = if self.palette_index == 0 {
            len - 1
        } else {
            self.palette_index - 1
        };
    }

    pub fn next_tool(&mut self) {
        let len = self.tools.len();
        self.tool_index = (self.tool_index + 1) % len;
    }

    pub fn prev_tool(&mut self) {
        let len = self.tools.len();
        if len == 0 {
            return;
        }
        self.tool_index = if self.tool_index == 0 {
            len - 1
        } else {
            self.tool_index - 1
        };
    }

    pub fn increase_size(&mut self, step: f32) {
        self.current_style.width += step;
    }

    pub fn decrease_size(&mut self, step: f32) {
        self.current_style.width -= step;
        self.current_style.width = f32::max(self.current_style.width, 1.);
    }

    pub fn output(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        for (idx, output) in self.outputs.iter().enumerate() {
            if output.layer.wl_surface() == surface {
                return Some(idx);
            }
        }
        None
    }

    pub fn draw_start(&mut self, kind: DrawKind, color: SolidSource) {
        self.drawing = true;

        let Some(idx) = self.current_output else {
            log::warn!("No current output found to start drawing");
            return;
        };

        let Some(pos) = self.last_pos else {
            log::warn!("No position recorded to start drawing");
            return;
        };

        let output = &mut self.outputs[idx];
        let action = match kind {
            DrawKind::Pen => DrawAction::Pen(Vec::new()),
            DrawKind::Line => DrawAction::Line(pos.0, pos.1),
            DrawKind::Rect => DrawAction::Rect(5.0, 5.0),
            DrawKind::Circle => DrawAction::Circle(pos.0 + 10.0, pos.1 + 10.0),
        };
        let draw = Draw {
            start: pos,
            style: self.current_style.clone(),
            color,
            distance: self.distance,
            action,
        };
        output.draws.push(draw);
    }

    pub fn command(&mut self, cmd: &Command) {
        match cmd {
            Command::Clear => {
                if let Some(idx) = self.current_output {
                    let output = &mut self.outputs[idx];
                    output.draws = Vec::new();
                }
            }
            Command::Undo => {
                if let Some(idx) = self.current_output {
                    let output = &mut self.outputs[idx];
                    output.draws.pop();
                }
            }
            Command::NextColor => self.next_color(),
            Command::PrevColor => self.prev_color(),
            Command::SetColor(idx) => {
                self.palette_index = *idx;
            }
            Command::NextTool => self.next_tool(),
            Command::PrevTool => self.prev_tool(),
            Command::SetTool(idx) => {
                self.tool_index = *idx;
            }
            Command::ToggleDistance => {
                self.distance = !self.distance;
            }
            Command::IncreaseSize(step) => self.increase_size(*step),
            Command::DecreaseSize(step) => self.decrease_size(*step),
            Command::TogglePause => {
                if let Some(idx) = self.current_output {
                    let current = self.outputs.get_mut(idx).unwrap();
                    current.toggle_screencopy_output(&self.conn, &self.globals, &self.shm);
                }
            }
            Command::Execute(ref cmd) => {
                if let Err(err) = std::process::Command::new("sh").arg("-c").arg(cmd).output() {
                    log::error!("Couldn't spawn process {cmd} with error: {err}");
                }
            }
            Command::Save => {
                // TODO: save output.draws into a file
                // how to handle multiple outputs? Save info about them
                // and when we deserilize we check if the outputs match?

                // How to resume? Since the output isn't created at startup but
                // rather in a callback. How would one resume this?
            }
            Command::Combo(cmds) => {
                for cmd in cmds {
                    self.command(cmd)
                }
            }
            // do nothing
            Command::Nop => {}
            Command::DrawStart(tidx, cidx) => {
                self.draw_start(
                    self.tools[(self.tool_index + tidx) % self.tools.len()],
                    self.palette[(self.palette_index + cidx) % self.palette.len()],
                );
            }
        }
    }

    pub fn draw(&mut self, qh: &QueueHandle<Self>, surface: &wl_surface::WlSurface) {
        if let Some(output) = self
            .outputs
            .iter_mut()
            .find(|x| x.layer.wl_surface() == surface)
        {
            let width = output.width;
            let height = output.height;

            let (buffer, canvas) = (
                output.buffers.buffer(),
                output
                    .buffers
                    .canvas(&mut output.pool)
                    .expect("Couldn't create canvas for drawing"),
            );

            if let Some(screen_copy) = &mut output.screencopy {
                let screen_canvas = screen_copy
                    .image
                    .canvas(&mut screen_copy.slot)
                    .expect("Couldn't copy the screencopy to the canvas");
                canvas.clone_from_slice(screen_canvas);
            } else {
                canvas.fill(0);
            }

            let mut dt = raqote::DrawTarget::from_backing(
                width as i32,
                height as i32,
                bytemuck::cast_slice_mut(canvas),
            );

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
            output.buffers.flip();
        }
    }
}

struct Buffers {
    buffers: [Buffer; 2],
    current: usize,
}

impl Buffers {
    fn new(pool: &mut SlotPool, width: u32, height: u32, format: wl_shm::Format) -> Buffers {
        Self {
            buffers: [
                pool.create_buffer(width as i32, height as i32, width as i32 * 4, format)
                    .expect("create buffer")
                    .0,
                pool.create_buffer(width as i32, height as i32, width as i32 * 4, format)
                    .expect("create buffer")
                    .0,
            ],
            current: 0,
        }
    }

    fn flip(&mut self) {
        self.current = 1 - self.current
    }

    fn buffer(&self) -> &Buffer {
        &self.buffers[self.current]
    }

    fn canvas<'a>(&'a self, pool: &'a mut SlotPool) -> Option<&mut [u8]> {
        self.buffers[self.current].canvas(pool)
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

delegate_noop!(SketchOver: ignore ZwlrScreencopyManagerV1);

impl ProvidesRegistryState for SketchOver {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
