use serde::{ser::SerializeStruct, Deserialize, Serialize};
use smithay_client_toolkit::shm::slot::Buffer;
use smithay_client_toolkit::{
    output::OutputInfo,
    shell::wlr_layer::{KeyboardInteractivity, LayerSurface},
    shm::{slot::SlotPool, Shm},
};
use wayland_client::{
    globals::GlobalList,
    protocol::{wl_output, wl_shm},
    Connection,
};

use crate::pause::{self, ScreenCopy};
use crate::tools::Tool;

pub fn restore(saved: &mut Vec<Saved>, info: &OutputInfo) -> Vec<Box<dyn Tool>> {
    let index = saved
        .iter()
        .position(|s| s.id == info.id && s.model == info.model && s.make == info.make);
    if let Some(index) = index {
        saved.remove(index).draws
    } else {
        Vec::new()
    }
}

#[derive(Deserialize)]
pub struct Saved {
    id: u32, // We use these 3 values to compare outputs, so we know if we should load an input
    model: String,
    make: String,
    draws: Vec<Box<dyn Tool>>,
}

pub struct OutPut {
    pub output: wl_output::WlOutput,
    pub width: u32,
    pub height: u32,
    pub info: OutputInfo,
    pub pool: SlotPool,
    pub buffers: Buffers,
    pub interactivity: KeyboardInteractivity,
    pub layer: LayerSurface,
    pub configured: bool,
    pub draws: Vec<Box<dyn Tool>>,
    pub screencopy: Option<ScreenCopy>,
}

impl Serialize for OutPut {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Saved", 4)?;
        state.serialize_field("id", &self.info.id)?;
        state.serialize_field("model", &self.info.model)?;
        state.serialize_field("make", &self.info.make)?;
        // state.serialize_field("draws", &self.draws)?;
        state.end()
    }
}

impl OutPut {
    pub fn toggle_screencopy_output(&mut self, conn: &Connection, globals: &GlobalList, shm: &Shm) {
        self.screencopy = match self.screencopy {
            Some(_) => None,
            None => pause::create_screenshot(conn, globals, shm, &self.output).ok(),
        }
    }
    pub fn toggle_passthrough(&mut self) {
        if self.interactivity == KeyboardInteractivity::Exclusive {
            self.interactivity = KeyboardInteractivity::None;
            self.layer
                .set_keyboard_interactivity(KeyboardInteractivity::None);
        } else {
            self.interactivity = KeyboardInteractivity::Exclusive;
            self.layer
                .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        }
    }
}

// Maybe move
pub struct Buffers {
    pub buffers: [Buffer; 2],
    pub current: usize,
}

impl Buffers {
    pub fn new(pool: &mut SlotPool, width: u32, height: u32, format: wl_shm::Format) -> Buffers {
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

    pub fn flip(&mut self) {
        self.current = 1 - self.current
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffers[self.current]
    }

    pub fn canvas<'a>(&'a self, pool: &'a mut SlotPool) -> Option<&mut [u8]> {
        self.buffers[self.current].canvas(pool)
    }
}
