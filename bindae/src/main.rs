use std::time::Duration;

mod manager;
mod socket;
mod core;
pub mod compositor;

fn main() {
    println!("Hello, world!");
    // TODO: Create uinput and input manager here and send the channels to the core.
    // Also create the socket connector instance here and give the core the channel.
    loop {
        std::thread::sleep(Duration::from_secs(20));
    }
}
