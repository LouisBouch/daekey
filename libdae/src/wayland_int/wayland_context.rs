use serde::{Deserialize, Serialize};
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState, seat::SeatState,
    shell::wlr_layer::LayerShell, shm::Shm,
};
use wayland_client::{Connection, protocol::wl_pointer};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

use crate::wayland_int::{Point, ScreenInfo};

#[derive(Debug)]
pub(crate) struct WlCtx {
    pub(crate) registry_state: RegistryState,
    pub(crate) connection: Connection,
    pub(crate) seat_state: SeatState,
    pub(crate) output_state: OutputState,
    pub(crate) compositor_state: CompositorState,
    pub(crate) layer_shell: LayerShell,
    pub(crate) shm: Shm,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) v_pointer: ZwlrVirtualPointerV1,
}

impl WlCtx {
    pub(crate) fn trigger_mouse(&self) {
        self.v_pointer.motion(0, 0.0, 0.0);
        self.v_pointer.frame();
    }
}
