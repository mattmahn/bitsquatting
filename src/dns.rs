use serde::Serialize;
use std::net::IpAddr;
use trust_dns_resolver::{
    name_server::{GenericConnection, GenericConnectionProvider, TokioRuntime},
    AsyncResolver, Name,
};

#[derive(Debug, Serialize)]
pub struct DomainData {
    ips: Vec<IpAddr>,
}

#[tracing::instrument]
pub async fn resolve_domain(
    resolver: &AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>,
    domain: Name,
) -> Option<DomainData> {
    let ips = resolver.lookup_ip(domain).await;

    ips.map(|ips| {
        Some(DomainData {
            ips: ips.iter().collect(),
        })
    })
    .unwrap_or(None)
}
