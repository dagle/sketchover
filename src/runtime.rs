use std::error;
use std::fs::File;

use calloop::{EventLoop, LoopSignal};
use cursor_icon::CursorIcon;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::compositor::CompositorHandler;
use smithay_client_toolkit::output::OutputHandler;
use smithay_client_toolkit::reexports::protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Modifiers};
use smithay_client_toolkit::seat::pointer::{
    PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::ShmHandler;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState,
    reexports::calloop_wayland_source::WaylandSource, registry::RegistryState, seat::SeatState,
    shell::wlr_layer::LayerShell, shm::Shm,
};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, registry_handlers,
};
use wayland_client::protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{delegate_noop, QueueHandle, EventQueue};
use wayland_client::{globals::registry_queue_init, Connection};

use wayland_client::globals::{BindError, GlobalList};
use xkbcommon::xkb::keysyms;

use crate::mousemap::{Mouse, MouseMap};
use crate::output::{self, Buffers, OutPut, Saved};
use crate::tools::Tool;

pub trait Events {
    fn init(&mut self) {}
    // type Item;
    // fn new_output(&self, runtime: &Runtime<Self::Item>);
    fn new_output(r: &mut Runtime<Self>, output: &mut OutPut)
    where
        Self: Sized;

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent)
    where
        Self: Sized;

    fn mousebinding(r: &mut Runtime<Self>, event: u32)
    where
        Self: Sized;
}

// The wayland state, not the sketchover runtime
pub struct WlState {
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
    themed_pointer: Option<ThemedPointer>,
    exit: LoopSignal,
}

pub struct Runtime<D> {
    pub data: D,

    wl_runtime: Option<WlState>,

    // theming, can we do these without
    // 400 different setters
    bgcolor: raqote::SolidSource,
    color: raqote::SolidSource,

    current_output: Option<usize>,
    outputs: Vec<OutPut>,

    drawing: bool,
    last_pos: Option<(f64, f64)>,
    distance: bool,

    cursor_icon: CursorIcon,

    modifiers: Modifiers,

    font_size: f32,
}

// impl<D: Bindable> Runtime<D> {
impl<D: Events + 'static> Runtime<D> {
    pub fn init(data: D) -> Runtime<D> {
        let bgcolor = raqote::SolidSource {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };

        // red is the default, for now
        let color = raqote::SolidSource {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };

        let runtime = Runtime {
            data,
            wl_runtime: None,
            bgcolor,
            color,
            current_output: None,
            outputs: Vec::new(),
            drawing: false,
            last_pos: None,
            distance: false,
            modifiers: Modifiers::default(),
            cursor_icon: CursorIcon::Default,
            font_size: 12.0,
        };
        runtime
    }

    pub fn run(&mut self) {
        let conn = Connection::connect_to_env().expect("Couldn't connect wayland compositor");
        let (globals, event_queue): (GlobalList, EventQueue<Runtime<D>>) =
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

        let wl_state = WlState {
            registry_state,
            seat_state,
            output_state,
            compositor_state,
            globals,
            shm,
            layer_shell,
            conn,
            keyboard: None,
            pointer: None,
            themed_pointer: None,
            exit: event_loop.get_signal(),
        };

        self.wl_runtime = Some(wl_state);

        event_loop
            .run(None, self, |_| {})
            .expect("Eventloop failed");

        // self.wl_runtime = None;
    }

    pub fn output(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.outputs
            .iter()
            .position(|o| o.layer.wl_surface() == surface)
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn set_drawing(&mut self, enable: bool) {
        self.drawing = enable;
    }

    pub fn start_drawing(&mut self, tool: Box<dyn Tool>) {
        if let Some(idx) = self.current_output {
            self.drawing = true;
            self.outputs[idx].start_draw(tool);
        }
    }

    pub fn exit(&self) {
        if let Some(ref rt) = self.wl_runtime {
            rt.exit.stop();
        }
    }

    pub fn clear(&mut self, all: bool) {
        if all {
            for output in self.outputs.iter_mut() {
                output.draws = Vec::new();
            }
        } else {
            if let Some(idx) = self.current_output {
                let output = &mut self.outputs[idx];
                output.draws = Vec::new();
            }
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
        // if let Some(idx) = self.current_output {
        //     let current = self.outputs.get_mut(idx).unwrap();
        //     current.toggle_screencopy_output(&self.conn, &self.globals, &self.shm);
        // }
    }

    pub fn set_color(&mut self, color: raqote::SolidSource) {
        self.color = color;
    }

    pub fn set_passthrough(&mut self, enable: bool) {
        // TODO: a way to specify the monitor
        for output in self.outputs.iter_mut() {
            output.set_enable(enable);
        }
    }

    pub fn set_fg(&mut self, color: raqote::SolidSource) {
        self.bgcolor = color;
    }

    // TODO: Fix this, this shouldn't toggle
    pub fn set_pause(&mut self, pause: bool) {
        // TODO: a way to specify the monitor?
        if let Some(idx) = self.current_output {
            let current = self.outputs.get_mut(idx).unwrap();
            if let Some(ref rt) = self.wl_runtime {
                current.set_screen_copy(&rt.conn, &rt.globals, &rt.shm, pause);
            }
        }
    }

    pub fn save(&mut self, path: &str) -> Result<(), Box<dyn error::Error>> {
        let file = File::create(path)?;
        serde_json::to_writer(file, &self.outputs)?;
        Ok(())
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

            // println!("out: {}", output.draws.len());
            for draw in output.draws.iter() {
                draw.draw(&mut dt);
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

impl<D: Events + 'static> CompositorHandler for Runtime<D> {
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

macro_rules! runtime {
    ($var:ident) => {
        $var.wl_runtime
            .as_mut()
            .expect("You are running without run")
    };
}

impl<D: Events + 'static> OutputHandler for Runtime<D> {
    fn output_state(&mut self) -> &mut OutputState {
        // This shouldn't happen, we shouldn't be able to start the compositor without
        // calling run
        if let Some(rt) = self.wl_runtime.as_mut() {
            &mut rt.output_state
        } else {
            panic!("You are running without run")
        }
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let rt = runtime!(self);
        let surface = rt.compositor_state.create_surface(qh);
        let layer = rt.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("sketchover"),
            Some(&output),
        );

        let Some(info) = rt.output_state.info(&output) else {
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

        let mut pool = SlotPool::new(width as usize * height as usize * 4, &rt.shm)
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

        D::new_output(self, &mut output);
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

impl<D: Events + 'static> LayerShellHandler for Runtime<D> {
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

impl<D: Events + 'static> SeatHandler for Runtime<D> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut runtime!(self).seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        let mut rt = runtime!(self);
        if capability == Capability::Keyboard && rt.keyboard.is_none() {
            let keyboard = rt
                .seat_state
                .get_keyboard(qh, &seat, None)
                .expect("Failed to create keyboard");
            rt.keyboard = Some(keyboard);
        }

        if capability == Capability::Pointer && rt.pointer.is_none() {
            let pointer = rt
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            rt.pointer = Some(pointer);
        }

        if capability == Capability::Pointer && rt.themed_pointer.is_none() {
            let surface = rt.compositor_state.create_surface(qh);
            let themed_pointer = rt
                .seat_state
                .get_pointer_with_theme(qh, &seat, rt.shm.wl_shm(), surface, ThemeSpec::default())
                .expect("Failed to create pointer");
            rt.themed_pointer.replace(themed_pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        let mut rt = runtime!(self);
        if capability == Capability::Keyboard && rt.keyboard.is_some() {
            rt.keyboard.take().unwrap().release();
        }

        if capability == Capability::Pointer && rt.pointer.is_some() {
            rt.pointer.take().unwrap().release();
        }

        if capability == Capability::Pointer && rt.themed_pointer.is_some() {
            rt.themed_pointer.take().unwrap().pointer().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<D: Events + 'static> KeyboardHandler for Runtime<D> {
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

        D::keybinding(self, event);
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

impl<D: Events + 'static> PointerHandler for Runtime<D> {
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
                    if let Some(themed_pointer) = runtime!(self).themed_pointer.as_mut() {
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
                            } else {
                                println!("This is empty!")
                            }
                        }
                    }
                }
                Press { button, .. } => {
                    D::mousebinding(self, button);
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
                    // let cmd = self.mouse_map.get(&mouse_map).unwrap_or(&Command::Nop);
                    // self.command(&cmd.clone());
                }
            }
        }
    }
}

delegate_registry!(@<D: Events + 'static> Runtime<D>);

delegate_compositor!(@<D: Events + 'static> Runtime<D>);
delegate_output!(@<D: Events + 'static> Runtime<D>);
delegate_shm!(@<D: Events + 'static> Runtime<D>);

delegate_seat!(@<D: Events + 'static> Runtime<D>);
delegate_keyboard!(@<D: Events + 'static> Runtime<D>);
delegate_pointer!(@<D: Events + 'static> Runtime<D>);

delegate_layer!(@<D: Events + 'static> Runtime<D>);

delegate_noop!(@<D: Events + 'static> Runtime<D>: ignore ZwlrScreencopyManagerV1);

impl<D: Events + 'static> ShmHandler for Runtime<D> {
    fn shm_state(&mut self) -> &mut Shm {
        &mut runtime!(self).shm
    }
}

impl<D: Events + 'static> ProvidesRegistryState for Runtime<D> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut runtime!(self).registry_state
    }
    registry_handlers![OutputState, SeatState];
}
