use core::fmt;
use std::sync::atomic::{Ordering, AtomicBool};

use smithay_client_toolkit::{
    reexports::protocols_wlr::screencopy::v1::client::{
        zwlr_screencopy_frame_v1::{ZwlrScreencopyFrameV1, self},
        zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    },
    shm::{Shm, slot::{SlotPool, self, Buffer, Slot}, raw::RawPool},
};
use wayland_client::{globals::GlobalList, Connection, protocol::{wl_shm::{WlShm, Format}, wl_output}, Dispatch, QueueHandle, delegate_noop, WEnum, DispatchError};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct FrameFormat {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FrameState {
    /// Compositor returned a failed event on calling `frame.copy`.
    Failed,
    /// Compositor sent a Ready event on calling `frame.copy`.
    Finished,
}

struct CaptureFrameState {
    pub format: Option<FrameFormat>,
    pub state: Option<FrameState>,
    pub buffer_done: AtomicBool,
}

#[derive(Debug)]
pub enum FrameError {
    DispatchError(DispatchError),
    NoFormat,
    FrameFailed,
}

impl std::error::Error for FrameError {
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::DispatchError(dispatch) => {
                write!(f, "Dispatch error: {dispatch}")
            }
            FrameError::NoFormat => {
                write!(f, "NoFormat error")
            }
            FrameError::FrameFailed => {
                write!(f, "Failed to render frame")
            }
        }
    }
}

impl From<DispatchError> for FrameError {
    fn from(value: DispatchError) -> Self {
        FrameError::DispatchError(value)
    }
}

pub struct ScreenCopy {
    pub format: FrameFormat,
    pub image: Buffer,
    pub slot: SlotPool,
}


pub fn create_screenshot(conn: &Connection, globals: &GlobalList, shm: &Shm, output: &wl_output::WlOutput)
    -> Result<ScreenCopy, FrameError> {
    let mut state = CaptureFrameState {
        format: None,
        state: None,
        buffer_done: AtomicBool::new(false),
    };

    let mut event_queue = conn.new_event_queue::<CaptureFrameState>();
    let qh = event_queue.handle();

    let screencopy_manager = globals.bind::<ZwlrScreencopyManagerV1, _, _>(&qh, 3..=3, ()).unwrap();

    // Capture output, but we don't want the cursor
    let frame: ZwlrScreencopyFrameV1 =
        screencopy_manager.capture_output(0, output, &qh, ());

    // Empty internal event buffer until buffer_done is set to true which is when the Buffer done
    // event is fired, aka the capture from the compositor is succesful.
    while !state.buffer_done.load(Ordering::SeqCst) {
        event_queue.blocking_dispatch(&mut state)?;
    }

    let format = match state.format {
        Some(f) => f,
        None => return Err(FrameError::NoFormat)
    };


    // TODO: Error!
    let mut pool = SlotPool::new(format.height as usize * format.stride as usize, shm).unwrap();

    // Instantiate shm global.
    // let shm_pool = pool.create_pool(fd.as_raw_fd(), frame_bytes as i32, &qh, ());
    let (buffer, _) = pool.create_buffer(
        format.width as i32,
        format.height as i32,
        format.stride as i32,
        format.format,
    ).unwrap();

    // Copy the pixel data advertised by the compositor into the buffer we just created.
    frame.copy(&buffer.wl_buffer());
    // On copy the Ready / Failed events are fired by the frame object, so here we check for them.
    loop {
        // Basically reads, if frame state is not None then...
        if let Some(state) = state.state {
            match state {
                FrameState::Failed => {
                    println!("Frame copy failed");
                    return Err(FrameError::FrameFailed);
                }
                FrameState::Finished => {
                    println!("Finished");
                    // buffer.destroy();
                    // shm_pool.destroy();
                    return Ok(ScreenCopy { format, image: buffer, slot: pool })
                }
            }
        }

        event_queue.blocking_dispatch(&mut state)?;
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for CaptureFrameState {
    fn event(
        frame: &mut Self,
        proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format, width, height, stride } => {
                    if let WEnum::Value(f) = format {
                        frame.format = Some(FrameFormat {
                            format: f,
                            width,
                            height,
                            stride
                        })
                    }
                }
            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                println!("flags!")
            },
            zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                println!("ready!");
                frame.state.replace(FrameState::Finished);
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                frame.state.replace(FrameState::Failed);
            }
            zwlr_screencopy_frame_v1::Event::Damage { x, y, width, height } => {
                println!("damage!")
            },
            zwlr_screencopy_frame_v1::Event::LinuxDmabuf { format, width, height } => {
                println!("dma!")
            },
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                frame.buffer_done.store(true, Ordering::SeqCst);
            },
            _ => unreachable!(),
        }
    }
}

// impl Dispatch<ZwlrScreencopyManagerV1, ()> for CaptureFrameState {
//     fn event(
//         state: &mut Self,
//         proxy: &ZwlrScreencopyManagerV1,
//         event: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
//         data: &(),
//         conn: &Connection,
//         qhandle: &QueueHandle<Self>,
//     ) {
//         todo!()
//     }
// }

delegate_noop!(CaptureFrameState: ignore ZwlrScreencopyManagerV1);
