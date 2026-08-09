// TODO: Figure out naming of things (make things more consistent) and figure out what goes where (Like ScreenSpace and such).
//! List of structs and methods to allow the user to interface with the wayland compositor.

use serde::{Deserialize, Serialize};

use std::error::Error;
use std::fmt::Display;
use std::thread;

use crossbeam_channel::Sender;
use smithay_client_toolkit::reexports::calloop::{self};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;

use crate::compositor_interface::compositor_client::{CallbackReason, CompositorClient};

pub(crate) mod compositor_client;
pub(crate) mod compositor_context;
/// Interfaces with the display outputs.
pub(crate) mod shell_layers;

pub type Pixel = i32;
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
/// Positional information for a screen.
pub struct ScreenInfo {
    /// Size of the screen.
    pub size: Size,
    /// Position of the origin relative ot the other monitors.
    pub origin: Point,
}

/// Span of the output monitors.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Range {
    /// Top left corner pixel of the screen array.
    pub top_left: Point,
    /// Bottom right corner pixel of the screen array.
    pub bottom_right: Point,
}
/// Information about a monitor/screen layout.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScreenSpace {
    /// The raw pixel range of the monitor layout.
    /// (top_left_corner, bottom_right_corner)
    range: Range,
    /// List of monitors sorted top to bottom, left to right.
    monitors: Vec<ScreenInfo>,
}
impl ScreenSpace {
    pub fn from_monitors(monitors: &[ScreenInfo]) -> Self {
        let mut range = Range {
            top_left: Default::default(),
            bottom_right: Default::default(),
        };
        for monitor in monitors {
            let o = monitor.origin;
            range.top_left.x = range.top_left.x.min(o.x);
            range.top_left.y = range.top_left.y.min(o.y);
            range.bottom_right.x = range.bottom_right.x.max(o.x + monitor.size.width as i32);
            range.bottom_right.y = range.bottom_right.y.max(o.y + monitor.size.height as i32);
        }
        // If the screen has width W, then the last pixel is at position W - 1.
        range.bottom_right.x -= 1;
        range.bottom_right.y -= 1;

        let mut sort_monitors: Vec<ScreenInfo> = monitors.to_vec();
        // Ensure order of monitors is top to bottom followed by left to right.
        sort_monitors.sort_by(|a, b| {
            let ao = a.origin;
            let bo = b.origin;
            (ao.y, ao.x).cmp(&(bo.y, bo.x))
        });
        ScreenSpace {
            range,
            monitors: sort_monitors,
        }
    }

    pub fn range(&self) -> Range {
        self.range
    }
    pub fn monitors(&self) -> &[ScreenInfo] {
        &self.monitors
    }
}

#[derive(Debug)]
/// Requests for the compositor's client.
enum CompReq {
    /// List of outputs.
    ScreenInfo(Sender<Vec<ScreenInfo>>),
    /// The absolute cursor position. Fails if the user is currently holding doing the cursor.
    /// Also, for the brief window the shell layer is up, mouse is captured and won't send clicks to
    /// other windows.
    AbsCursorPos(Sender<Result<Point, CursorFetchErr>>),
}
#[derive(Debug)]
/// Represents a failed attempt at capturing the cursor. Given that there is no way to actually get
/// why the cursor wasn't captured, the best way to give info about the error is to put some context
/// around it.
pub struct CursorFetchErr(Vec<CursorFetchErrCtx>);
impl Display for CursorFetchErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            write!(
                f,
                "Cursor fetch error. No extra context available, user probably held down the cursor during fetch."
            )
        } else {
            write!(f, "Cursor fetch error with context: {:?}", self)
        }
    }
}

#[derive(Debug)]
/// Context surrounding the cursor capture error.
enum CursorFetchErrCtx {
    /// The layers were not all ready when the cursor fetch was attempted. Could be that the reason
    /// why it failed was that the user was holding down click while some other layer was not ready,
    /// but as stated before, there is no absolute way to know what went wrong and this is only an approximation.
    LayerNotReady,
}
#[derive(Clone)]
pub(crate) struct CompositorInterface {
    sender: calloop::channel::Sender<CompReq>,
    // If termination sync is needed, return Arc<CompClientInterface> isntead and add the join
    // handle to the fields with a drop implementation to join on it. Also make both fields options
    // to allow take().
}
/// Cloneable interface to talk to the compositor client.
impl CompositorInterface {
    /// Create a new connection to the compositor that runs on its own thread and return its interface.
    pub fn init() -> CompositorInterface {
        let (sender, receiver) = calloop::channel::channel::<CompReq>();
        thread::spawn(|| {
            if let Err(e) = CompositorInterface::run(receiver) {
                eprintln!("wayland connection failed: {e}");
            }
        });
        CompositorInterface { sender }
    }
    fn run(receiver: calloop::channel::Channel<CompReq>) -> Result<(), Box<dyn Error>> {
        let (mut cmp_client, event_queue, mut event_loop) = CompositorClient::setup_client()?;
        let conn = cmp_client.cmp_ctx.connection.clone();
        let qh = event_queue.handle();
        let lh = event_loop.handle();
        let s = WaylandSource::new(conn.clone(), event_queue);
        s.insert(lh.clone())?;
        lh.insert_source(receiver, move |event, _, cmp_client| {
            let r = || -> Result<(), Box<dyn Error>> {
                match event {
                    calloop::channel::Event::Msg(msg) => {
                        match &msg {
                            CompReq::AbsCursorPos(sender) => {
                                cmp_client.set_shell_layer_activation(true);
                                // Force focus on the newly created layer to get the cursor enter event.
                                cmp_client.cmp_ctx.trigger_mouse();

                                cmp_client
                                    .cmp_ctx
                                    .connection
                                    .display()
                                    .sync(&qh, CallbackReason::SyncAbsMousePos(sender.clone()));
                            }
                            CompReq::ScreenInfo(sender) => {
                                let mut screen_info = Vec::new();
                                for shell_layer in &cmp_client.shell_layers {
                                    screen_info.push(shell_layer.screen_info);
                                }
                                sender.send(screen_info)?;
                            }
                        }
                    }
                    calloop::channel::Event::Closed => {
                        return Err(Box::<dyn Error>::from(format!(
                            "failed to receive message, channel closed"
                        )));
                    }
                }
                Ok(())
            }();
            if let Err(e) = r {
                eprintln!("Failed to execute request: {e}");
            }
        })?;
        loop {
            event_loop.dispatch(None, &mut cmp_client)?;
        }
    }
    /// Request the absolute position of the cursor.
    pub fn req_abs_cursor_pos(&self) -> Result<Point, CursorFetchErr> {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.sender
            .send(CompReq::AbsCursorPos(sender))
            .expect("should be able to send");
        receiver.recv().expect("should be able to receive")
    }
    /// Request information about the screens.
    pub fn req_screen_info(&self) -> Vec<ScreenInfo> {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.sender
            .send(CompReq::ScreenInfo(sender))
            .expect("should be able to send");
        receiver.recv().expect("should be able to receive")
    }
}
