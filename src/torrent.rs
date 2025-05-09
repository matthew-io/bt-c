use std::collections::BTreeMap;
use sha1::{Digest, Sha1};

use crate::bencoding::{encoder, Bencode};

// file struct for single file torrents. 
// TODO: implement multi-file struct for multi file torrents

#[derive(Debug)]
pub struct File {
    pub name: String,
    length: u64,
}

// this is bad gems but i cba rewriting this 
// TODO: should maybe just have public functions that
// return the values and get rid of the torrent 
// struct entirely. but this will do.

pub struct Torrent {
    pub info_hash: Vec<u8>,
    pub announce: String,
    pub multi_file: bool,
    pub piece_length: u32,
    pub total_size: u64,
    pub pieces: Vec<u8>,
    pub output_file: String,
    pub files: Vec<File>,
}

// get the sha1 hash of the bencode of the info dict
// for sending to the tracker as a param
pub fn get_sha1_info_hash(bencode: &Bencode) -> Result<Vec<u8>, String> {
    let encoded = encoder::encode(bencode);
    
    let mut hasher = Sha1::new();
    hasher.update(&encoded);
    Ok(hasher.finalize().to_vec())
}

// torrent building function that takes in bencoded
// data and extracts the most relevant data, returning a torrent struct
pub fn build_torrent(bencode: &Bencode) -> Result<Torrent, String> {
    let dict = match bencode {
        Bencode::Dict(d) => d,
        _ => return Err("top level bencode is not a dict".to_string()),
    };

    let announce = match dict.get(&b"announce"[..]) {
        Some(Bencode::Bytes(b)) => String::from_utf8(b.clone()).map_err(|e| e.to_string())?,
        _ => return Err("couldn't find announce url".to_string()),
    };

    let info = match dict.get(&b"info"[..]) {
        Some(Bencode::Dict(d)) => d,
        _ => return Err("couldn't find info dict".to_string()),
    };

    let info_bencode = match dict.get(&b"info"[..]) {
        Some(info @ Bencode::Dict(_)) => info,
        _ => return Err("cant find info bencode".to_string()),
    };

    let name = match info.get(&b"name"[..]) {
        Some(Bencode::Bytes(b)) => String::from_utf8(b.clone()).map_err(|e| e.to_string())?,
        _ => return Err("couldn't get name field from info dict".to_string())
    };

    let length = match info.get(&b"length"[..]) {
        Some(Bencode::Int(i)) => *i as u64,
        _ => return Err("couldn't get length field from info dict".to_string())
    };

    let piece_length = match info.get(&b"piece length"[..])  {
        Some(Bencode::Int(i)) => *i as u32,
        _ => return Err("couldn't find pieces length".to_string())
    };

    let pieces = match info.get(&b"pieces"[..]) {
        Some(Bencode::Bytes(b)) => b.to_vec(),
        _ => return Err("couldn't get pieces from info dict".to_string()),
    };

    let file = File {
        name: name.clone(),
        length,
    };

    Ok(Torrent {
        info_hash: get_sha1_info_hash(info_bencode)?,
        announce, 
        multi_file: false,
        piece_length,
        total_size: length,
        pieces,
        output_file: name,
        files: vec![file]
    })
}
