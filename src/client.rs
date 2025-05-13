use std::{collections::{BTreeMap, HashMap}, fs::{File, OpenOptions}, hash::Hash, path::Path};
use std::io::{Result as IoResult};

use sha1::{Sha1, Digest};

use crate::{protocol::PeerConnection, torrent::Torrent};

const REQUEST_SIZE: u32 = 2_u32.pow(14);

// **** ENUMS **** //

// status enum for pieces
#[derive(PartialEq, Clone)]
enum Status {
    Missing = 0,
    Pending = 1,
    Retrieved = 2,
}

// **** STRUCTS **** // 


// a block is the smallest unit used in torrents. 
// each block has a coreresponding piece, offset and length.
// these three values are used to determine their location in the torrent data.
#[derive(Clone)]
pub struct Block {
    piece: u64,
    offset: u64,
    length: u64,
    status: Status,
    data: Option<Vec<u8>>,
}

// torrents are composed of pieces. each of a particular size.
// pieces themselves are composed of a smaller unit: blocks.
// a piece is considered complete if all of its blocks
// have been received.
pub struct Piece {
    index: u32,
    blocks: Vec<Block>,
    hash_value: String,
}

pub struct PieceManager {
    torrent: Torrent,
    peers: HashMap<String, Vec<u8>>,
    pending_blocks: Vec<Block>,
    missing_pieces: Vec<Piece>,
    ongoing_pieces: Vec<Piece>,
    have_pieces: Vec<Piece>,
    max_pending_time: u32,
    total_pieces: u16,
    fd: File,
}


impl PieceManager {
    pub fn new(torrent: Torrent) -> IoResult<PieceManager> {
        let total_pieces = torrent.pieces.len() as u16;

        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(Path::new(&torrent.output_file))?;

        let mut pm = PieceManager {
            torrent,
            peers: HashMap::new(),
            pending_blocks: Vec::new(),
            missing_pieces: Vec::new(),
            ongoing_pieces: Vec::new(),
            have_pieces: Vec::new(),
            max_pending_time: 300_000,
            total_pieces,
            fd,
        };

        pm.missing_pieces = pm.initiate_pieces();

        Ok(pm)
    }

    // preconstruct the length of the missing piece vec for a particular torrent
    pub fn initiate_pieces(&self) -> Vec<Piece> {
        let torrent = &self.torrent;
        let mut pieces: Vec<Piece> = Vec::new();
        let total_pieces = torrent.pieces.len();
        let std_piece_blocks = (torrent.piece_length + REQUEST_SIZE - 1) / REQUEST_SIZE;
        
        let mut blocks: Vec<Block> = Vec::new(); 

        for (i, hash_value) in torrent.pieces.iter().enumerate() {
            if i < (total_pieces - 1) {
                for offset in 0..std_piece_blocks {
                    let block: Block = Block::new(i as u64, (offset * REQUEST_SIZE )as u64, REQUEST_SIZE as u64);
                    blocks.push(block);
                }
            }
        }

        return pieces
    }
    
}

impl Block {
    pub fn new(piece: u64, offset: u64, length: u64) -> Block {
        Block {
            piece,
            offset,
            length,
            status: Status::Missing,
            data: None,
        }
    } 
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
        if let Some(block) = self.blocks.iter_mut().find(|b| b.offset == offset as u64) {
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