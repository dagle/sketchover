use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::KeyEvent;
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

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent) {
        let modifier = r.modifiers();

        match event.keysym.raw() {
            KEY_Q if modifier.shift == true => {
                r.exit();
            }
            KEY_S => {
                r.data.save = true;
            }
            _ => {}
        }
    }

    fn mousebinding(r: &mut Runtime<Self>, button: u32) {
        let mouse: MouseEvent = button.into();
        match mouse {
            MouseEvent::BtnLeft => {
                r.start_drawing(Box::new(Pen::new()));
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
    rt.run();

    if rt.data.save {
        // the user wants to save on exit and we have exited.
        // rt.save("");
    }
}