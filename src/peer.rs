use std::{net::{TcpStream, SocketAddr, TcpListener}, sync::{Arc, RwLock}, thread, time::{Duration, Instant}, collections::{HashMap, HashSet}, io::{Error, ErrorKind}};
use local_ip_address::local_ip;

use crate::{Connection, Message::*};

type ConnectionMap = HashMap<SocketAddr, Connection>;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
pub const REFRESH_CONNECTIONS_INTERVAL: Duration = Duration::from_secs(60); // 1min
pub const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONNECTIONS: usize = 12;

pub const DEFAULT_PORT: u16 = 8998;

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

    pub fn connections(&self) -> usize {
        self.connections.read().unwrap().len()
    }

    pub fn start(port: Option<u16>) -> Result<Self, String> {
        // Resolve the local address
        let ip = local_ip().unwrap();
        let mut address = SocketAddr::new(ip, port.unwrap_or(0));

        let listener = match TcpListener::bind(address) {
            Ok(listener) => listener,
            Err(err) => return Err(format!("Could not listen on {address}: {err}")),
        };

        address.set_port(listener.local_addr().unwrap().port());

        let peer = Peer {
            address: Arc::new(address),
            connections: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start sending heartbeats and sending heartbeats
        peer.heartbeat_loop();
        peer.refresh_connections_loop();

        // Spawn a thread to listen for incoming connectionss
        let peer_clone = peer.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let peer = peer_clone.clone();
                        thread::spawn(move || {
                            peer.handle_incoming(Connection::new(stream));
                        });
                    },
                    Err(err) => println!("Error handling incoming connection: {}", err),
                }
            }
        });

        println!("Listening on {}", address);

        return Ok(peer)
    }

    pub fn connect_to_network(port: Option<u16>, seed_address: SocketAddr) -> Result<Self, String> {
        // Start the peer
        let peer = match Peer::start(port) {
            Ok(peer) => peer,
            Err(err) => return Err(err),
        };

        // Bootstrap the peer
        let mut connected = false;
        let mut visited_peers: HashSet<SocketAddr> = HashSet::new();
        let mut last_peers: Vec<SocketAddr> = Vec::new();
        last_peers.push(seed_address);
        visited_peers.insert(*peer.address);
        while peer.connections.read().unwrap().len() <= MAX_CONNECTIONS / 2 && !last_peers.is_empty() {
            let mut new_peers: Vec<SocketAddr> = Vec::new();
            fastrand::shuffle(last_peers.as_mut_slice());
            let mut peers = peer.connections.read().unwrap().len();
            for p in last_peers {
                if peers >= MAX_CONNECTIONS / 2 {
                    break;
                }
                if let Ok(new) = peer.request_peers_connect(p) {
                    connected = true;
                    peers += new.len();
                    new.iter().filter(|addr| !visited_peers.contains(addr)).for_each(|addr| {
                        new_peers.push(*addr);
                    }
                )}
            }
            visited_peers.extend(&new_peers);
            last_peers = new_peers;
        }

        //println!("Connected to {} peers", peer.connections.read().unwrap().len());

        if !connected {
            Err("Could not connect to any peers".to_string())
        } else {
            Ok(peer)
        }
    }

    /// Start sending heartbeats to all connected peers
    fn heartbeat_loop(&self) {
        let peer = self.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(HEARTBEAT_INTERVAL);
                for (_, conn) in peer.connections.read().unwrap().iter() {
                    if conn.send_msg(HeartBeat).is_err() {
                        break
                    }
                }
            }
        });
    }

    /// Refresh the connections by connecting to new peers once in a while
    /// 
    /// This keeps the network tightly connected
    fn refresh_connections_loop(&self) {
        let peer = self.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(REFRESH_CONNECTIONS_INTERVAL);
                let rand = fastrand::usize(..peer.connections.read().unwrap().len());
                let rand_peer = peer.connections.read().unwrap().iter().nth(rand).unwrap().0.clone();
                peer.connections.read().unwrap().get(&rand_peer).unwrap().send_msg(PeerListRequest).unwrap();
            }
        });
    }

    /// Handle an incoming connection
    fn handle_incoming(&self, conn: Connection) {
        // Set a timeout for the conn. If we don't accept via join within limit, disconnect
        let connect_time = Instant::now();
        let address;

        loop {
            if connect_time.elapsed() > Duration::from_secs(5) {
                println!("Timed out waiting for join response from {}", conn.peer_addr());
                conn.shutdown();
                return;
            }

            let msg = match conn.read_msg() {
                Ok(msg) => msg,
                Err(_) => {
                    return;
                },
            };

            match msg {
                Join(port) => {
                    // Check if we have room for another connection
                    if self.connections.read().unwrap().len() >= MAX_CONNECTIONS {
                        conn.send_msg(JoinDenied).unwrap();
                        continue;
                    } else {
                        conn.send_msg(JoinAccepted).unwrap();
                    }

                    // Add the connection to the peer list
                    let mut addr = conn.peer_addr();
                    addr.set_port(port);
                    println!("{} joined", addr);
                    address = Some(addr);
                    break
                },
                PeerListRequest => {
                    self.send_peer_list(&conn)
                },
                _ => {
                    println!("Unexpected message from {}: {:?}", conn.peer_addr(), msg);
                    conn.shutdown();
                    return;
                }
            }
        }

        self.handle_connection(conn, address.unwrap())
    }

    /// Send a list of peers to the given connection
    fn send_peer_list(&self, conn: &Connection) {
        let addresses: Vec<SocketAddr> = self.connections
            .read().unwrap()
            .keys().cloned()
            .collect();

        match conn.send_msg(PeerList(addresses)) {
            Ok(_) => {},
            Err(err) => println!("Error sending message: {}", err),
        }
    }

    /// Connect to a peer
    fn connect(&self, address: SocketAddr) -> Result<(), Error> {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
        let conn = Connection::new(stream);
        self.join(conn, address)?;
        Ok(())
    }

    /// Attempt to join the peer
    fn join(&self, conn: Connection, address: SocketAddr) -> Result<(), Error> {
        if self.connections.read().unwrap().len() >= MAX_CONNECTIONS
            || self.connections.read().unwrap().contains_key(&address) {
            return Err(Error::new(ErrorKind::ConnectionRefused, "Max connections reached"));
        }
        conn.send_msg(Join(self.address.port()))?;
        match conn.read_msg()? {
            JoinAccepted => {},
            JoinDenied => {
                return Err(Error::new(ErrorKind::ConnectionRefused, "Join denied"));
            },
            msg => {
                println!("Unexpected message from {}: {:?}", conn.peer_addr(), msg);
                conn.shutdown();
                return Err(Error::new(ErrorKind::Other, "Unexpected message"));
            }
        }
        let peer = self.clone();
        thread::spawn(move || {
            peer.handle_connection(conn, address)
        });
        println!("Connected to {}", address);
        Ok(())
    }

    /// Connect to a peer but request a list of peers from them first for bootstrapping
    fn request_peers_connect(&self, address: SocketAddr) -> Result<Vec<SocketAddr>, Error> {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
        let conn = Connection::new(stream);
        conn.send_msg(PeerListRequest)?;
        let peers = match conn.read_msg()? {
            PeerList(addresses) => addresses,
            msg => {
                println!("Unexpected message from {}: {:?}", conn.peer_addr(), msg);
                conn.shutdown();
                return Err(Error::new(ErrorKind::Other, "Unexpected message"))
            }
        };
        // Attempt joining the peer
        let _ = self.join(conn, address);
        Ok(peers)
    }

    /// Terminate a connection. This will remove it from the peer list, and close the sockets
    fn terminate(&self, address: SocketAddr) {
        if let Some(conn) = self.connections.read().unwrap().get(&address) {
            conn.shutdown();
        }
        self.connections.write().unwrap().remove(&address);
    }

    /// Handle a connection after handshake is complete
    fn handle_connection(&self, conn: Connection, address: SocketAddr) {
        println!("Succesfully connected to {}", address);
        self.connections.write().unwrap().insert(address, conn.clone());

        loop {
            // Read next message and clean up if there's an error
            let message = match conn.read_msg() {
                Ok(message) => message,
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::UnexpectedEof => {
                            println!("Disconnected from {}", address);
                            self.terminate(address);
                        }
                        _ => println!("Error reading message from {}: {}", address, err)
                    }
                    break;
                },
            };

            // Handle the message
            match message {
                Join(_) | JoinDenied | JoinAccepted => {
                    println!("Terminating connetion with {} because of unexpected message {:?}", address, message);
                    self.terminate(address)
                },
                PeerListRequest => {
                    self.send_peer_list(&conn)
                },
                PeerList(mut addresses) => {
                    // Shuffle the addresses and connect until we have MAX_CONNECTIONS
                    fastrand::shuffle(addresses.as_mut_slice());
                    for addr in addresses {
                        // Do not connect to self or existing conns
                        if addr != *self.address && !self.connections.read().unwrap().contains_key(&addr) {
                            self.connect(addr).unwrap();
                        }
                    }
                    // Remove random connections until we have MAX_CONNECTIONS. This randomized which are kept
                    let mut connections = self.connections.write().unwrap();
                    while connections.len() > MAX_CONNECTIONS / 2 {
                        let rand = fastrand::usize(..connections.len());
                        let rand_peer = connections.iter().nth(rand).unwrap().0.clone();
                        connections.remove(&rand_peer);
                    }

                    println!("-------- NEW --------");
                    for conn in connections.keys() {
                        println!("Connected to {}", conn);
                    }
                },
                HeartBeat => { /* Just refreshes the underlying socket timout */ },
                Flood(msg) => {
                    println!("Received: {}", msg);
                }
            }
        }
    }

    pub fn flood(&self, message: String) {
        for (_, conn) in self.connections.read().unwrap().iter() {
            match conn.send_msg(Flood(message.clone())) {
                Ok(_) => {},
                Err(err) => println!("Error sending message: {}", err),
            }
        }
    }
}