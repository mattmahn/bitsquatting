#![feature(async_closure)]
#![feature(option_result_contains)]

use actix_files::Files;
use actix_web::{
    dev::Service,
    get,
    http::header::{self, CacheDirective, HeaderName, HeaderValue},
    middleware,
    web::{self},
    App, HttpResponse, HttpServer, Responder,
};
use actix_web_lab::guard::Acceptable;
use handlebars::Handlebars;
use opentelemetry::global;
use serde_json::json;
use tracing::info;
use tracing_actix_web::TracingLogger;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use trust_dns_resolver::{config::*, AsyncResolver};

mod api;
mod dns;

#[get("/")]
async fn hello(hb: web::Data<Handlebars<'_>>) -> impl Responder {
    let data = json!({
        "name": "Alice"
    });
    let body = hb.render("index.html", &data).unwrap();
    HttpResponse::Ok()
        .insert_header(header::CacheControl(vec![
            CacheDirective::Public,
            CacheDirective::Extension("stale-while-revalidate".to_string(), Some("60".to_string())),
            CacheDirective::Extension("stale-if-error".to_string(), Some("86400".to_string())),
            CacheDirective::MaxAge(300),
        ]))
        .content_type(mime::TEXT_HTML_UTF_8)
        .body(body)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Hello, world!");
    init_telemetry();

    let mut ns_group = NameServerConfigGroup::google();
    ns_group.merge(NameServerConfigGroup::cloudflare());
    let nameserver_count = ns_group.len();
    let resolver = web::Data::new(
        AsyncResolver::tokio(
            ResolverConfig::from_parts(None, vec![], ns_group),
            ResolverOpts {
                cache_size: 8192,
                edns0: true,
                ip_strategy: LookupIpStrategy::Ipv4AndIpv6,
                num_concurrent_reqs: nameserver_count * 2,
                preserve_intermediates: false,
                timeout: std::time::Duration::from_secs(3),
                use_hosts_file: false,
                ..ResolverOpts::default()
            },
        )
        .unwrap(),
    );

    let mut handlebars = Handlebars::new();
    handlebars
        .register_templates_directory(".hbs", "./templates")
        .unwrap();
    let hb_ref = web::Data::new(handlebars);

    let addr = "[::]:8080";
    let srv = HttpServer::new(move || {
        // TODO add CORS headers

        App::new()
            .wrap(TracingLogger::default())
            .wrap(middleware::Compress::default())
            .wrap_fn(|req, srv| {
                let fut = srv.call(req);
                async {
                    let mut res = fut.await?;
                    res.headers_mut().insert(
                        header::CONTENT_SECURITY_POLICY,
                        HeaderValue::from_static("default-src 'self'; img-src 'self' https://samherbert.net/svg-loaders/svg-loaders/tail-spin.svg; script-src 'self' 'unsafe-inline' https://unpkg.com/htmx.org@1.3.3; style-src 'self' 'unsafe-inline' https://unpkg.com/normalize.css@8.0.1/normalize.css; report-uri https://mattmahnke.report-uri.com/r/d/csp/wizard; report-to default"),
                    );
                    res.headers_mut().insert(
                        HeaderName::from_static("report-to"),
                        HeaderValue::from_static(r#"{"group":"default","max_age":31536000,"endpoints":[{"url":"https://mattmahnke.report-uri.com/a/d/g"}],"include_subdomains":true}"#),
                    );
                    Ok(res)
                }
            })
            .app_data(resolver.clone())
            .app_data(hb_ref.clone())
            .service(hello)
            .service(Files::new("/public", "./public"))
            .service(
                web::scope("/api")
                    .route(
                        "/bitflip",
                        web::post()
                            .guard(Acceptable::new(mime::TEXT_HTML))
                            .to(api::bitflip_html),
                    )
                    .route("/bitflip", web::post().to(api::bitflip_json)),
            )
    })
    .shutdown_timeout(10)
    .bind(addr)?
    .run();
    info!("server listening on {}", addr);

    srv.await?;

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn init_telemetry() {
    let app_name = "bitsquatting";

    // global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(app_name)
        .install_batch(opentelemetry::runtime::TokioCurrentThread)
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
