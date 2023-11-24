use calloop::EventLoop;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState,
    reexports::calloop_wayland_source::WaylandSource, registry::RegistryState, seat::SeatState,
    shell::wlr_layer::LayerShell, shm::Shm,
};
use wayland_client::{globals::registry_queue_init, Connection};

use wayland_client::globals::GlobalList;

use crate::output::{self, Buffers, OutPut, Saved};

struct Runtime<D> {
    data: D,

    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    globals: GlobalList,
    shm: Shm,
    layer_shell: LayerShell,
    conn: Connection,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    bgcolor: raqote::SolidSource,

    current_output: Option<usize>,
    outputs: Vec<OutPut>,
    saved: Option<Vec<Saved>>,

    exit: LoopSignal,
    drawing: bool,
    last_pos: Option<(f64, f64)>,
    distance: bool,

    tool: Tool,
    modifiers: Modifiers,

    themed_pointer: Option<ThemedPointer>,
    cursor_icon: CursorIcon,

    font_size: f32,
    save_on_exit: bool,
}

impl<D> Runtime<D> {
    // TODO: remove all the expect
    pub fn init() -> Runtime {
        let conn = Connection::connect_to_env().expect("Couldn't connect wayland compositor");
        let (globals, event_queue) =
            registry_queue_init(&conn).expect("Couldn't create an event queue");
        let qh = event_queue.handle();

        let registry_state = RegistryState::new(&globals);

        let output_state = OutputState::new(&globals, &qh);

        let seat_state = SeatState::new(&globals, &qh);

        // We don't need this one atm but we will the future to set the set the cursor icon
        let compositor_state =
            CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, &qh).expect("Layer shell is not available");

        let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

        let mut event_loop = EventLoop::try_new().expect("couldn't create event-loop");
        let loop_handle = event_loop.handle();

        WaylandSource::new(conn.clone(), event_queue)
            .insert(loop_handle)
            .unwrap();
        event_loop
            .run(None, &mut sketch_over, |_| {})
            .expect("Eventloop failed")
    }

    fn event_loop<'l>(&self) -> &mut EventLoop<'l, D> {
        self.event_loop;
    }

    fn run(&mut self) {
        // self.event_loop
        //     .run(None, &mut sketch_over, |_| {})
        //     .expect("Eventloop failed")
    }
    
    pub fn clear(&mut self, all: bool) {
        if all {
            for output in self.outputs.iter() {
                output.draws = Vec::new();
            }
        } else {
            let output = &mut self.outputs[idx];
            output.draws = Vec::new();
        }
    }

    pub fn undo(&mut self) {
        if let Some(idx) = self.current_output {
            let output = &mut self.outputs[idx];
            output.draws.pop();
        }
    }
    // pub fn set_distance(&mut self) {
    //
    // }
    // pub fn unset_distance(&mut self) {
    //
    // }

    pub fn increase_size(&mut self) {
        if let Some(idx) = self.current_output {
            let current = self.outputs.get_mut(idx).unwrap();
            current.toggle_screencopy_output(&self.conn, &self.globals, &self.shm);
        }
    }
    pub fn set_fg(&mut self, color: raqote::SolidSource) {
        self.bgcolor = color;
    }
    pub fn pause(&mut self) {

    }
    pub fn unpause(&mut self) {

    }

    pub fn save() {

    }

    pub fn command(&mut self, cmd: &Command) {
        match cmd {
            // Command::NextColor => self.next_color(),
            // Command::PrevColor => self.prev_color(),
            Command::SetColor(idx) => {
                self.palette_index = *idx;
            }
            // Command::NextTool => self.next_tool(),
            // Command::PrevTool => self.prev_tool(),
            // Command::SetTool(idx) => {
            //     self.tool_index = *idx;
            // }
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
            Command::Save => {
                let _ = self.save();
            }
            Command::Combo(cmds) => {
                for cmd in cmds {
                    self.command(cmd)
                }
            }
            Command::ToggleFg => {
                let temp = self.bgcolor;
                self.bgcolor = self.alt_bgcolor;
                self.alt_bgcolor = temp;
            }
            // do nothing
            // Command::Nop => {}
            Command::DrawStart => {
                // check if we are already drawing? Or should we just
                // kidnapp the drawing
                // self.tools[(self.tool_index + tidx) % self.tools.len()],
                // self.palette[(self.palette_index + cidx) % self.palette.len()],
                self.draw_start();
            }
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>, surface: &wl_surface::WlSurface) {
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

            // If we have paused the screen, we draw our screenshot
            // on top. This gives the illusion that we have paused the screen.
            if let Some(ref mut screen_copy) = output.screencopy {
                let screen_canvas = screen_copy
                    .image
                    .canvas(&mut screen_copy.slot)
                    .expect("Couldn't copy the screencopy to the canvas");
                canvas.clone_from_slice(screen_canvas);
            }

            let mut dt = raqote::DrawTarget::from_backing(
                width as i32,
                height as i32,
                bytemuck::cast_slice_mut(canvas),
            );

            if output.screencopy.is_none() {
                dt.clear(self.bgcolor);
            }

            let mut pb = raqote::PathBuilder::new();

            pb.rect(0., 0., 400 as f32, 400 as f32);
            dt.fill(
                &pb.finish(),
                &Source::Solid(solid),
                &DrawOptions {
                    blend_mode: BlendMode::Src,
                    alpha: 1.,
                    antialias: AntialiasMode::Gray,
                },
            );

            for draw in output.draws.iter() {
                draw.draw(&mut dt);

                // if draw.distance {
                //     draw.draw_size(
                //         &mut dt,
                //         &self.font,
                //         self.font_size,
                //         &self.font_color.into(),
                //         &raqote::DrawOptions::default(),
                //     );
                // }
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

    fn transform_changed(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_transform: wl_output::Transform,
    ) {
        println!("Transform not implemented, your drawing might be weird");
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
            Layer::Overlay,
            Some("sketchover"),
            Some(&output),
        );

        let Some(info) = self.output_state.info(&output) else {
            log::error!("Can't get screen info for new output");
            self.exit();
            return;
        };

        let Some(logical) = info.logical_size else {
            log::error!("Can't get logical info info for new output");
            self.exit();
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

        // TODO: Add this again
        // let draws = if let Some(ref mut saved) = self.saved {
        //     output::restore(saved, &info)
        // } else {
        //     Vec::new()
        // };
        let draws = Vec::new();

        let mut output = OutPut {
            output,
            width,
            height,
            info,
            pool,
            layer,
            buffers,
            configured: false,
            draws,
            screencopy: None,
            interactivity: KeyboardInteractivity::Exclusive,
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
        self.exit();
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

        if capability == Capability::Pointer && self.themed_pointer.is_none() {
            let surface = self.compositor_state.create_surface(qh);
            let themed_pointer = self
                .seat_state
                .get_pointer_with_theme(qh, &seat, self.shm.wl_shm(), surface, ThemeSpec::default())
                .expect("Failed to create pointer");
            self.themed_pointer.replace(themed_pointer);
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

        if capability == Capability::Pointer && self.themed_pointer.is_some() {
            self.themed_pointer.take().unwrap().pointer().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for SketchOver {
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
        if event.keysym.raw() == keysyms::KEY_Escape {
            self.exit();
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
                    // TODO: Add modifier
                    // last.update(None, &self.modifiers);
                }
            }
        }
    }

    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[xkbcommon::xkb::Keysym],
    ) {
    }
}

impl PointerHandler for SketchOver {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
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
                    if let Some(themed_pointer) = self.themed_pointer.as_mut() {
                        // TODO: warn
                        let _ = themed_pointer.set_cursor(conn, self.cursor_icon);
                    }
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
                                last.update(event.position)
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
                    let action = if vertical.absolute > 1. {
                        Mouse::ScrollDown
                    } else if vertical.absolute < -1. {
                        Mouse::ScrollUp
                    } else if horizontal.absolute > 1. {
                        Mouse::ScrollLeft
                    } else if horizontal.absolute < -1. {
                        Mouse::ScrollRight
                    } else {
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

// delegate_registry!(SketchOver);
//
// delegate_compositor!(SketchOver);
// delegate_output!(SketchOver);
// delegate_shm!(SketchOver);
//
// delegate_seat!(SketchOver);
// delegate_keyboard!(SketchOver);
// delegate_pointer!(SketchOver);
//
// delegate_layer!(SketchOver);
//
// delegate_noop!(SketchOver: ignore ZwlrScreencopyManagerV1);

impl ShmHandler for SketchOver {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for SketchOver {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
