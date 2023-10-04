use core::fmt;
use std::error;
use std::sync::atomic::{AtomicBool, Ordering};

use smithay_client_toolkit::{
    reexports::protocols_wlr::screencopy::v1::client::{
        zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
        zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm,
    },
};
use wayland_client::{
    delegate_noop,
    globals::GlobalList,
    protocol::{wl_output, wl_shm::Format},
    Connection, Dispatch, QueueHandle, WEnum,
};

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
    NoFormat,
    FrameFailed,
}

impl std::error::Error for FrameError {}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::NoFormat => {
                write!(f, "NoFormat error")
            }
            FrameError::FrameFailed => {
                write!(f, "Failed to render frame")
            }
        }
    }
}

#[derive(Debug)]
pub struct ScreenCopy {
    pub format: FrameFormat,
    pub image: Buffer,
    pub slot: SlotPool,
}

pub fn create_screenshot(
    conn: &Connection,
    globals: &GlobalList,
    shm: &Shm,
    output: &wl_output::WlOutput,
) -> Result<ScreenCopy, Box<dyn error::Error>> {
    let mut state = CaptureFrameState {
        format: None,
        state: None,
        buffer_done: AtomicBool::new(false),
    };

    let mut event_queue = conn.new_event_queue::<CaptureFrameState>();
    let qh = event_queue.handle();

    let screencopy_manager = globals.bind::<ZwlrScreencopyManagerV1, _, _>(&qh, 3..=3, ())?;

    // Capture output, but we don't want the cursor
    let frame: ZwlrScreencopyFrameV1 = screencopy_manager.capture_output(0, output, &qh, ());

    // Empty internal event buffer until buffer_done is set to true which is when the Buffer done
    // event is fired, aka the capture from the compositor is succesful.
    while !state.buffer_done.load(Ordering::SeqCst) {
        event_queue.blocking_dispatch(&mut state)?;
    }

    let format = match state.format {
        Some(f) => f,
        None => return Err(FrameError::NoFormat.into()),
    };

    let mut pool = SlotPool::new(format.height as usize * format.stride as usize, shm)?;

    let (buffer, canvas) = pool.create_buffer(
        format.width as i32,
        format.height as i32,
        format.stride as i32,
        format.format,
    )?;

    frame.copy(buffer.wl_buffer());
    loop {
        if let Some(state) = state.state {
            match state {
                FrameState::Failed => {
                    log::error!("Screencopy frame failed");
                    return Err(FrameError::FrameFailed.into());
                }
                FrameState::Finished => {
                    log::info!("Screencopy frame copied");
                    match format.format {
                        Format::Argb8888 => {}
                        Format::Xbgr8888 => {
                            log::info!("Screencopy frame converted from Xbgr8888 to Argb8888");
                            for chunk in canvas.chunks_exact_mut(4) {
                                chunk.swap(0, 2);
                            }
                        }
                        _ => {
                            panic!("Frame format not supported")
                        }
                    }
                    return Ok(ScreenCopy {
                        format,
                        image: buffer,
                        slot: pool,
                    });
                }
            }
        }

        event_queue.blocking_dispatch(&mut state)?;
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for CaptureFrameState {
    fn event(
        frame: &mut Self,
        _proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                log::info!("Got screencopy buffer info");
                if let WEnum::Value(f) = format {
                    frame.format = Some(FrameFormat {
                        format: f,
                        width,
                        height,
                        stride,
                    })
                }
            }
            zwlr_screencopy_frame_v1::Event::Flags { flags: _ } => {}
            zwlr_screencopy_frame_v1::Event::Ready {
                tv_sec_hi: _,
                tv_sec_lo: _,
                tv_nsec: _,
            } => {
                log::info!("Screencopy buffer is ready for copying");
                frame.state.replace(FrameState::Finished);
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                frame.state.replace(FrameState::Failed);
            }
            zwlr_screencopy_frame_v1::Event::Damage {
                x: _,
                y: _,
                width: _,
                height: _,
            } => {
                log::info!("Screencopy buffer was damaged");
            }
            zwlr_screencopy_frame_v1::Event::LinuxDmabuf {
                format: _,
                width: _,
                height: _,
            } => {}
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                log::info!("Screencopy buffer was done copying");
                frame.buffer_done.store(true, Ordering::SeqCst);
            }
            _ => unreachable!(),
        }
    }
}

delegate_noop!(CaptureFrameState: ignore ZwlrScreencopyManagerV1);
