mod core;

use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{
    App, HttpServer,
    http::header,
    middleware::{DefaultHeaders, from_fn},
    web,
};

use core::{
    config::config,
    handlers::send_crl,
    middleware::{
        RATE_LIMIT_BURST, RATE_LIMIT_PER_SEC, REQ_HEADER_TIMEOUT_SEC, header_size_mw,
        req_timeout_mw,
    },
    util::app_init,
};
use std::time::Duration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    app_init().await;
    let config = config();
    let governor_conf = GovernorConfigBuilder::default()
        .const_burst_size(RATE_LIMIT_BURST as u32)
        .const_requests_per_second(RATE_LIMIT_PER_SEC as u64)
        .finish()
        .unwrap();
    let server = HttpServer::new(move || {
        let def_headers = DefaultHeaders::new().add((header::X_CONTENT_TYPE_OPTIONS, "nosniff"));
        App::new()
            .wrap(def_headers)
            .wrap(from_fn(req_timeout_mw))
            .wrap(from_fn(header_size_mw))
            .wrap(Governor::new(&governor_conf))
            .app_data(web::PayloadConfig::new(0))
            .service(
                web::resource("/crl/{crl_file}")
                    .route(web::head().to(send_crl))
                    .route(web::get().to(send_crl)),
            )
    });
    let server = match config.num_workers {
        Some(w) => server.workers(w),
        None => server,
    };
    server
        .keep_alive(None)
        .client_request_timeout(Duration::from_secs(REQ_HEADER_TIMEOUT_SEC as u64))
        .bind(config.bind_addr())?
        .run()
        .await
}
