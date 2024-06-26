use std::error;
use std::fs::File;
use std::path::Path;

use calloop::{EventLoop, LoopSignal};
use cursor_icon::CursorIcon;
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

use wayland_client::globals::GlobalList;
use xkbcommon::xkb::keysyms;

use crate::mousemap::{Mouse, MouseMap};
use crate::output::OutPut;
use crate::tools::Tool;

pub trait Events {
    fn init(_r: &mut Runtime<Self>)
    where
        Self: Sized,
    {
    }

    fn new_output(r: &mut Runtime<Self>, output: &mut OutPut)
    where
        Self: Sized;

    fn destroy_output(_r: &mut Runtime<Self>, _output_id: u32)
    where
        Self: Sized,
    {
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent, press: bool)
    where
        Self: Sized;

    fn mousebinding(r: &mut Runtime<Self>, event: u32, press: bool)
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
    current_output: Option<usize>,
    outputs: Vec<OutPut>,

    drawing: bool,
    last_pos: Option<(f64, f64)>,
    last_serial: Option<u32>,

    cursor_icon: CursorIcon,

    modifiers: Modifiers,
}

// impl<D: Bindable> Runtime<D> {
impl<D: Events + 'static> Runtime<D> {
    pub fn current_output_id(&self) -> Option<u32> {
        self.current_output.map(|x| self.outputs[x].info.id)
    }

    pub fn init(data: D) -> Runtime<D> {
        Runtime {
            data,
            wl_runtime: None,
            current_output: None,
            outputs: Vec::new(),
            drawing: false,
            last_pos: None,
            last_serial: None,
            modifiers: Modifiers::default(),
            cursor_icon: CursorIcon::Default,
        }
    }

    pub fn run(&mut self, mut event_loop: EventLoop<Runtime<D>>) {
        let conn = Connection::connect_to_env().expect("Couldn't connect wayland compositor");
        let (globals, event_queue): (GlobalList, EventQueue<Runtime<D>>) =
            registry_queue_init(&conn).expect("Couldn't create an event queue");
        let qh = event_queue.handle();

        let registry_state = RegistryState::new(&globals);

        let output_state = OutputState::new(&globals, &qh);

        let seat_state = SeatState::new(&globals, &qh);

        let compositor_state =
            CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, &qh).expect("Layer shell is not available");

        let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

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
    }

    pub fn output(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.outputs
            .iter()
            .position(|o| o.layer.wl_surface() == surface)
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn pos(&self) -> (f64, f64) {
        self.last_pos.unwrap_or((0.0, 0.0))
    }

    /// Stop drawing
    pub fn stop_drawing(&mut self) {
        self.drawing = false;
    }

    /// Start drawing using the specified tool
    /// We will draw until stop drawing is called.
    pub fn start_drawing(&mut self, tool: Box<dyn Tool>) {
        if let Some(idx) = self.current_output {
            self.drawing = true;
            self.outputs[idx].start_draw(tool);
        }
    }

    /// Save all outputs to path
    /// To be able to resume later on
    pub fn save_all<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn error::Error>> {
        let file = File::create(path)?;
        for output in self.outputs.iter() {
            serde_json::to_writer(&file, output)?;
        }
        Ok(())
    }

    pub fn exit(&self) {
        if let Some(ref rt) = self.wl_runtime {
            rt.exit.stop();
        }
    }

    // TODO: Remove
    pub fn clear(&mut self, all: bool) {
        if all {
            for output in self.outputs.iter_mut() {
                output.draws = Vec::new();
            }
        } else if let Some(idx) = self.current_output {
            let output = &mut self.outputs[idx];
            output.draws = Vec::new();
        }
    }

    // optional oid?
    // TODO: Remove
    pub fn undo(&mut self) {
        if let Some(idx) = self.current_output {
            let output = &mut self.outputs[idx];
            output.draws.pop();
        }
    }
    pub fn locate_output(&mut self, id: Option<u32>) -> Option<&mut OutPut> {
        if let Some(id) = id {
            for output in self.outputs.iter_mut() {
                if output.info.id == id {
                    return Some(output);
                }
            }
            None
        } else {
            Some(&mut self.outputs[self.current_output.unwrap()])
        }
    }
    pub fn locate_output_idx(&mut self, id: Option<u32>) -> Option<usize> {
        if let Some(id) = id {
            for (i, output) in self.outputs.iter().enumerate() {
                if output.info.id == id {
                    return Some(i);
                }
            }
            return None;
        }
        self.current_output
    }

    // TODO: Remove
    // pub fn set_passthrough(&mut self, enable: bool) {
    //     for output in self.outputs.iter_mut() {
    //         output.set_enable(enable);
    //     }
    // }

    pub fn set_pause(&mut self, pause: bool, id: usize) {
        let output = self.outputs.get_mut(id).expect("Can't get screen");
        if let Some(ref rt) = self.wl_runtime {
            output.set_screen_copy(&rt.conn, &rt.globals, &rt.shm, pause);
        }
    }

    // TODO: Remove
    // pub fn save(&mut self, path: &str) -> Result<(), Box<dyn error::Error>> {
    //     let file = File::create(path)?;
    //     serde_json::to_writer(file, &self.outputs)?;
    //     Ok(())
    // }

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
                dt.clear(output.fgcolor);
            }

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
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
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

        let pool = SlotPool::new(width as usize * height as usize * 4, &rt.shm)
            .expect("Failed to create pool");

        let mut output = OutPut::new(output, width, height, info, pool, layer);

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
            D::destroy_output(self, self.outputs[index].info.id);
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
        let rt = runtime!(self);
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
        let state = runtime!(self);
        if capability == Capability::Keyboard && state.keyboard.is_some() {
            state.keyboard.take().unwrap().release();
        }

        if capability == Capability::Pointer && state.pointer.is_some() {
            state.pointer.take().unwrap().release();
        }

        if capability == Capability::Pointer && state.themed_pointer.is_some() {
            state.themed_pointer.take().unwrap().pointer().release();
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
        _id: u32,
        event: KeyEvent,
    ) {
        // Esc is hardcoded
        if event.keysym.raw() == keysyms::KEY_Escape {
            self.exit();
            return;
        }
        D::keybinding(self, event, true);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        D::keybinding(self, event, false);
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
                if let Some(_last) = output.draws.last_mut() {
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
                Press { button, serial, .. } => {
                    if self.last_serial.map_or(true, |s| s != serial) {
                        D::mousebinding(self, button, true);
                    }
                    self.last_serial = Some(serial);
                }
                Release { button, .. } => {
                    D::mousebinding(self, button, false);
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
                    // TODO: Handle these
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
