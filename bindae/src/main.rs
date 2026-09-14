use std::time::Duration;

mod manager;
mod socket;

fn main() {
    println!("Hello, world!");
    loop {
        std::thread::sleep(Duration::from_secs(20));
    }
}
