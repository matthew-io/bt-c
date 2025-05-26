use std::collections::VecDeque;
use std::error::Error;
use std::thread::JoinHandle;
use tokio::io::{BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::client::PieceManager;

// in version 1.0 of the bittorrent protocol the 
// handshake message has a length of 68
// it has the following format:
// <pstrlen><pstr><reserved><info_hash>peer_id>
// pstrlen = 19, pstr = "BitTorrent protocol"
// thus the length is 49 + 19

const HANDSHAKE_LENGTH: usize = 49 + 19; 


pub struct PeerConnection {
    state: Vec<u8>,
    peer_state: Vec<u8>,
    queue: VecDeque<u8>,
    info_hash: Vec<u8>,
    peer_id: String,
    remote_id: String,
    reader: Option<BufReader<OwnedReadHalf>>,
    writer: Option<BufWriter<OwnedWriteHalf>>,
    piece_manager: PieceManager,
    future: Option<JoinHandle<()>>
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

pub struct Handshake {
    info_hash: Vec<u8>,
    peer_id: Vec<u8>
}

impl Handshake {
    // create new handshake from peer id and info hash
    pub fn new(info_hash: Vec<u8>, peer_id: Vec<u8>) -> Result<Handshake, Box<dyn Error>> {
        if info_hash.len() != 20 {
            return Err("info hash is not of the correct length!".into())
        }

        if peer_id.len() != 20 {
            return Err("peer id is not of the correct length!".into())
        }

        Ok(Handshake {
            info_hash,
            peer_id
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(68);
        buf.push(19); // pstrlen
        buf.extend_from_slice(b"BitTorrent protocol"); // pstr
        buf.extend_from_slice(&[0u8; 8]); // reserved bytes
        buf.extend_from_slice(&self.info_hash); // info hash
        buf.extend_from_slice(&self.peer_id); // peer _id
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Handshake, Box<dyn Error>> {
        if data.len() != HANDSHAKE_LENGTH {
            return Err("invalid handshake length".into());
        }

        let pstrlen = data[0];
        if pstrlen != 19 {
            return Err("invalid pstrlen".into())
        }

        let pstr = &data[1..20];
        if pstr != b"BitTorrent protocol" {
            return Err("invalid protocol string".into());
        }

        let info_hash = data[28..48].to_vec();
        let peer_id = data[48..68].to_vec();

        Handshake::new(info_hash, peer_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_encode_decode() {
        let info_hash = vec![0xAB; 20];
        let peer_id = b"-MY6969-123456789012".to_vec();

        let handshake = Handshake::new(info_hash.clone(), peer_id.clone()).unwrap();
        let encoded = handshake.encode();
        let decoded = Handshake::decode(&encoded).unwrap();

        assert_eq!(decoded.info_hash, info_hash);
        assert_eq!(decoded.peer_id, peer_id);
    }

    #[test]
    fn test_handshake_decode_invalid_length() {
        let invalid_data = vec![0u8; 67];
        let result = Handshake::decode(&invalid_data);
        assert!(result.is_err());
    }

}