//! List of structs and methods to allow the user to interface with the wayland compositor.
use std::error::Error;
use std::fmt::Display;
use std::num::NonZeroU32;
use std::thread;
use std::time::Duration;

use crossbeam_channel::Sender;
use smithay_client_toolkit::reexports::calloop::{self, EventLoop};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{Shm, ShmHandler},
};
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_region, wl_seat, wl_surface},
};
use wayland_client::{Dispatch, EventQueue};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

use crate::compositor_interface::ScreenInfo;
use crate::compositor_interface::shell_layers::ShellLayer;
use crate::compositor_interface::compositor_context::CmpCtx;
#[derive(Debug)]
enum Answer {
    AbsCursorPos(Point),
}
/// The state of the shell layers.
#[derive(Debug, PartialEq)]
enum LayersState {
    /// There are no layers yet.
    Empty,
    /// Initialization has been committed and is waiting for configure event.
    AwaitingConf,
    /// Configuration has been committed and is waiting for sync update.
    AwaitingSync,
    /// Wayland has finished configuring the layers and replied with a Done event.
    Ready,
}
#[derive(Debug)]
pub(crate) struct CmpClient {
    compositor_context: CmpCtx,

    /// The list of shell layers that cover the screen outputs.
    shell_layers: Vec<ShellLayer>,
    /// State of the layers.
    layers_state: LayersState,

    answer: Option<Answer>,
}
impl CmpClient {
    fn setup_client()
    -> Result<(CmpClient, EventQueue<CmpClient>, EventLoop<'static, Self>), Box<dyn Error>> {
        // Try to connect to the Wayland server.
        let conn = Connection::connect_to_env()?;

        // Now create an event queue and a handle to the queue so we can create objects.
        let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
        let qh: QueueHandle<CmpClient> = event_queue.handle();

        // Since we are not using the GPU in this example, we use wl_shm to allow software rendering to a buffer
        // we share with the compositor process.
        let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");
        let p_man: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=2, ())?;
        let v_pointer = p_man.create_virtual_pointer(None, &qh, ());

        // Set up application state.
        //
        // This is where you will store your delegates and any data you wish to access/mutate while the
        // application is running.
        let cmp_ctx = CmpCtx {
            // Initialize the registry handling so other parts of Smithay's client toolkit may bind
            // globals.
            registry_state: RegistryState::new(&globals),
            connection: conn.clone(),
            // Seats and outputs may be hotplugged at runtime, therefore we need to setup a registry state to
            // listen for seats and outputs.
            seat_state: SeatState::new(&globals, &qh),
            // Initialize the delegate we will use for outputs.
            output_state: OutputState::new(&globals, &qh),
            compositor_state: CompositorState::bind(&globals, &qh)?,
            layer_shell: LayerShell::bind(&globals, &qh)?,
            shm,
            pointer: None,
            v_pointer,
        };
        let event_loop = EventLoop::try_new()?;
        let mut comp_client_state = CmpClient {
            compositor_context: cmp_ctx,
            shell_layers: Vec::new(),
            answer: None,
            layers_state: LayersState::Empty,
        };
        event_queue.roundtrip(&mut comp_client_state)?;
        Ok((comp_client_state, event_queue, event_loop))
    }
    fn initialize_layers(&mut self, qh: &QueueHandle<CmpClient>) -> Result<(), Box<dyn Error>> {
        // Clear old layers.
        self.layers_state = LayersState::Empty;
        self.shell_layers.clear();

        let outputs: Vec<WlOutput> = self.compositor_context.output_state.outputs().collect();
        for output in outputs.iter() {
            let shell_layer = ShellLayer::from_output(&output, &self.compositor_context, &qh)?;
            self.shell_layers.push(shell_layer);
        }
        self.layers_state = LayersState::AwaitingConf;
        // Sorts outputs by their y position followed by their x position.
        self.shell_layers
            .sort_by_key(|v| (v.screen_info.origin.y, v.screen_info.origin.x));
        Ok(())
    }
    fn set_shell_layer_activation(&mut self, activated: bool) {
        if !activated {
            for layer in &self.shell_layers {
                layer
                    .layer_surface
                    .wl_surface()
                    .set_input_region(Some(&layer.empty_region));
                layer.layer_surface.commit();
            }
        } else {
            for layer in &self.shell_layers {
                layer.layer_surface.wl_surface().set_input_region(None);
                layer.layer_surface.commit();
            }
        }
    }
}
// Used to allow region creation.
impl Dispatch<wl_region::WlRegion, ()> for CmpClient {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<CmpClient>,
    ) {
    }
}
// Used to allow creation of virtual cursor.
impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for CmpClient {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerManagerV1,
        _: <ZwlrVirtualPointerManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ZwlrVirtualPointerV1, ()> for CmpClient {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerV1,
        _: <ZwlrVirtualPointerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
enum CallbackReason {
    SyncAbsMousePos(Sender<Result<Point, CursorFetchErr>>),
    LayersConfigured,
}
impl Dispatch<WlCallback, CallbackReason> for CmpClient {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        data: &CallbackReason,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let wayland_client::protocol::wl_callback::Event::Done { .. } = event else {
            eprintln!("Did not receive Done event from wayland after sync.");
            return;
        };
        match data {
            CallbackReason::SyncAbsMousePos(sender) => {
                // Send the deactivation request to the compositor.
                if let Some(Answer::AbsCursorPos(point)) = state.answer.take() {
                    sender.send(Ok(point)).unwrap();
                } else {
                    // No answer means it was not deactivated, so do it now.
                    state.set_shell_layer_activation(false);
                    // Force focus back to window that was under the layer.
                    state.compositor_context.trigger_mouse();
                    let err = if state.layers_state != LayersState::Ready {
                        Err(CursorFetchErr::LayerNotReady)
                    } else {
                        Err(CursorFetchErr::CursorCaptureFailed)
                    };
                    sender.send(err).unwrap();
                }
            }
            CallbackReason::LayersConfigured => {
                state.layers_state = LayersState::Ready;
                println!("confed");
            }
        }
    }
}

// In order to use OutputDelegate, we must implement this trait to indicate when something has happened to an
// output and to provide an instance of the output state to the delegate when dispatching events.
impl OutputHandler for CmpClient {
    // First we need to provide a way to access the delegate.
    //
    // This is needed because delegate implementations for handling events use the application data type in
    // their function signatures. This allows the implementation to access an instance of the type.
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.compositor_context.output_state
    }

    // Then there exist these functions that indicate the lifecycle of an output.
    // These will be called as appropriate by the delegate implementation.
    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        println!("new");
        if let Err(e) = self.initialize_layers(&qh) {
            eprintln!("Could not initialize layers: {e:?}");
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        println!("update");
        if let Err(e) = self.initialize_layers(&qh) {
            eprintln!("Could not initialize layers: {e:?}");
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        println!("destroy");
        if let Err(e) = self.initialize_layers(&qh) {
            eprintln!("Could not initialize layers: {e:?}");
        }
    }
}

smithay_client_toolkit::delegate_dispatch2!(CmpClient);

impl PointerHandler for CmpClient {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            // Ignore events for other surfaces
            let Some(shell_layer) = self
                .shell_layers
                .iter()
                .find(|v| v.layer_surface.wl_surface() == &event.surface)
            else {
                continue;
            };
            match event.kind {
                Enter { .. } => {
                    let o = &shell_layer.screen_info.origin;
                    let pos = Point {
                        x: event.position.0 as i32 + o.x,
                        y: event.position.1 as i32 + o.y,
                    };
                    self.answer = Some(Answer::AbsCursorPos(pos));

                    self.set_shell_layer_activation(false);
                    // Force focus back to window that was under the layer.
                    self.compositor_context.trigger_mouse();
                }
                Leave { .. } => {
                    println!(
                        "Pointer left @{:?} in layer at position {:?} and size {:?}",
                        event.position,
                        shell_layer.screen_info.origin,
                        shell_layer.screen_info.size,
                    );
                }
                _ => (),
            }
        }
    }
}

impl SeatHandler for CmpClient {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.compositor_context.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.compositor_context.pointer.is_none() {
            println!("Set pointer capability");
            let pointer = self
                .compositor_context
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.compositor_context.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.compositor_context.pointer.is_some() {
            println!("Unset pointer capability");
            self.compositor_context.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

// In order for delegate_registry to work, our application data type needs to provide a way for the
// implementation to access the registry state.
//
// We also need to indicate which delegates will get told about globals being created. We specify
// the types of the delegates inside the array.
impl ProvidesRegistryState for CmpClient {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.compositor_context.registry_state
    }

    registry_handlers! {
        // Here we specify that OutputState needs to receive events regarding the creation and destruction of
        // globals.
        OutputState,
        SeatState
    }
}

// In order for our delegate to know of the existence of globals, we need to implement registry
// handling for the program. This trait will forward events to the RegistryHandler trait
// implementations.
delegate_registry!(CmpClient);

impl ShmHandler for CmpClient {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.compositor_context.shm
    }
}
impl CompositorHandler for CmpClient {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed for this example.
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed for this example.
    }
}
impl LayerShellHandler for CmpClient {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(shell_layer) = self
            .shell_layers
            .iter_mut()
            .find(|v| &v.layer_surface == layer)
        else {
            return;
        };
        let new_width = NonZeroU32::new(configure.new_size.0).map_or(256, NonZeroU32::get);
        let new_height = NonZeroU32::new(configure.new_size.1).map_or(256, NonZeroU32::get);
        let dim_changed = (shell_layer.screen_info.size.width != new_width)
            || (shell_layer.screen_info.size.height != new_height);

        // Initiate the draw only when dimensions changed or if it has never been configured.
        // It only checks for dimension because draw_shell only uses the dimensions.
        if shell_layer.first_configure {
            shell_layer.first_configure = false;
            shell_layer.screen_info.size.width = new_width;
            shell_layer.screen_info.size.height = new_height;
            shell_layer.draw_shell();
            if self.shell_layers.iter().all(|s| !s.first_configure) {
                self.layers_state = LayersState::AwaitingSync;
                self.compositor_context
                    .connection
                    .display()
                    .sync(&qh, CallbackReason::LayersConfigured);
            }
        } else if dim_changed {
            shell_layer.screen_info.size.width = new_width;
            shell_layer.screen_info.size.height = new_height;
            shell_layer.draw_shell();
        }
    }
}
pub type Pixel = i32;
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}
#[derive(Debug)]
/// Requests for the compositor's client.
enum CompClientReq {
    /// List of outputs.
    ScreenInfo(Sender<Vec<ScreenInfo>>),
    /// The absolute cursor position. Fails if the user is currently holding doing the cursor.
    /// Also, for the brief window the shell layer is up, mouse is captured and won't send clicks to
    /// other windows.
    AbsCursorPos(Sender<Result<Point, CursorFetchErr>>),
}
#[derive(Debug)]
/// Best effort error when the cursor failed to be captured.
/// Given that there is no absolute way to know why a capture failed, the best we can do is
/// approximate why it failed.
enum CursorFetchErr {
    /// The shell layer failed to capture the mouse. This is most likely caused by the user holding
    /// down the click button when the fetch is requested.
    CursorCaptureFailed,
    /// The layers were not all ready when the cursor fetch was attempted. Could be that the reason
    /// why it failed was that the user was holding down click while some other layer was not ready,
    /// but as stated before, there is no absolute way to know what went wrong and this is only an approximation.
    LayerNotReady,
}
impl Display for CursorFetchErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
#[derive(Clone)]
pub(crate) struct CompClientInterface {
    sender: calloop::channel::Sender<CompClientReq>,
    // If termination sync is needed, return Arc<CompClientInterface> isntead and add the join
    // handle to the fields with a drop implementation to join on it. Also make both fields options
    // to allow take().
}
/// Cloneable interface to talk to the compositor client.
impl CompClientInterface {
    /// Create a new connection to the compositor that runs on its own thread and return its interface.
    pub fn init() -> CompClientInterface {
        let (sender, receiver) = calloop::channel::channel::<CompClientReq>();
        thread::spawn(|| {
            if let Err(e) = CompClientInterface::run(receiver) {
                eprintln!("wayland connection failed: {e}");
            }
        });
        CompClientInterface { sender }
    }
    fn run(receiver: calloop::channel::Channel<CompClientReq>) -> Result<(), Box<dyn Error>> {
        let (mut cmp_client, event_queue, mut event_loop) = CmpClient::setup_client()?;
        let conn = cmp_client.compositor_context.connection.clone();
        let qh = event_queue.handle();
        let lh = event_loop.handle();
        let s = WaylandSource::new(conn.clone(), event_queue);
        s.insert(lh.clone())?;
        lh.insert_source(receiver, move |event, _, cmp_client| {
            let r = || -> Result<(), Box<dyn Error>> {
                match event {
                    calloop::channel::Event::Msg(msg) => {
                        match &msg {
                            CompClientReq::AbsCursorPos(sender) => {
                                cmp_client.set_shell_layer_activation(true);
                                // Force focus on the newly created layer to get the cursor enter event.
                                cmp_client.compositor_context.trigger_mouse();

                                cmp_client
                                    .compositor_context
                                    .connection
                                    .display()
                                    .sync(&qh, CallbackReason::SyncAbsMousePos(sender.clone()));
                            }
                            CompClientReq::ScreenInfo(sender) => {
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
    pub fn req_abs_cursor_pos(&self) -> Result<Point, CursorFetchErr> {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.sender
            .send(CompClientReq::AbsCursorPos(sender))
            .expect("should be able to send");
        receiver.recv().expect("should be able to receive")
    }
    pub fn req_screen_info(&self) -> Vec<ScreenInfo> {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.sender
            .send(CompClientReq::ScreenInfo(sender))
            .expect("should be able to send");
        receiver.recv().expect("should be able to receive")
    }
}
fn main() {
    let c = CompClientInterface::init();
    println!("screen info: {:?}", c.req_screen_info());
    loop {
        match c.req_abs_cursor_pos() {
            Ok(pos) => println!("pos is: {pos:?}"),
            Err(e) => eprintln!("coudlnt get pos: {e}"),
        }
        thread::sleep(Duration::from_secs_f64(0.5));
    }
}
