mod bencoding;
mod tracker;
mod torrent;
mod protocol;
mod client;

use {
    bencoding::decoder,
    client::PieceManager,
    std::{error, fs, sync::Arc},
    torrent::{build_torrent, Torrent},
    tracker::Tracker,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let file_data_result = fs::read("debian-12.11.0-amd64-netinst.iso.torrent").expect("couldn't read data");

    let file_data = match decoder::decode(&file_data_result) {
        Ok((bencode, _)) => bencode,
        Err(e) => panic!("error: {}", e),
    };

    let torrent_result = build_torrent(&file_data);
    let torrent = match torrent_result {
        Ok(t) => t,
        Err(e) => panic!("couldn't create torrent from bencode: {}", e),
    };

    let torrent = Arc::new(torrent);

    let pm = PieceManager::new(torrent.clone())?;
    pm.print();

    let tracker = Tracker::new(torrent.clone());
    tracker.connect(true, 0, 0).await?;

    Ok(())
}
