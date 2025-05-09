mod bencoding;
mod tracker;
mod torrent;

use {bencoding::decoder, std::error, std::fs, torrent::{build_torrent, Torrent}, tracker::Tracker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
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

    let tracker = Tracker::new(torrent);
    tracker.connect(true, 0, 0).await?;
    Ok(())
}
