mod command;
mod socket;
mod store;
mod value;

use crate::socket::Socket;
use crate::store::Store;
use std::sync::{Arc, Mutex};
use tokio::{io::Result, net::TcpListener};

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    let cache = Arc::new(Mutex::new(Store::new()));

    loop {
        let (socket, addr) = listener.accept().await?;

        println!("accepted connection from {}", addr);

        let clone = cache.clone();

        tokio::spawn(async move {
            match Socket::from(socket, clone).run().await {
                Ok(()) => {}
                Err(error) => eprintln!("encountered error: {}", error),
            }

            println!("dropped connection with {}", addr);
        });
    }
}
