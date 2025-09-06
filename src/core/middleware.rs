use std::{fmt::Display, time::Duration};

use actix_web::{
    Error, HttpResponse,
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};
use tokio::time::timeout;

const MAX_HEADER_TOTAL_SIZE: u16 = 8192;
const MAX_HEADER_PER_SIZE: u16 = 2048;
const REQ_PROC_TIMEOUT_SEC: u8 = 2;
pub const REQ_HEADER_TIMEOUT_SEC: u8 = 3;
pub const RATE_LIMIT_BURST: u8 = 10;
pub const RATE_LIMIT_PER_SEC: u8 = 5;

#[derive(Debug)]
struct RequestTimeoutError;

impl Display for RequestTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Server took longer than 2 seconds to send response")
    }
}

impl actix_web::ResponseError for RequestTimeoutError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::RequestTimeout()
            .finish()
            .map_into_boxed_body()
    }
}

pub async fn header_size_mw(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let mut total_size: usize = 0;
    let res = HttpResponse::RequestHeaderFieldsTooLarge().finish();
    for (_, value) in req.headers().iter() {
        let header_len = value.as_bytes().len();
        if header_len > MAX_HEADER_PER_SIZE as usize {
            return Ok(req.into_response(res));
        }
        total_size += header_len;
        if total_size > MAX_HEADER_TOTAL_SIZE as usize {
            return Ok(req.into_response(res));
        }
    }
    next.call(req).await
}

pub async fn req_timeout_mw(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    timeout(
        Duration::from_secs(REQ_PROC_TIMEOUT_SEC as u64),
        next.call(req),
    )
    .await
    .unwrap_or_else(|_| Err(RequestTimeoutError.into()))
}
