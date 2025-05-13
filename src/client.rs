use sha1::{Sha1, Digest};

use crate::{protocol::PeerConnection, torrent::Torrent};

// **** ENUMS **** //

// status enum for pieces
#[derive(PartialEq, Clone)]
enum Status {
    Missing = 0,
    Pending = 1,
    Retrieved = 2,
}

// **** STRUCTS **** // 
#[derive(Clone)]
pub struct Block {
    piece: u32,
    offset: u32,
    length: u32,
    status: Status,
    data: Option<Vec<u8>>,
}

pub struct Piece {
    index: u32,
    blocks: Vec<Block>,
    hash_value: String,
}

pub struct PieceManager {
    torrent: Torrent,
    peers: Vec<PeerConnection>
}

// **** IMPLEMENTATIONS **** // 
impl Piece {
    // create new piece object
    pub fn new(index: u32, blocks: Vec<Block>, hash_value: String) -> Piece {
        Piece {
            index,
            blocks,
            hash_value
        }
    }

    // set the status of all blocks to missing
    pub fn reset(&mut self) {
        for block in &mut self.blocks {
            block.status = Status::Missing;
        }
    }

    // get next block to be requested by the client.
    pub fn next_request(&self) -> Block {
        let mut missing: Vec<Block> = self.blocks
            .iter()
            .filter(|b| b.status == Status::Missing)
            .cloned()
            .collect();
        missing.first().unwrap().clone()
    }

    // update the block information if the block has now been received by the client
    pub fn block_received(&mut self, offset: u32, data: Vec<u8>) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.offset == offset) {
            block.status = Status::Retrieved;
            block.data = Some(data);
        } else {
            eprintln!("trying to finish a non-existing block: {}", offset)
        }
    }

    // check if all of the blocks for this piece have been received
    pub fn is_complete(&self) -> bool {
        let blocks: Vec<Block> = self.blocks
            .iter()
            .filter(|b| b.status != Status::Retrieved)
            .cloned()
            .collect();
        blocks.len() == 0
    }

    pub fn is_hash_matching(&self) -> bool {
        let mut hasher = Sha1::new();

        for block in &self.blocks {
            if let Some(ref data) = block.data {
                hasher.update(data);
            } else {
               return false;
            }
        }

        let calculated_hash = hasher.finalize();

        let hex_hash = hex::encode(calculated_hash);
        self.hash_value == hex_hash
    }
}