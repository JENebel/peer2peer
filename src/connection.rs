use std::{net::TcpStream, sync::{Arc, Mutex, atomic::AtomicBool}, io::{Write, Read}};
use postcard::from_bytes;

use crate::Message;

#[derive(Clone)]
pub struct Connection {
    is_reading: Arc<AtomicBool>,
    write_stream: Arc<Mutex<TcpStream>>,
    read_stream: Arc<Mutex<TcpStream>>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        let write_stream = stream.try_clone().unwrap();
        Connection {
            is_reading: Arc::new(AtomicBool::new(false)),
            write_stream: Arc::new(Mutex::new(write_stream)),
            read_stream: Arc::new(Mutex::new(stream)),
        }
    }

    /// Sends a message to the other end of the connection
    pub fn send_msg(&self, message: Message) -> Result<(), std::io::Error> {
        // Pad the message with the length of the message before sending
        let payload = match postcard::to_allocvec(&message) {
            Ok(bytes) => bytes,
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Could not serialize message"))
        };
        if payload.len() > u16::MAX as usize {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Message too large"));
        }
        let length_bytes = (payload.len() as u16).to_be_bytes();
        let mut msg = Vec::with_capacity(2 + payload.len());
        msg.extend_from_slice(&length_bytes);
        msg.extend_from_slice(&payload);
        self.write_stream.lock().unwrap().write(&msg)?;
        Ok(())
    }

    /// Blocking read message
    pub fn read_msg(&self) -> Result<Message, std::io::Error> {
        if self.is_reading.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Already reading"));
        }

        let mut length_buffer = [0; 2];
        let mut stream = self.read_stream.lock().unwrap();
        stream.read_exact(&mut length_buffer)?;
        let length = u16::from_be_bytes(length_buffer);

        let mut message_buffer = vec![0; length as usize];
        stream.read_exact(&mut message_buffer)?;

        self.is_reading.store(false, std::sync::atomic::Ordering::SeqCst);

        match from_bytes(&message_buffer) {
            Ok(msg) => Ok(msg),
            Err(_) => Err(std::io::Error::new(std::io::ErrorKind::Other, "Could not deserialize message"))
        }
    }
}