//! Holds required methods and structs to detect and define the monitors in use.

use std::error::Error;

use serde::{Deserialize, Serialize};
use smithay_client_toolkit::{
    delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{Connection, QueueHandle, globals::registry_queue_init, protocol::wl_output};

pub type Pixel = i32;
/// (x,y) coordinates
pub type Point = [Pixel; 2];
#[derive(Serialize, Deserialize, Debug, Clone)]
/// Information about a monitor layout.
pub struct ScreenSpace {
    /// The raw pixel range of the monitor layout.
    /// (top_left_corner, bottom_right_corner)
    range: (Point, Point),
    /// List of monitors sorted top to bottom, left to right.
    monitors: Vec<MonitorInfo>,
}
impl ScreenSpace {
    pub fn new(monitors: &[MonitorInfo]) -> Self {
        let mut range = ([0, 0], [0, 0]);
        let mut sort_monitors: Vec<MonitorInfo> = Vec::new();
        // Ensure order of monitors is top to bottom followed by left to right.
        for monitor in monitors {
            let o = monitor.origin;
            range.0[0] = range.0[0].min(o[0]);
            range.0[1] = range.0[1].min(o[1]);
            range.1[0] = range.1[0].max(o[0] + monitor.width);
            range.1[1] = range.1[1].max(o[1] + monitor.height);
            let mut placed = false;
            for sort_mon_i in 0..sort_monitors.len() {
                let sort_m = &sort_monitors[sort_mon_i];
                if o[1] < sort_m.origin[1] || (o[1] == sort_m.origin[1] && o[0] < sort_m.origin[0])
                {
                    let mut place_next = monitor.clone();
                    for i in sort_mon_i..sort_monitors.len() {
                        let temp = sort_monitors[i].clone();
                        sort_monitors[i] = place_next;
                        place_next = temp;
                    }
                    sort_monitors.push(place_next);
                    placed = true;
                    break;
                }
            }
            if !placed {
                sort_monitors.push(monitor.clone());
            }
        }
        ScreenSpace {
            range,
            monitors: sort_monitors,
        }
    }
    pub fn range(&self) -> (Point, Point) {
        self.range
    }
    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
/// Positional information for a monitor.
pub struct MonitorInfo {
    /// Width of the monitor in pixels.
    width: Pixel,
    /// Height of the monitor in pixels.
    height: Pixel,
    /// Position of the origin relative ot the other monitors.
    origin: Point,
}
impl MonitorInfo {
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

pub fn get_monitor_info() -> Result<ScreenSpace, Box<dyn Error>> {
    // Try to connect to the Wayland server.
    let conn = Connection::connect_to_env()?;

    // Now create an event queue and a handle to the queue so we can create objects.
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
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

    // Now our outputs have been initialized with data, we may access what outputs exist and information about
    // said outputs using the output delegate.
    let mut monitors = Vec::new();
    for output in list_outputs.output_state.outputs() {
        let info = &list_outputs
            .output_state
            .info(&output)
            .ok_or_else(|| "output has no info".to_owned())?;
        let location = info.location;

        let (width, height) = match info.logical_size {
            Some(v) => v,
            None => return Err(Box::from(format!("monitor {info:?} has no logical size"))),
        };
        monitors.push(MonitorInfo {
            width,
            height,
            origin: [location.0, location.1],
        });
    }

    Ok(ScreenSpace::new(&monitors))
}

/// Application data.
///
/// This type is where the delegates for some parts of the protocol and any application specific data will
/// live.
struct ListOutputs {
    registry_state: RegistryState,
    output_state: OutputState,
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

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

// Now we need to say we are delegating the responsibility of output related events for our application data
// type to the requisite delegate.
delegate_output!(ListOutputs);

// In order for our delegate to know of the existence of globals, we need to implement registry
// handling for the program. This trait will forward events to the RegistryHandler trait
// implementations.
delegate_registry!(ListOutputs);

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
