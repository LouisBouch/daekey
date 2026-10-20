use library::client::Client;

fn main() {
    let mut client = Client::instantiate(5).unwrap();
    client.run().unwrap();
}
