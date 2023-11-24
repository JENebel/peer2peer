use std::{net::{SocketAddr, IpAddr}, str::FromStr};
#[allow(unused_imports)]
use std::{thread, time::Duration};

use peer2peer::*;

fn main() {
    let peer = match std::env::args().nth(1).as_ref().map(String::as_str) {
        None => Peer::start(Some(DEFAULT_PORT)).unwrap(),
        Some(address) => {
            let iter = address.split(":");
            let ip = iter.clone().next().unwrap();
            let port = iter.clone().nth(1).unwrap();
            Peer::connect_to_network(None, SocketAddr::new(IpAddr::from_str(ip).unwrap(), port.parse().unwrap())).unwrap()
        }
    };

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let cmd = input.trim().split(" ").clone().next().unwrap();
        let rest = input.trim().split_at(cmd.len()).1.trim();
        match cmd {
            "exit" => break,
            "flood" => peer.flood(rest.to_string()),
            _ => println!("Unknown command: {}", cmd),
        }
    }
}

#[test]
fn auto() {
    let peer1 = Peer::start(Some(DEFAULT_PORT)).unwrap();

    thread::sleep(Duration::from_millis(100));

    let mut peers: Vec<Peer> = Vec::new();
    peers.push(peer1);

    for _ in 1..1000 {
        let p = Peer::connect_to_network(None, peers[0].address()).unwrap();
        peers.push(p);
    }

    thread::sleep(Duration::from_millis(100));

    peers[0].flood("Monkey".to_string());

    thread::sleep(Duration::from_millis(100));

    peers[7].flood("Donkey".to_string());

    thread::sleep(Duration::from_millis(1000));
}