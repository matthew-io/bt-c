
use std::collections::BTreeMap;

const LIST_INDICATOR: u8 = b'l';
const INT_INDICATOR: u8 = b'i';
const DICT_INDICATOR: u8 = b'd';
const BYTES_INDICATOR: std::ops::RangeInclusive<u8> = b'0'..=b'9';

#[derive(Debug)]
pub enum Bencode {
    Int(i64),
    List(Vec<Bencode>),
    Bytes(Vec<u8>),
    Dict(BTreeMap<Vec<u8>, Bencode>)
}

pub mod encoder {
    use super::Bencode;

    pub fn encode(data: &Bencode) -> Vec<u8> {
        match data {
            Bencode::Int(i) => format!("i{}e", i).into_bytes(),
            Bencode::Bytes(b) => [b.len().to_string().into_bytes(), vec![b':'], b.clone()].concat(),
            Bencode::List(items) => {
                let mut v = vec![b'l'];
                for item in items {
                    v.extend(encode(item))
                }
                v.push(b'e');
                v
            }
            Bencode::Dict(map) => {
                let mut v = vec![b'd'];
                for (key, value) in map {
                    v.extend(encode(&Bencode::Bytes(key.clone())));
                    v.extend(encode(value));
                }
                v.push(b'e');
                v
            }
        }
    }
}


pub mod decoder {
    use super::{Bencode, BYTES_INDICATOR, DICT_INDICATOR, INT_INDICATOR, LIST_INDICATOR};
    use std::{collections::BTreeMap, num::ParseIntError};

    fn parse_int(input: &[u8]) -> Result<(Bencode, &[u8]), String> {
        let string = String::from_utf8_lossy(input);
        let end = string.find("e").ok_or("couldn't find end of input string")?;
        
        let num_str = &string[..end];
    
        if num_str.starts_with("0") && num_str.len() > 2 || num_str.starts_with("-0") && num_str.len() > 2 {
            return Err("input string has leading zero".to_string())
        }
    
        let number = num_str.parse::<i64>().map_err(|e| e.to_string())?;
        let remaining = &input[end + 1..];
    
        Ok((Bencode::Int(number), remaining))
    }
    
    fn parse_bytes(input: &[u8]) -> Result<(Bencode, &[u8]), String> {
        let colon_pos = input.iter().position(|&x| x == b':').ok_or("couldn't find colon in input")?;
        let len_bytes = &input[..colon_pos];
        let len_str = String::from_utf8_lossy(len_bytes);
        let length = len_str.parse::<usize>().map_err(|e: ParseIntError| e.to_string())?;
    
        let start = colon_pos + 1;
        let end = start + length;
    
        if input.len() < end {
            return Err("byte string is shorted than expected".to_string());
        }
    
        let bytes = input[start..end].to_vec();
        let remaining = &input[end..];
        
        Ok((Bencode::Bytes(bytes), remaining))  
    }   
    
    fn parse_list(mut input: &[u8]) -> Result<(Bencode, &[u8]), String> {
        let mut items = Vec::new();
    
        while let Some(&b) = input.first() {
            if b == b'e' {
                return Ok((Bencode::List(items), &input[1..]));
            }
    
            let (item, rest) = decode(input)?;
            items.push(item);
            input = rest;
        }
    
        Err("unterminated list (missing e)".to_string())
    }
    
    fn parse_dict(mut input: &[u8]) -> Result<(Bencode, &[u8]), String> {
        let mut map:BTreeMap<Vec<u8>, Bencode> = BTreeMap::new();
    
        while let Some(&b) = input.first() {
            if b == b'e' {
                return Ok((Bencode::Dict(map), &input[1..]));
            }
    
            let (key_bencode, rest) = parse_bytes(input)?;
            let key = match key_bencode {
                Bencode::Bytes(k) => k,
                _ => return Err("dictionary key is not a byte string".to_string()),
            };
            input = rest;
    
            let (value, rest) = decode(input)?;
            input = rest;
    
            map.insert(key, value);
        }
    
        Err("unterminated dictionary (missing e)".to_string())
    }
    
    pub fn decode(input: &[u8]) -> Result<(Bencode, &[u8]), String> {
        match input.first() {
            Some(&INT_INDICATOR) => parse_int(&input[1..]),
            Some(&LIST_INDICATOR) => parse_list(&input[1..]),
            Some(&DICT_INDICATOR) => parse_dict(&input[1..]),
            Some(&b) if BYTES_INDICATOR.contains(&b) => parse_bytes(input),
            _ => Err("invalid bencode type".to_string())
        }
    }
}