use std::{collections::BTreeMap, error, sync::Arc, time};
use crate::{bencoding::{self, Bencode}, torrent::Torrent};
use reqwest::{Client, Response};
use rand::{self, Rng};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};

pub struct Tracker {
    torrent: Arc<Torrent>,
    peer_id: String,
    http_client: Client,
}

pub struct TrackerResponse {
    pub failure: String,
    pub interval: u32,
    pub complete: u64,
    pub incomplete: u64,
    pub peers: Vec<(String, u16)>,
}

impl TrackerResponse {
    fn parse_peers(data: &[u8]) -> Result<Vec<(String, u16)>, Box<dyn error::Error>> {
        if data.len() % 6 != 0 {
            return Err("peers field length is not a multiple of 6".into());
        }

        let mut result = Vec::new();
        for chunk in data.chunks(6) {
            let ip = format!(
                "{}.{}.{}.{}",
                chunk[0], chunk[1], chunk[2], chunk[3]
            );
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            result.push((ip, port));
        }

        Ok(result)
    }
    

    // parses the response from tracker and returns a TrackerResponse
    pub async fn new(response: Response) -> Result<TrackerResponse, Box<dyn error::Error>> {
        // converts the response to bytes to be decoded
        let bytes = response.bytes().await?;
        println!("{:#?}", bytes);
        
        // decodes the bytes into bencode format
        let bencode = bencoding::decoder::decode(&bytes)?;

        // gets the top level dictionary from the bencoded response
        let dict = match bencode {
            (Bencode::Dict(d), _) => d,
            _ => return Err("tracker response does not contain a top-level dictionary".into()),
        };

        // gets the response failure reason (if applicable, defaults to an empty string if not).
        let failure = dict
        .get(&b"failure reason"[..])
        .and_then(|b| match b {
            Bencode::Bytes(bytes) => Some(String::from_utf8(bytes.clone()).ok()?),
            _ => None,
        })
        .unwrap_or_default();
        
        // gets the tracker request interval in seconds
        let interval = match dict.get(&b"interval"[..]) {
            Some(Bencode::Int(i)) => i.clone() as u32,
            _ => return Err("couldn't get interval:(".into()),
        };


        // gets the number of peers within the entire file (i.e. seeders)
        let complete = match dict.get(&b"complete"[..]) {
            Some(Bencode::Int(i)) => i.clone() as u64,
            _ => 0,
        };

        // gets the number of non-seeding peers within the entire file (i.e. leechers)
        let incomplete = match dict.get(&b"incomplete"[..]) {
            Some(Bencode::Int(i)) => i.clone() as u64,
            _ => 0,
        };

        // gets the compact peer list as a byte string (each peer is 6 bytes: 4 IP + 2 port)
        let raw_peers = match dict.get(&b"peers"[..]) {
            Some(Bencode::Bytes(b)) => b.clone(),
            _ => return Err("couldn't get peers dict from tracker response".into()),
        };

        let peers = Self::parse_peers(&raw_peers)?;


        Ok(TrackerResponse { failure, interval,complete, incomplete, peers: peers })
    }

    // print formatted tracker response data
    pub fn print(self) {
        println!(
            "Failure reason (if applicable): {}. \n Interval: {}. Complete: {}. Incomplete: {}.", self.failure, self.interval, self.complete, self.incomplete
        );

        println!("peer list:");
        for (ip, port) in self.peers {
            println!(" {}:{} ", ip, port); 
        }
    }
}

// helper function to generate random digits to create peer id
pub fn calculate_peer_id() -> String {
    let client_code = "-MY6969-";
    let mut rng = rand::rng();
    let random_digits: u64 = rng.random_range(100_000_000_000..=999_999_999_999);
    format!("{}{}", client_code, random_digits)
}



impl Tracker {
    pub fn new(torrent: Arc<Torrent>) -> Tracker {
        Tracker {
            torrent,
            peer_id: calculate_peer_id(),
            http_client: Client::new(),
        }
    }

    // connects to the tracker for the given torrent
    pub async fn connect(&self, first: bool, uploaded: u64, downloaded: u64) -> Result<(), Box<dyn error::Error>> {
        let info_hash_param = self.torrent.info_hash.iter()
            .map(|&byte| format!("%{:02X}", byte))
            .collect::<String>();
        
        let (uploaded_str, downloaded_str) = (uploaded.to_string(), downloaded.to_string());
        let left_str = (self.torrent.total_size - downloaded).to_string();
        
        // builds query in bittorrent specific format. 
        // see here for formatting details: https://wiki.theory.org/BitTorrentSpecification#Tracker_HTTP/HTTPS_Protocol
        let mut query = format!(
            "?info_hash={}&peer_id={}&port=6889&uploaded={}&downloaded={}&left={}&compact=1",
            info_hash_param,
            self.peer_id,
            uploaded_str,
            downloaded_str,
            left_str
        );
        
        // if this is our first request add that to the query
        if first {
            query.push_str("&event=started");
        }
        
        // build formatted query string
        let url = format!("{}{}", self.torrent.announce, query);
        
        // get response from the tracker
        let res = self.http_client
            .get(&url)
            .timeout(time::Duration::from_secs(10))
            .send()
            .await?;
        
        // if the response was successful, build a TrackerResponse from it
        if res.status().is_success() {
            match TrackerResponse::new(res).await {
                Ok(tracker_res) => tracker_res.print(),
                Err(e) => println!("couldn't create tracker response: {}", e),
            }
        // if not, print error to console
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
