//! Functions and structs to represent a client of the compositor.

use std::error::Error;
use std::num::NonZeroU32;

use crossbeam_channel::Sender;
use smithay_client_toolkit::reexports::calloop::EventLoop;
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

use crate::compositor_interface::compositor_context::CompositorContext;
use crate::compositor_interface::shell_layers::ShellLayer;
use crate::compositor_interface::{CursorFetchErr, CursorFetchErrCtx, Point, ScreenInfo};

#[derive(Debug)]
/// Describe a change in the compositor.
pub(crate) enum CompUpdate {
    /// The layout of the output screens changed.
    ScreenLayoutChanged(Vec<ScreenInfo>),
}

#[derive(Debug)]
pub(crate) enum Answer {
    AbsCursorPos(Point),
}
/// The state of the shell layers.
#[derive(Debug, PartialEq)]
pub(crate) enum LayersState {
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
/// The struct that acts as the client to the compositor.
pub(crate) struct CompositorClient {
    pub(crate) cmp_ctx: CompositorContext,

    // TODO: Maybe extract screen info into its own struct instead of relying on shell_layers.
    // This will allow non shell layer compatible compositor to still ahve access to the function.
    /// The list of shell layers that cover the screen outputs.
    pub(crate) shell_layers: Vec<ShellLayer>,
    /// State of the layers.
    layers_state: LayersState,

    /// Answer from different modules of the client.
    answer: Option<Answer>,

    /// Channel to communicate back to the core.
    update_channel: Sender<CompUpdate>,
}
impl CompositorClient {
    pub(crate) fn setup_client(
        update_sender: Sender<CompUpdate>,
    ) -> Result<
        (
            CompositorClient,
            EventQueue<CompositorClient>,
            EventLoop<'static, Self>,
        ),
        Box<dyn Error>,
    > {
        // Try to connect to the Wayland server.
        let conn = Connection::connect_to_env()?;

        // Now create an event queue and a handle to the queue so we can create objects.
        let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
        let qh: QueueHandle<CompositorClient> = event_queue.handle();

        // Since we are not using the GPU, we use wl_shm to allow software rendering to a buffer
        // we share with the compositor process.
        let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");
        let p_man: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=2, ())?;
        let v_pointer = p_man.create_virtual_pointer(None, &qh, ());

        // Set up application state.
        //
        // This is where you will store your delegates and any data you wish to access/mutate while the
        // application is running.
        let cmp_ctx = CompositorContext {
            // Initialize the registry handling so other parts of Smithay's client toolkit may bind
            // globals.
            registry_state: RegistryState::new(&globals),
            connection: conn.clone(),
            // Seats and outputs may be hotplugged at runtime, therefore we need to setup a registry state to
            // listen for seats and outputs.
            seat_state: SeatState::new(&globals, &qh),
            // Initialize the delegate we will use for outputs.
            output_state: OutputState::new(&globals, &qh),
            cmp_state: CompositorState::bind(&globals, &qh)?,
            layer_shell: LayerShell::bind(&globals, &qh)?,
            shm,
            pointer: None,
            v_pointer,
        };
        let event_loop = EventLoop::try_new()?;
        let mut cmp_client = CompositorClient {
            cmp_ctx,
            shell_layers: Vec::new(),
            answer: None,
            layers_state: LayersState::Empty,
            update_channel: update_sender,
        };
        event_queue.roundtrip(&mut cmp_client)?;
        Ok((cmp_client, event_queue, event_loop))
    }
    fn initialize_layers(
        &mut self,
        qh: &QueueHandle<CompositorClient>,
    ) -> Result<(), Box<dyn Error>> {
        // Clear old layers.
        self.layers_state = LayersState::Empty;
        self.shell_layers.clear();

        let outputs: Vec<WlOutput> = self.cmp_ctx.output_state.outputs().collect();
        for output in outputs.iter() {
            let shell_layer = ShellLayer::from_output(&output, &self.cmp_ctx, qh)?;
            self.shell_layers.push(shell_layer);
        }
        self.layers_state = LayersState::AwaitingConf;
        // Sorts outputs by their y position followed by their x position.
        self.shell_layers
            .sort_by_key(|v| (v.screen_info.origin.y, v.screen_info.origin.x));
        Ok(())
    }
    pub(crate) fn set_shell_layer_activation(&mut self, activated: bool) {
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
    /// Get the screen info of the compositor.
    pub(crate) fn screen_info(&self) -> Vec<ScreenInfo> {
        let mut screen_info = Vec::new();
        for shell_layer in &self.shell_layers {
            screen_info.push(shell_layer.screen_info);
        }
        screen_info
    }
}
// Used to allow region creation.
impl Dispatch<wl_region::WlRegion, ()> for CompositorClient {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<CompositorClient>,
    ) {
    }
}
// Used to allow creation of virtual cursor.
impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for CompositorClient {
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
impl Dispatch<ZwlrVirtualPointerV1, ()> for CompositorClient {
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
pub(crate) enum CallbackReason {
    SyncAbsMousePos(Sender<Result<Point, CursorFetchErr>>),
    LayersConfigured,
}
impl Dispatch<WlCallback, CallbackReason> for CompositorClient {
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
                    state.cmp_ctx.trigger_mouse();
                    let err = if state.layers_state != LayersState::Ready {
                        Err(CursorFetchErr(vec![CursorFetchErrCtx::LayerNotReady]))
                    } else {
                        Err(CursorFetchErr(Vec::new()))
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
// TODO: Add back the input device listeners and notify input of it.

// In order to use OutputDelegate, we must implement this trait to indicate when something has happened to an
// output and to provide an instance of the output state to the delegate when dispatching events.
impl OutputHandler for CompositorClient {
    // First we need to provide a way to access the delegate.
    //
    // This is needed because delegate implementations for handling events use the application data type in
    // their function signatures. This allows the implementation to access an instance of the type.
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.cmp_ctx.output_state
    }

    // Then there exist these functions that indicate the lifecycle of an output.
    // These will be called as appropriate by the delegate implementation.
    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        if let Err(e) = self.initialize_layers(&qh) {
            eprintln!("Could not initialize layers: {e}");
        }
        if let Err(e) = self
            .update_channel
            .send(CompUpdate::ScreenLayoutChanged(self.screen_info()))
        {
            eprintln!("Failed to send updated screen info: {e}");
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        if let Err(e) = self.initialize_layers(&qh) {
            eprintln!("Could not initialize layers: {e}");
        }
        if let Err(e) = self
            .update_channel
            .send(CompUpdate::ScreenLayoutChanged(self.screen_info()))
        {
            eprintln!("Failed to send updated screen info: {e}");
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        if let Err(e) = self.initialize_layers(&qh) {
            eprintln!("Could not initialize layers: {e}");
        }
        if let Err(e) = self
            .update_channel
            .send(CompUpdate::ScreenLayoutChanged(self.screen_info()))
        {
            eprintln!("Failed to send updated screen info: {e}");
        }
    }
}

smithay_client_toolkit::delegate_dispatch2!(CompositorClient);

impl PointerHandler for CompositorClient {
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
                    self.cmp_ctx.trigger_mouse();
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

impl SeatHandler for CompositorClient {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.cmp_ctx.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.cmp_ctx.pointer.is_none() {
            println!("Set pointer capability");
            let pointer = self
                .cmp_ctx
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.cmp_ctx.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.cmp_ctx.pointer.is_some() {
            println!("Unset pointer capability");
            self.cmp_ctx.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

// In order for delegate_registry to work, our application data type needs to provide a way for the
// implementation to access the registry state.
//
// We also need to indicate which delegates will get told about globals being created. We specify
// the types of the delegates inside the array.
impl ProvidesRegistryState for CompositorClient {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.cmp_ctx.registry_state
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
delegate_registry!(CompositorClient);

impl ShmHandler for CompositorClient {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.cmp_ctx.shm
    }
}
impl CompositorHandler for CompositorClient {
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
impl LayerShellHandler for CompositorClient {
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
                self.cmp_ctx
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
