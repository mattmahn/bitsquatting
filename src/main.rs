#![feature(async_closure)]

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use opentelemetry::{
    global, runtime::TokioCurrentThread, sdk::propagation::TraceContextPropagator,
};
use tracing_actix_web::TracingLogger;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use trust_dns_resolver::{config::*, AsyncResolver};

mod api;
mod dns;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("hey ;)")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Hello, world!");
    init_telemetry();

    let mut ns_group = NameServerConfigGroup::google();
    ns_group.merge(NameServerConfigGroup::cloudflare());
    let resolver = web::Data::new(
        AsyncResolver::tokio(
            ResolverConfig::from_parts(None, vec![], ns_group),
            ResolverOpts {
                cache_size: 1024,
                edns0: true,
                preserve_intermediates: false,
                use_hosts_file: false,
                ..ResolverOpts::default()
            },
        )
        .unwrap(),
    );

    let _srv = HttpServer::new(move || {
        // let response = resolver.lookup_ip("www.example.com.").await.unwrap();
        // println!("{:?}", response);

        App::new()
            .wrap(TracingLogger::default())
            .app_data(resolver.clone())
            .service(hello)
            .service(web::scope("/api").service(api::do_it))
    })
    .shutdown_timeout(10)
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn init_telemetry() {
    let app_name = "bitsquatting";

    global::set_text_map_propagator(TraceContextPropagator::new());
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(app_name)
        .install_batch(TokioCurrentThread)
        .expect("failed to install OpenTelemetry tracer");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let formatter_layer = BunyanFormattingLayer::new(app_name.into(), std::io::stdout);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    let subscriber = Registry::default()
        .with(env_filter)
        .with(telemetry)
        .with(JsonStorageLayer)
        .with(formatter_layer);
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to install tracing subscriber");
}
