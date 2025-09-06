use actix_web::{HttpRequest, http::header};
use flate2::{Compression, write::GzEncoder};
use openssl::sha::sha512;
use std::{
    io::Write,
    time::{Duration, Instant, SystemTime},
};
use x509_cert::{crl::TbsCertList, time::Time};

use super::{cache::init_crl_cache, config::init_config, database::init_db_pool};

pub async fn app_init() {
    init_config();
    init_crl_cache();
    init_db_pool().await;
}

pub fn duration_from_creation(
    created_at: Instant,
    next_update: Time,
    refresh_factor: f32,
) -> Option<Duration> {
    let nu_sys_time: SystemTime = next_update.into();
    nu_sys_time
        .duration_since(SystemTime::now())
        .ok()
        .and_then(|d| {
            Instant::now()
                .checked_add(d)
                .map(|i| i.duration_since(created_at).mul_f32(refresh_factor))
        })
}

#[inline]
pub fn crl_hash(der: &[u8]) -> String {
    hex::encode(sha512(der))
}

pub fn crl_expiration(
    TbsCertList {
        this_update,
        next_update,
        ..
    }: TbsCertList,
) -> (u32, SystemTime, SystemTime) {
    const DEF_MAX_AGE_SEC: u8 = 1;
    let now = SystemTime::now();
    let last_modified: SystemTime = this_update.into();
    next_update.map_or((DEF_MAX_AGE_SEC as u32, now, last_modified), |t| {
        let exp_date: SystemTime = t.into();
        let max_age = exp_date
            .duration_since(now)
            .map_or(DEF_MAX_AGE_SEC as u32, |d| d.as_secs() as u32);
        (max_age, exp_date, last_modified)
    })
}

#[inline]
pub fn parse_header(req: &HttpRequest, name: header::HeaderName) -> Option<&str> {
    req.headers().get(name).and_then(|h| h.to_str().ok())
}

#[inline]
pub fn trim_space_qoutes(value: &str) -> &str {
    value.trim_matches(|c: char| c == '"' || c.is_whitespace())
}

pub fn etag_matches(req: &HttpRequest, hash: &str) -> bool {
    parse_header(req, header::IF_NONE_MATCH).is_some_and(|tags| {
        tags.split(',').any(|t| {
            let tag = trim_space_qoutes(t);
            tag == "*" || tag == hash
        })
    })
}

pub fn since_matches(req: &HttpRequest, last_modified: SystemTime) -> bool {
    req.headers().get(header::IF_NONE_MATCH).is_none()
        && parse_header(req, header::IF_MODIFIED_SINCE)
            .and_then(|since| httpdate::parse_http_date(since).ok())
            .is_some_and(|since| last_modified <= since)
}

#[inline]
pub fn accepts_gzip(req: &HttpRequest) -> bool {
    parse_header(req, header::ACCEPT_ENCODING).is_some_and(|enc| enc.contains("gzip"))
}

#[inline]
pub fn gzip_der(der: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&der)?;
    encoder.finish()
}
