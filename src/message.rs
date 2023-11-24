use std::net::SocketAddr;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Join(u16), // Join the network with the given port
    JoinDenied,
    JoinAccepted,
    PeerListRequest,
    PeerList(Vec<SocketAddr>),
    HeartBeat,
    Flood(String),
}