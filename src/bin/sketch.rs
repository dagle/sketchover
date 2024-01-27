use calloop::signals::Signal;
use calloop::signals::Signals;
use calloop::EventLoop;
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::draw::Draw;
use sketchover::tools::draw::line::Line;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::KeyEvent;
use xkbcommon::xkb::keysyms;
use xkbcommon::xkb::keysyms::KEY_P;
use xkbcommon::xkb::keysyms::KEY_Q;
use xkbcommon::xkb::keysyms::KEY_S;

struct Bindings {
    outputs: Vec<String>,
    saved: Vec<String>,
    save: bool,
}

pub enum MouseEvent {
    BtnLeft,
    BtnRight,
    BtnMiddle,
    BtnSide,
    BtnExtra,
    BtnForward,
    BtnBack,
    BtnTask,
    Raw(u32),
}

impl From<u32> for MouseEvent {
    fn from(value: u32) -> Self {
        match value {
            0x110 => MouseEvent::BtnLeft,
            0x111 => MouseEvent::BtnRight,
            0x112 => MouseEvent::BtnMiddle,
            0x113 => MouseEvent::BtnSide,
            0x114 => MouseEvent::BtnExtra,
            0x115 => MouseEvent::BtnForward,
            0x116 => MouseEvent::BtnBack,
            0x117 => MouseEvent::BtnTask,
            raw => MouseEvent::Raw(raw),
        }
    }
}

impl Events for Bindings {
    fn new_output(r: &mut Runtime<Self>, output: &mut OutPut) {
        let name = output.name();
        if r.data.saved.contains(&name) {
            // we have saved data, lets restore it
            // output.restore("");
        }

        r.data.outputs.push(name);
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent, press: bool) {
        let modifier = r.modifiers();
        let id = r.current_output_id();

        if !press {
            return;
        }

        match event.keysym.raw() {
            keysyms::KEY_Q if modifier.shift == true => {
                r.exit();
            }
            keysyms::KEY_S => {
                r.data.save = true;
            }
            keysyms::KEY_P => {
                if let Some(id) = id {
                    r.set_pause(true, id);
                }
            }
            keysyms::KEY_N => {
                if let Some(id) = id {
                    r.set_pause(false, id);
                }
            }
            _ => {}
        }
    }

    fn mousebinding(r: &mut Runtime<Self>, button: u32, press: bool) {
        let mouse: MouseEvent = button.into();

        if !press {
            r.stop_drawing();
            return;
        }

        match mouse {
            MouseEvent::BtnLeft => {
                r.start_drawing(Box::new(Line::new(r.pos(), Draw::default())));
            }
            _ => {}
        }
    }
}

fn main() {
    let b = Bindings {
        outputs: Vec::new(),
        saved: Vec::new(),
        save: false,
    };
    let mut rt = Runtime::init(b);
    let event_loop = EventLoop::try_new().expect("couldn't create event-loop");

    event_loop
        .handle()
        .insert_source(
            Signals::new(&[Signal::SIGTSTP]).unwrap(),
            move |evt, &mut (), runtime: &mut Runtime<Bindings>| {
                if evt.signal() == Signal::SIGTSTP {
                    runtime.set_passthrough(true);
                }
            },
        )
        .expect("Unable to configure signal handler");
    rt.run(event_loop);

    if rt.data.save {
        // the user wants to save on exit and we have exited.
        // rt.save("");
    }
}
