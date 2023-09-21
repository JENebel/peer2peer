use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    BootstrapRequest,
    BootstrapResponse,
    HeartBeat,

    Flood(String),
}