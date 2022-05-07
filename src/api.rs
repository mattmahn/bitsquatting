use actix_web::{web, HttpResponse, Responder};
use futures::future::join_all;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter::*};
use trust_dns_resolver::{
    name_server::{GenericConnection, GenericConnectionProvider, TokioRuntime},
    AsyncResolver, Name,
};

use crate::dns::{self, DomainData};

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

impl Iterator for BitFlipper {
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
pub struct MyInfo {
    domain_name: String,
}

#[derive(Debug, Serialize)]
struct MyResponse {
    domains: HashMap<String, Option<dns::DomainData>>,
}

pub async fn bitflip_html(
    hb: web::Data<Handlebars<'_>>,
    info: web::Form<MyInfo>,
    resolver: web::Data<AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>>,
) -> impl Responder {
    let name = Name::from_str_relaxed(&info.domain_name).unwrap();
    let resolver = resolver.get_ref();
    let flipper = BitFlipper::new(&name);

    let domains = HashMap::<String, Option<DomainData>>::from_iter(
        join_all(flipper.map(async move |d| {
            let new_name = d.to_utf8();
            let dd = dns::resolve_domain(resolver, d).await;
            (new_name, dd)
        }))
        .await,
    );

    HttpResponse::Ok().content_type(mime::TEXT_HTML_UTF_8).body(
        hb.render("domain_table.html", &MyResponse { domains })
            .unwrap(),
    )
}

pub async fn bitflip_json(
    info: web::Form<MyInfo>,
    resolver: web::Data<AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>>,
) -> impl Responder {
    let name = Name::from_str_relaxed(&info.domain_name).unwrap();
    let flipper = BitFlipper::new(&name);
    let mut ret = MyResponse {
        domains: HashMap::new(),
    };

    for d in flipper {
        let new_name = d.to_utf8();
        let dd = dns::resolve_domain(resolver.get_ref(), d).await;
        ret.domains.insert(new_name, dd);
    }

    HttpResponse::Ok().json(ret)
}
