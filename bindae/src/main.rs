use crate::ipc::connector;

pub mod compositor_interface;
mod router;
pub mod ipc;
mod device_interface;

fn main() {
    // TODO: Create uinput and input manager here and send the channels to the core.
    // Also create the socket connector instance here and give the core the channel.

    // Start socket server.
    let (connection_tx, connection_rx) = crossbeam_channel::unbounded();
    let socket_server = match connector::create_socket_server() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create socket server, aborting: {e}");
            return;
        }
    };
    let conn_listener_handle = connector::listen_socket_server(socket_server, connection_tx);

    // Start UInput manager.

    // Start Input manager.

    // Join on created threads.
    match conn_listener_handle.join(){
        Ok(_) => (),
        Err(e) => eprintln!("Socket server thread panicked: {e:?}"),
    }
}
