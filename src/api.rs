use actix_web::{post, web, Responder, HttpResponse};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter};
use trust_dns_resolver::{
    name_server::{GenericConnection, GenericConnectionProvider, TokioRuntime},
    AsyncResolver, Name,
};

use crate::dns;

#[derive(Debug)]
struct BitFlipper {
    bytes: Vec<u8>,
    vector_pos: usize,
    byte_pos: u8,
}

impl BitFlipper {
    fn new(domain: &Name) -> Self {
        Self {
            bytes: domain.to_lowercase().to_ascii().into_bytes(),
            vector_pos: 0,
            byte_pos: 0,
        }
    }
}

impl iter::Iterator for BitFlipper {
    type Item = Name;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.vector_pos >= self.bytes.len() - 1 {
                // exhausted the whole byte string
                return None;
            } else if self.byte_pos >= 8 {
                // reached the end of a byte
                // go to next byte
                self.vector_pos += 1;
                // reset to 0th-bit position
                self.byte_pos = 0;
            }

            // println!("{:?}", self);

            let mut bytes = self.bytes.clone();
            bytes[self.vector_pos] = bytes[self.vector_pos] ^ (1 << self.byte_pos);
            self.byte_pos += 1; // move to next bit position in a byte

            if let Ok(new_str) = String::from_utf8(bytes) {
                if let Ok(bit_flipped_name) = Name::from_str_relaxed(new_str) {
                    return Some(bit_flipped_name);
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct MyInfo {
    domain_name: String,
}

#[derive(Debug, Serialize)]
struct MyResponse {
    domains: HashMap<String, Option<dns::DomainData>>,
}

#[post("/bitflip")]
async fn do_it(
    info: web::Json<MyInfo>,
    resolver: web::Data<AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>>,
) -> impl Responder {
    let name = Name::from_str_relaxed(&info.domain_name).unwrap();
    let flipper = BitFlipper::new(&name);
    let mut ret = MyResponse {
        domains: HashMap::new()
    };

    for d in flipper {
        let new_name = d.to_utf8();
        let dd  = dns::resolve_domain(resolver.get_ref(), d).await;
        ret.domains.insert(new_name, dd);
    }

    HttpResponse::Ok().json(ret)
}
