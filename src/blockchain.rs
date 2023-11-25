use std::collections::HashMap;
use sha3::{Digest, Sha3_256};

pub struct Transaction {
    id: u64,
    sender: [u8; 32],
    recipient: [u8; 32],
    signature: [u8; 64],
    amount: u64,
}

impl Transaction {
    pub fn bytes(&self) -> [u8; 144] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.id.to_be_bytes());
        bytes.extend_from_slice(&self.sender);
        bytes.extend_from_slice(&self.recipient);
        bytes.extend_from_slice(&self.signature);
        bytes.extend_from_slice(&self.amount.to_be_bytes());
        bytes.try_into().unwrap()
    }
}

pub struct Block {
    slot: u64,
    winner: [u8; 32],
    previous: [u8; 32],
    transactions: Vec<Transaction>,
}

impl Block {
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(self.slot.to_be_bytes());
        hasher.update(&self.winner);
        hasher.update(self.previous);
        for transaction in &self.transactions {
            hasher.update(&transaction.bytes());
        }
        let result = hasher.finalize();
        result.into()
    }
}

pub struct Blockchain {
    blocks: HashMap<Sha3_256, Block>
}