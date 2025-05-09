use std::error;

use crate::torrent::Torrent;
use reqwest::Client;
use rand::{self, random, Rng};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};

pub struct Tracker {
    torrent: Torrent,
    peer_id: String,
    http_client: Client,
}

// helper function to generate random digits to create peer id
// to be sent to the tracker as a param
pub fn calculate_peer_id() -> String {
    let client_code = "-MY6969-";
    let mut rng = rand::rng();
    let random_digits: u64 = rng.random_range(1_000_000_000_000..=9_999_999_999_999);

    format!("{}{}", client_code, random_digits)
}


impl Tracker {
    pub fn new(torrent: Torrent) -> Tracker {
        Tracker {
            torrent,
            peer_id: calculate_peer_id(),
            http_client: Client::new(),
        }
    }

    pub async fn connect(&self, first: bool, uploaded: u64, downloaded: u64) -> Result<(), Box<dyn error::Error>> {
        let peer_id_encoded = percent_encode(self.peer_id.as_bytes(), NON_ALPHANUMERIC).to_string();
        let info_hash_encoded = percent_encode(&self.torrent.info_hash, NON_ALPHANUMERIC).to_string();

        let (uploaded_str, downloaded_str) = (uploaded.to_string(), downloaded.to_string());
        let left_str = (self.torrent.total_size - downloaded).to_string();

        let mut params = vec![
            ("info_hash", info_hash_encoded.as_str()), 
            ("peer_id", peer_id_encoded.as_str()), 
            ("port", "6889"), 
            ("uploaded", &uploaded_str),
            ("downloaded", &downloaded_str),     
            ("left", &left_str),
            ("compact", "1")
        ];

        if first {
            params.push(("event", "started"));
        };

        let res = self
            .http_client
            .get(&self.torrent.announce)
            .query(&params)
            .send()
            .await?;

        let body = res.text().await?;
        println!("tracker response: {}", body);

        Ok(())
    }
}

