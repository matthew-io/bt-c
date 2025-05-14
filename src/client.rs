use std::{collections::{BTreeMap, HashMap}, error::Error, fs::{File, OpenOptions}, future::Pending, hash::Hash, io::{self, ErrorKind}, os::unix::fs::FileExt as _, path::Path, time::{SystemTime, UNIX_EPOCH}};
use std::io::{Result as IoResult};

use log::{info, warn};
use sha1::{Sha1, Digest};

use crate::{protocol::PeerConnection, torrent::Torrent};

const REQUEST_SIZE: u32 = 2_u32.pow(14);

// **** ENUMS **** //

// status enum for pieces
#[derive(PartialEq, Clone, Debug)]
enum Status {
    Missing = 0,
    Pending = 1,
    Retrieved = 2,
}

// **** STRUCTS **** // 


// a block is the smallest unit used in torrents. 
// each block has a coreresponding piece, offset and length.
// these three values are used to determine their location in the torrent data.
#[derive(Clone, PartialEq, Debug)]
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
#[derive(Clone)]
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

    pub fn block_received(&mut self, peer_id: String, piece_index: u64, block_offset: u64, data: Vec<u8>) {
        if let Some(pos) = self.pending_blocks.iter().position(|r| {
            r.block.piece == piece_index && r.block.offset == block_offset
        }) {
            self.pending_blocks.remove(pos);
        }
    
        let index = piece_index as u32;
        if let Some(pos) = self.ongoing_pieces.iter().position(|p| p.index == index) {
            let mut piece = self.ongoing_pieces.remove(pos);
    
            piece.block_received(block_offset as u32, data);
    
            if piece.is_complete() {
                if piece.is_hash_matching() {
                    let offset = piece.index as u64 * self.torrent.piece_length as u64;
                    if let Err(e) = self.write_piece(offset, &piece.blocks) {
                        eprintln!("failed to write piece {} to file: {}", piece.index, e);
                        return;
                    }
    
                    self.have_pieces.push(piece);
    
                    let complete = self.have_pieces.len();
                    let total = self.total_pieces as usize;
                    let percentage = (complete as f64 / total as f64) * 100.0;
                    info!("{}/{} pieces downloaded ({:.2}%)", complete, total, percentage);
                } else {
                    warn!("discarding corrupt piece {}", piece.index);
                    piece.reset();
                    self.ongoing_pieces.push(piece);
                }
            } else {
                self.ongoing_pieces.push(piece);
            }
        } else {
            warn!("trying to update piece {} that is not ongoing!", piece_index);
        }
    }
    

    pub fn write_piece(&mut self, offset: u64, blocks: &[Block]) -> io::Result<()> {
        let mut buffer = Vec::new();

        for block in blocks {
            if let Some(ref data) = block.data {
                buffer.extend_from_slice(data);
            } else {
                return Err(io::Error::new(ErrorKind::Other, "missing block data"));
            }
        }

        self.fd.write_all_at(&buffer, offset)?;
        Ok(())
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

    pub fn next_request(&mut self, peer_id: &String) -> Option<Block> {
        
        if let Some(block) = self.expired_requests(peer_id) {
            return Some(block);
        }

        if let Some(block) = self.next_ongoing(peer_id) {
            return Some(block);
        }

        if let Some(mut block) = self.get_rarest_piece(peer_id) {
            let next_block = block.next_request()?;
            return Some(next_block);
        }

        None
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
    }

    pub fn get_rarest_piece(&mut self, peer_id: &String) -> Option<Piece> {
        let mut piece_count: HashMap<u32, u32> = HashMap::new();

        let peer_bitfield = match self.peers.get(peer_id) {
            Some(bf) => bf,
            None => {
                eprintln!("peer not found: {}", peer_id);
                return None;
            }
        };

        for piece in &self.missing_pieces {
            if !peer_bitfield[piece.index as usize] == 0 {
                continue;
            }

            let mut count = 0;
            for other_bitfield in self.peers.values() {
                if other_bitfield[piece.index as usize] > 0 {
                    count += 1
                }
            }

            piece_count.insert(piece.index, count);
        }

        let rarest_index = piece_count
            .iter()
            .min_by_key(|(_, &count)| count)
            .map(|(&index, _)| index)?;

        if let Some(pos) = self.missing_pieces.iter().position(|p| p.index == rarest_index) {
            let piece = self.missing_pieces.remove(pos);
            self.ongoing_pieces.push(piece.clone());
            return Some(piece);
        }

        None
    }

    pub fn next_missing(&mut self, peer_id: &str) -> Option<Block> {
        if let Some(bitfield) = self.peers.get(peer_id) {
            for i in 0..self.missing_pieces.len() {
                let index = self.missing_pieces[i].index as usize;
    
                if let Some(&bit) = bitfield.get(index) {
                    if bit != 0 {
                        let mut piece = self.missing_pieces.remove(i);
                        self.ongoing_pieces.push(piece.clone());
                        return piece.next_request();
                    }
                }
            }
        } else {
            eprintln!("peer not found: {}", peer_id);
        }
    
        None
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
    pub fn next_request(&mut self) -> Option<Block> {
        let index = self.blocks.iter().position(|b| b.status == Status::Missing);

        if let Some(idx) = index {
            self.blocks[idx].status = Status::Pending;
            Some(self.blocks[idx].clone())
        } else {
            None
        }
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

mod tests {
    use super::*;

    fn create_test_blocks() -> Vec<Block> {
        (0..10).map(|offset| Block::new(0, offset * 10, 10)).collect()
    }

    #[test]
    fn test_empty_piece() {
        let mut p = Piece::new(0, vec![], "".to_string());
        assert_eq!(p.next_request(), None); 
    }

    #[test]
    fn test_request_ok() {
        let blocks = create_test_blocks();
        let mut p = Piece::new(0, blocks, "".to_string());

        let block = p.next_request().expect("should return a block");
        let missing = p.blocks.iter().filter(|b| b.status == Status::Missing).count();
        let pending = p.blocks.iter().filter(|b| b.status == Status::Pending).count();

        assert_eq!(pending, 1);
        assert_eq!(missing, 9);
        assert_eq!(block.offset, 0);
    }

    #[test]
    fn test_reset_missing_block() {
        let mut p = Piece::new(0, vec![], "".to_string());
        p.block_received(123, b"hello".to_vec());
    }

    #[test]
    fn test_reset_block() {
        let blocks = create_test_blocks();
        let mut p = Piece::new(0, blocks, "".to_string());

        p.block_received(10, b"hello".to_vec());

        let retrieved = p.blocks.iter().filter(|b| b.status == Status::Retrieved).count();
        let missing = p.blocks.iter().filter(|b| b.status == Status::Missing).count();

        assert_eq!(retrieved, 1);
        assert_eq!(missing, 9);
    }
}