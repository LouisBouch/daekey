use std::convert::TryInto;
use std::error::Error;

use smithay_client_toolkit::{
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, Layer, LayerSurface},
    },
    shm::slot::SlotPool,
};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::{QueueHandle, protocol::wl_shm};

use crate::compositor_interface::{ScreenInfo, CompositorClient};
use crate::compositor_interface::compositor_context::CompositorContext;

#[derive(Debug)]
pub(crate) struct ShellLayer {
    pub(crate) screen_info: ScreenInfo,
    pub(crate) pool: SlotPool,
    pub(crate) layer_surface: LayerSurface,
    pub(crate) empty_region: WlRegion,
    pub(crate) first_configure: bool,
}
impl ShellLayer {
    pub(crate) fn from_output(
        output: &WlOutput,
        cmp_ctx: &CompositorContext,
        qh: &QueueHandle<CompositorClient>,
    ) -> Result<Self, Box<dyn Error>> {
        let info = cmp_ctx
            .output_state
            .info(output)
            .ok_or(Box::<dyn Error>::from(format!(
                "No info available for output: {output:?}"
            )))?;
        match info.logical_position {
            Some(v) => {
                println!("name: {:?}", info.name);
                dbg!(v);
            }
            None => {
                dbg!("no vlaue");
            }
        };
        let location = info
            .logical_position
            .ok_or_else(|| format!("monitor {info:?} has no logical position"))?;

        let (width, height) = info
            .logical_size
            .ok_or_else(|| format!("monitor {info:?} has no logical size"))?;

        let layer_surface = cmp_ctx.layer_shell.create_layer_surface(
            &qh,
            cmp_ctx.cmp_state.create_surface(&qh),
            Layer::Overlay,
            Some(format!(
                "Monitor with origin: ({}, {}), width: {}px, height: {}px",
                location.0, location.1, width, height,
            )),
            Some(&output),
        );
        let empty_region = cmp_ctx
            .cmp_state
            .wl_compositor()
            .create_region(qh, ());
        layer_surface.set_size(width as u32, height as u32);
        layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer_surface.set_exclusive_zone(-1);
        layer_surface
            .wl_surface()
            .set_input_region(Some(&empty_region));
        layer_surface.commit();
        let screen_info = ScreenInfo {
            size: super::Size {
                width: width as u32,
                height: height as u32,
            },
            origin: super::Point {
                x: location.0,
                y: location.1,
            },
        };
        let shell_layer = Self {
            screen_info,
            pool: SlotPool::new(512 * 512 * 4, &cmp_ctx.shm).expect("Failed to create pool"),
            layer_surface,
            empty_region,
            first_configure: true,
        };
        Ok(shell_layer)
    }
    pub(crate) fn draw_shell(&mut self) {
        let width = self.screen_info.size.width;
        let height = self.screen_info.size.height;
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("buffer should be created successfully");

        // Draw to the window:
        {
            canvas
                .chunks_exact_mut(4)
                .enumerate()
                .for_each(|(_index, chunk)| {
                    let a = 0x55;
                    let r = 0x62 * a / 255;
                    let g = 0x00 * a / 255;
                    let b = 0xFF * a / 255;
                    let color: u32 = (a << 24) + (r << 16) + (g << 8) + b;

                    let array: &mut [u8; 4] = chunk.try_into().unwrap();
                    *array = color.to_le_bytes();
                });
        }

        // Attach and commit.
        buffer
            .attach_to(self.layer_surface.wl_surface())
            .expect("buffer attach");

        self.layer_surface.commit();
    }
}
