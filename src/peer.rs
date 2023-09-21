use std::{net::{TcpStream, SocketAddr, TcpListener}, sync::{Arc, RwLock}, thread, time::Duration, collections::HashMap};
use local_ip_address::local_ip;

use crate::{Connection, message};

type ConnectionMap = HashMap<SocketAddr, Connection>;

pub struct Peer {
    address: Arc<SocketAddr>,
    connections: Arc<RwLock<ConnectionMap>>,
}

impl Clone for Peer {
    fn clone(&self) -> Self {
        Peer {
            address: self.address.clone(),
            connections: self.connections.clone(),
        }
    }
}

impl Peer {
    pub fn address(&self) -> SocketAddr {
        *self.address
    }

    pub fn start(port: Option<u16>) -> Result<Self, String> {
        // Resolve the local address
        let ip = local_ip().unwrap();

        let mut address: SocketAddr = match port {
            Some(port) => SocketAddr::new(ip, port),
            None => SocketAddr::new(ip, 0), // Request open port from OS
        };

        let listener = match TcpListener::bind(address) {
            Ok(listener) => listener,
            Err(err) => return Err(format!("Could not listen on {address}: {err}")),
        };

        address.set_port(listener.local_addr().unwrap().port());

        let peer = Peer {
            address: Arc::new(address),
            connections: Arc::new(RwLock::new(HashMap::new())),
        };

        // Spawn a thread to listen for incoming connections
        let peer_clone = peer.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let peer = peer_clone.clone();
                        thread::spawn(move || {
                            peer.handle_connection(stream);
                        });
                    },
                    Err(err) => println!("Error handling incoming connection: {}", err),
                }
            }
        });

        println!("Listening on {}", address);

        return Ok(peer)
    }

    pub fn start_peer(own_port: Option<u16>, seed_address: SocketAddr) -> Result<Self, String> {
        // Start the peer
        let peer = match Peer::start(own_port) {
            Ok(peer) => peer,
            Err(err) => return Err(err),
        };

        // Connect to the seed peer
        peer.connect(seed_address)?;

        Ok(peer)
    }

    fn connect(&self, seed_address: SocketAddr) -> Result<(), String> {
        println!("Connecting to {}", seed_address);

        // Connect to the peer over TCP
        let peer = self.clone();
        match TcpStream::connect_timeout(&seed_address, Duration::from_secs(3)) {
            Ok(stream) => {
                thread::spawn(move || {
                    peer.handle_connection(stream);
                });
            },
            Err(err) => return Err(format!("Could not connect to peer: {}", err)),
        };

        Ok(())
    }

    fn handle_connection(&self, stream: TcpStream) {
        println!("Incoming connection from {}", stream.peer_addr().unwrap());

        // Add the stream to the list of peers
        let addr = stream.peer_addr().unwrap();
        let connection = Connection::new(stream);
        self.connections.write().unwrap().insert(addr, connection.clone());

        loop {
            let message = match connection.read_msg() {
                Ok(message) => message,
                Err(err) => {
                    println!("Error reading message from {}: {}", addr, err);
                    break;
                },
            };

            match message {
                crate::Message::BootstrapRequest => todo!(),
                crate::Message::BootstrapResponse => todo!(),
                crate::Message::HeartBeat => todo!(),
                crate::Message::Flood(msg) => {
                    println!("Received: {}", msg);
                }
            }
        }
    }

    pub fn flood(&self, message: String) {
        for (_, conn) in self.connections.read().unwrap().iter() {
            match conn.send_msg(message::Message::Flood(message.clone())) {
                Ok(_) => {},
                Err(err) => println!("Error sending message: {}", err),
            }
        }
    }
}