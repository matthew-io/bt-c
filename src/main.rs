mod bencoding;
mod tracker;
mod torrent;

use {bencoding::decoder, std::fs, torrent::{build_torrent, Torrent}};


fn main(){
    let file_data_result = fs::read("test.torrent").expect("couldnt read data");

    let file_data = match decoder::decode(&file_data_result) {
        Ok((bencode, _)) => {
            bencode
        }
        Err(e) => {
            panic!("error {}", e);
        }
    };

    let torrent_result= build_torrent(&file_data);
    let torrent = match torrent_result {
        Ok(t) => t,
        Err(e) => panic!("couldn't create torrent from bencode")
    };

    println!("{:#?}", torrent.info_hash);
}
