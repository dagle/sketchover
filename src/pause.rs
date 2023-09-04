use smithay_client_toolkit::{
    reexports::protocols_wlr::screencopy::v1::client::{
        zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    },
    shm::Shm,
};
use wayland_client::{globals::GlobalList, Connection};

struct CaptureFrameState {}

pub fn create_screnshot(conn: &Connection, globals: &GlobalList, shm: &Shm) {
    let mut state = CaptureFrameState {
        // formats: Vec::new(),
        // state: None,
        // buffer_done: AtomicBool::new(false),
    };

    let mut event_queue = conn.new_event_queue::<CaptureFrameState>();
    let qh = event_queue.handle();

    let screencopy_manager = match globals.bind::<ZwlrScreencopyManagerV1, _, _>(&qh, 3..=3, ()) {
        Ok(x) => x,
        Err(e) => {
            // log::error!("Failed to create screencopy manager. Does your compositor implement ZwlrScreencopy?");
            // log::error!("err: {e}");
            // return Err(Error::ProtocolNotFound(
            //     "ZwlrScreencopy Manager not found".to_string(),
            // ));
        }
    };
    // Capture output.
    let frame: ZwlrScreencopyFrameV1 =
        screencopy_manager.capture_output(cursor_overlay, output, &qh, ());

    // Empty internal event buffer until buffer_done is set to true which is when the Buffer done
    // event is fired, aka the capture from the compositor is succesful.
    while !state.buffer_done.load(Ordering::SeqCst) {
        event_queue.blocking_dispatch(&mut state)?;
    }

    log::debug!(
        "Received compositor frame buffer formats: {:#?}",
        state.formats
    );
    // Filter advertised wl_shm formats and select the first one that matches.
    let frame_format = state
        .formats
        .iter()
        .find(|frame| {
            matches!(
                frame.format,
                wl_shm::Format::Xbgr2101010
                    | wl_shm::Format::Abgr2101010
                    | wl_shm::Format::Argb8888
                    | wl_shm::Format::Xrgb8888
                    | wl_shm::Format::Xbgr8888
            )
        })
        .copied();
    log::debug!("Selected frame buffer format: {:#?}", frame_format);

    // Check if frame format exists.
    let frame_format = match frame_format {
        Some(format) => format,
        None => {
            log::error!("No suitable frame format found");
            return Err(Error::NoSupportedBufferFormat);
        }
    };

    // Bytes of data in the frame = stride * height.
    let frame_bytes = frame_format.stride * frame_format.height;
    if let Some(file) = file {
        file.set_len(frame_bytes as u64)?;
    }
    // Create an in memory file and return it's file descriptor.

    // Instantiate shm global.
    let shm_pool = shm.create_pool(fd.as_raw_fd(), frame_bytes as i32, &qh, ());
    let buffer = shm_pool.create_buffer(
        0,
        frame_format.width as i32,
        frame_format.height as i32,
        frame_format.stride as i32,
        frame_format.format,
        &qh,
        (),
    );

    // Copy the pixel data advertised by the compositor into the buffer we just created.
    frame.copy(&buffer);
    // On copy the Ready / Failed events are fired by the frame object, so here we check for them.
    loop {
        // Basically reads, if frame state is not None then...
        if let Some(state) = state.state {
            match state {
                FrameState::Failed => {
                    log::error!("Frame copy failed");
                    return Err(Error::FramecopyFailed);
                }
                FrameState::Finished => {
                    buffer.destroy();
                    shm_pool.destroy();
                    return Ok(frame_format);
                }
            }
        }

        event_queue.blocking_dispatch(&mut state)?;
    }
}
