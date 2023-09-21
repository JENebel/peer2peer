use std::{thread, time::Duration};

use peer2peer::*;

fn main() {
    let peer1 = Peer::start(None).unwrap();

    // Sleep for a bit to make sure the peer is listening
    thread::sleep(Duration::from_millis(100));

    Peer::start_peer(None, peer1.address()).unwrap();

    thread::sleep(Duration::from_millis(100));

    peer1.flood("Hello, world!".to_string());

    println!("Done");
    loop {
        
    }
}