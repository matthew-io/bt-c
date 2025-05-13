use std::collections::VecDeque;

pub struct PeerConnection {
    state: Vec<u8>,
    peer_state: Vec<u8>,
    queue: VecDeque<u8>,
    info_hash: Vec<u8>,
    peer_id: String,
    remote_id: String,
    // writer: 
}

pub enum MessageType {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel =  8,
    Port = 9,
}