use std::{collections::{BTreeMap, HashMap}, fs::{File, OpenOptions}, future::Pending, hash::Hash, path::Path, time::{SystemTime, UNIX_EPOCH}};
use std::io::{Result as IoResult};

use log::info;
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

pub struct PendingRequest {
    block: Block,
    added: u128,
}

pub struct PieceManager {
    torrent: Torrent,
    peers: HashMap<String, Vec<u8>>,
    pending_blocks: Vec<PendingRequest>,
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
        let std_piece_blocks = torrent.piece_length.div_ceil(REQUEST_SIZE);

        for (i, hash_value) in torrent.pieces.iter().enumerate() {
            let mut blocks: Vec<Block> = Vec::new(); 
            // check if the current piece is not the last piece
            if i < (total_pieces - 1) {
                for offset in 0..std_piece_blocks {
                    let block: Block = Block::new(i as u64, (offset * REQUEST_SIZE )as u64, REQUEST_SIZE as u64);
                    blocks.push(block);
                }
            // if the current piece is not the last piece, then it might be the case
            // that the length of this piece is not the same as the rest of the pieces
            // and we need to account for that
            } else {
                // get the length of the last piece and corresponding blocks
                let last_length = torrent.total_size % torrent.piece_length as u64;
                let num_blocks = last_length.div_ceil(REQUEST_SIZE as u64);

                for offset in 0..num_blocks {
                    let start = offset * REQUEST_SIZE as u64;
                    let length = std::cmp::min(REQUEST_SIZE as u64, last_length - start);
                    blocks.push(Block::new(i as u64, start, length));
                }

                if last_length % REQUEST_SIZE as u64 > 0 {
                    if let Some(last_block) = blocks.last_mut() {
                        last_block.length = last_length % REQUEST_SIZE as u64;
                    }
                }
            }

            pieces.push(Piece 
                { index: i as u32, blocks, hash_value: hash_value.to_string(),  }
            )
        }
        pieces
    }

    pub fn complete(&self) -> bool {
        // returns true if we have downloaded all of the pieces for this torrent
        self.have_pieces.len() == self.total_pieces as usize
    }

    pub fn bytes_downloaded(&self) -> u64 {
        // gets the number of bytes downloaded
        (self.have_pieces.len() * self.torrent.piece_length as usize) as u64
    }

    pub fn bytes_uploaded(&self) -> u64 {
        // TODO: seeding not implemented
        0
    }

    // adds a peer and its corresponding bitfield
    pub fn add_peer(&mut self, peer_id: String, bitfield: Vec<u8>) {
        self.peers.insert(peer_id, bitfield);
    }

    pub fn update_peer(&mut self, peer_id: String, index: u32) {
        if let Some(bitfield) = self.peers.get_mut(&peer_id) {
            if let Some(byte) = bitfield.get_mut(index as usize) {
                *byte = 1
            } else {
                eprintln!("index {} out of range for peer {}", index, peer_id)
            }
        } else {
            eprintln!("peer {} not found", peer_id)
        }
    }

    pub fn delete_peer(&mut self, peer_id: String) {
        if self.peers.remove(&peer_id).is_none() {
            eprintln!("couldn't remove peer because it doesn't exist")
        }
    }

    pub fn next_request(&mut self, peer_id: String) {
        if !self.peers.contains_key(&peer_id) {
            eprintln!("peer doesn't exist")
        }

        unimplemented!()
    }


    pub fn expired_requests(&mut self, peer_id: &str) -> Option<Block> {
        let current = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u128;

        for request in self.pending_blocks.iter_mut() {
            if let Some(bitfield) = self.peers.get(peer_id) {
                if let Some(&has_piece) = bitfield.get(request.block.piece as usize) {
                    if has_piece != 0 && request.added + (self.max_pending_time as u128) < current {
                        info!(
                            "re-requesting block {} for piece {}",
                            request.block.offset, request.block.piece
                        );
                        request.added = current;
                        return Some(request.block.clone());
                    }
                }
            }
        }
        None
    }

    pub fn next_ongoing(&mut self, peer_id: &str) -> Option<Block> {
        for piece_idx in 0..self.ongoing_pieces.len() {
            let piece = &mut self.ongoing_pieces[piece_idx];
            
            if let Some(bitfield) = self.peers.get(peer_id) {
                if piece.index as usize >= bitfield.len() || bitfield[piece.index as usize] == 0 {
                    continue;
                }
                
                if let Some(block) = piece.next_request() {
                    let current_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("Time went backwards")
                        .as_millis();
                    
                    self.pending_blocks.push(PendingRequest {
                        block: block.clone(),
                        added: current_time,
                    });
                    
                    return Some(block);
                }
            }
        }
        
        None
    }}

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
    pub fn next_request(&self) -> Option<Block> {
        self.blocks
            .iter()
            .find(|b| b.status == Status::Missing)
            .cloned()
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