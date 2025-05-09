use std::{error, time};
use crate::{bencoding::{self, Bencode}, torrent::Torrent};
use reqwest::{Client, Response};
use rand::{self, Rng};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};

pub struct Tracker {
    torrent: Torrent,
    peer_id: String,
    http_client: Client,
}

pub struct TrackerResponse {
    response: Response,
}

impl TrackerResponse {
    pub fn new(response: Response) -> TrackerResponse {
        TrackerResponse {
            response
        }
    }
}

// Helper function to generate random digits to create peer id
pub fn calculate_peer_id() -> String {
    let client_code = "-MY6969-";
    let mut rng = rand::rng();
    let random_digits: u64 = rng.random_range(100_000_000_000..=999_999_999_999);
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
        println!("info hash: {:#?}", self.torrent.info_hash);
        let info_hash_param = self.torrent.info_hash.iter()
            .map(|&byte| format!("%{:02X}", byte))
            .collect::<String>();
        
        let (uploaded_str, downloaded_str) = (uploaded.to_string(), downloaded.to_string());
        let left_str = (self.torrent.total_size - downloaded).to_string();
        
        let mut query = format!(
            "?info_hash={}&peer_id={}&port=6889&uploaded={}&downloaded={}&left={}&compact=1",
            info_hash_param,
            self.peer_id,
            uploaded_str,
            downloaded_str,
            left_str
        );
        
        if first {
            query.push_str("&event=started");
        }
        
        let url = format!("{}{}", self.torrent.announce, query);
        
        let res = self.http_client
            .get(&url)
            .timeout(time::Duration::from_secs(10))
            .send()
            .await?;
        
        if res.status().is_success() {
            let tracker_res = TrackerResponse::new(res);
        } else {
            println!("error response from tracker: {} {}", res.status(), res.status().as_str());
            
            match res.text().await {
                Ok(error_text) => println!("error: {}", error_text),
                Err(_) => println!("couldn't get error details")
            }
        }
        
        Ok(())
    }
    
}
