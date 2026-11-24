use client_lib::{builder::BindingsBuilder, client::Client};

fn main() {
    let bindings = BindingsBuilder::new();
    let mut client = Client::connect(bindings.build().unwrap()).unwrap();
    client.run().unwrap();
}
