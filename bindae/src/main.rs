use crate::{
    device_interface::{input::Input, uinput::UInput},
    ipc::connector,
    router::Router,
};

pub mod compositor_interface;
mod device_interface;
pub mod ipc;
mod router;

fn main() {
    // Start socket server.
    let (connector_tx, fr_connector_rx) = crossbeam_channel::unbounded();
    let socket_server = match connector::create_socket_server() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create socket server, aborting: {e}");
            return;
        }
    };
    let conn_listener_handle = connector::listen_socket_server(socket_server, connector_tx);

    // Define channels.
    // TODO: Go over what channels are defined and if they should be merged/split.
    let (fr_dev_interface_tx, fr_dev_interface_rx) = crossbeam_channel::unbounded();
    let (fr_connection_tx, fr_connection_rx) = crossbeam_channel::unbounded();
    let (to_uinput_tx, to_uinput_rx) = crossbeam_channel::unbounded();
    let (to_input_tx, to_input_rx) = crossbeam_channel::unbounded();

    // Start UInput interface.
    let uinput_handle = UInput::new(fr_dev_interface_tx.clone(), to_uinput_rx).launch();

    // Start Input interface.
    let input_handle = Input::new(fr_dev_interface_tx.clone(), to_input_rx, to_uinput_tx.clone()).launch();

    // Start Router.
    let router_handle = Router::new(
        fr_dev_interface_rx,
        fr_connection_rx,
        fr_connector_rx,
        to_uinput_tx,
        to_input_tx,
        fr_connection_tx,
    )
    .launch();

    // Join on created threads.
    match conn_listener_handle.join() {
        Ok(_) => (),
        Err(e) => eprintln!("Socket server thread panicked: {e:?}"),
    }
    match uinput_handle.join() {
        Ok(_) => (),
        Err(e) => eprintln!("UInput interface thread panicked: {e:?}"),
    }
    match input_handle.join() {
        Ok(_) => (),
        Err(e) => eprintln!("Input interface thread panicked: {e:?}"),
    }
    match router_handle.join() {
        Ok(_) => (),
        Err(e) => eprintln!("Router thread panicked: {e:?}"),
    }
}
