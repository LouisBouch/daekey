//!Holds required methods and structs to detect and define the monitors in use and interfaces with the display outputs.

use std::error::Error;

use serde::{Deserialize, Serialize};
use smithay_client_toolkit::{
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{
    Connection, QueueHandle, globals::registry_queue_init, protocol::wl_output::WlOutput,
};

use crate::Pixel;

/// Obtain the [`ListOutputs`] from Wayland.
pub(crate) fn get_list_outputs() -> Result<ListOutputs, Box<dyn Error>> {
    // Try to connect to the Wayland server.
    let conn = Connection::connect_to_env()?;

    // Now create an event queue and a handle to the queue so we can create objects.
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    // Initialize the registry handling so other parts of Smithay's client toolkit may bind
    // globals.
    let registry_state = RegistryState::new(&globals);

    // Initialize the delegate we will use for outputs.
    let output_delegate = OutputState::new(&globals, &qh);

    // Set up application state.
    //
    // This is where you will store your delegates and any data you wish to access/mutate while the
    // application is running.
    let mut list_outputs = ListOutputs {
        registry_state,
        output_state: output_delegate,
    };

    // `OutputState::new()` binds the output globals found in `registry_queue_init()`.
    //
    // After the globals are bound, we need to dispatch again so that events may be sent to the newly
    // created objects.
    event_queue.roundtrip(&mut list_outputs)?;
    Ok(list_outputs)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Positional information for a screen.
pub struct ScreenInfo {
    /// Width of the screen in pixels.
    width: Pixel,
    /// Height of the screen in pixels.
    height: Pixel,
    /// Position of the origin relative ot the other monitors.
    origin: Point,
}
impl ScreenInfo {
    pub fn width(&self) -> Pixel {
        self.width
    }
    pub fn height(&self) -> Pixel {
        self.height
    }
    pub fn origin(&self) -> Point {
        self.origin
    }
}

#[derive(Debug, Clone)]
pub struct Screen {
    /// The output that represents it.
    output: WlOutput,
    /// Info about the monitor/screen.
    screen_info: ScreenInfo,
}
impl Screen {
    pub fn output(&self) -> &WlOutput {
        &self.output
    }
    pub fn screen_info(&self) -> &ScreenInfo {
        &self.screen_info
    }
}
/// (x,y) coordinates
pub type Point = [Pixel; 2];
#[derive(Serialize, Deserialize, Debug, Clone)]
/// Information about a monitor/screen layout.
pub struct ScreenSpace {
    /// The raw pixel range of the monitor layout.
    /// (top_left_corner, bottom_right_corner)
    range: (Point, Point),
    /// List of monitors sorted top to bottom, left to right.
    monitors: Vec<ScreenInfo>,
}
impl ScreenSpace {
    pub fn from_monitors(monitors: &[ScreenInfo]) -> Self {
        let mut range: (Point, Point) = ([0, 0], [0, 0]);
        for monitor in monitors {
            let o = monitor.origin;
            range.0[0] = range.0[0].min(o[0]);
            range.0[1] = range.0[1].min(o[1]);
            range.1[0] = range.1[0].max(o[0] + monitor.width);
            range.1[1] = range.1[1].max(o[1] + monitor.height);
        }
        // If the screen has width W, then the last pixel is at position W - 1, this is why 1 is subtracted.
        range.1[0] -= 1;
        range.1[1] -= 1;

        let mut sort_monitors: Vec<ScreenInfo> = monitors.to_vec();
        // Ensure order of monitors is top to bottom followed by left to right.
        sort_monitors.sort_by(|a, b| {
            let ao = a.origin();
            let bo = b.origin();
            (ao[1], ao[0]).cmp(&(bo[1], bo[0]))
        });
        ScreenSpace {
            range,
            monitors: sort_monitors,
        }
    }

    pub fn range(&self) -> (Point, Point) {
        self.range
    }
    pub fn monitors(&self) -> &[ScreenInfo] {
        &self.monitors
    }
}
/// Application data.
///
/// This type is where the delegates for some parts of the protocol and any application specific data will
/// live.
pub(crate) struct ListOutputs {
    registry_state: RegistryState,
    output_state: OutputState,
}
impl ListOutputs {
    // Returns a list of screens sorted by their y position followed by their x position.
    pub fn get_screens(&self) -> Result<Vec<Screen>, Box<dyn Error>> {
        let mut screens = Vec::new();
        for output in self.output_state.outputs() {
            let info = &self
                .output_state
                .info(&output)
                .ok_or_else(|| "output has no info".to_owned())?;
            let location = match info.logical_position {
                Some(l) => l,
                None => {
                    return Err(Box::from(format!(
                        "monitor {info:?} has no logical position"
                    )));
                }
            };

            let (width, height) = match info.logical_size {
                Some(v) => v,
                None => return Err(Box::from(format!("monitor {info:?} has no logical size"))),
            };
            let monitor_info = ScreenInfo {
                width,
                height,
                origin: [location.0, location.1],
            };
            screens.push(Screen {
                screen_info: monitor_info,
                output: output.clone(),
            });
        }
        let mut increasing: Vec<usize> = (0..screens.len()).collect();
        increasing.sort_by_key(|&i| {
            let o = screens[i].screen_info.origin;
            (o[1], o[0])
        });
        Ok(increasing.iter().map(|&i| screens[i].clone()).collect())
    }
}

// In order to use OutputDelegate, we must implement this trait to indicate when something has happened to an
// output and to provide an instance of the output state to the delegate when dispatching events.
impl OutputHandler for ListOutputs {
    // First we need to provide a way to access the delegate.
    //
    // This is needed because delegate implementations for handling events use the application data type in
    // their function signatures. This allows the implementation to access an instance of the type.
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    // Then there exist these functions that indicate the lifecycle of an output.
    // These will be called as appropriate by the delegate implementation.

    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
    }
}

// In order for our delegate to know of the existence of globals, we need to implement registry
// handling for the program. This trait will forward events to the RegistryHandler trait
// implementations.
delegate_registry!(ListOutputs);
smithay_client_toolkit::delegate_dispatch2!(ListOutputs);

// In order for delegate_registry to work, our application data type needs to provide a way for the
// implementation to access the registry state.
//
// We also need to indicate which delegates will get told about globals being created. We specify
// the types of the delegates inside the array.
impl ProvidesRegistryState for ListOutputs {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers! {
        // Here we specify that OutputState needs to receive events regarding the creation and destruction of
        // globals.
        OutputState,
    }
}
