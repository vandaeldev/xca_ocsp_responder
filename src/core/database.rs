use anyhow::Result;
use der::{Decode, pem::Base64Decoder};
use sqlx::{Sqlite, SqlitePool, query_scalar};
use tokio::sync::{OnceCell, SetError};
use x509_cert::crl::CertificateList;

use super::config::config;

static DB_POOL: OnceCell<SqlitePool> = OnceCell::const_new();

#[inline]
fn to_cert_list(raw_crl: Option<String>) -> Result<Option<CertificateList>> {
    if raw_crl.is_none() {
        return Ok(None);
    }
    let mut buf: Vec<u8> = vec![];
    Base64Decoder::new(raw_crl.unwrap().as_bytes())
        .expect("Could not instantiate base64 decoder")
        .decode_to_end(&mut buf)
        .expect("Failed to decode input bytes");
    buf.shrink_to_fit();
    CertificateList::from_der(&buf)
        .map_err(|e| e.into())
        .map(|c| Some(c))
}

pub async fn fetch_crl(name: &str) -> Result<Option<CertificateList>> {
    query_scalar::<Sqlite, String>(
        "SELECT crl FROM view_crls WHERE name = ? ORDER BY date DESC LIMIT 1;",
    )
    .bind(name)
    .fetch_optional(db_pool())
    .await
    .map_err(|e| e.into())
    .and_then(to_cert_list)
}

pub async fn init_db_pool() {
    let path = &config().db_path;
    let pool = SqlitePool::connect(path)
        .await
        .expect(format!("Could not connect to database at '{path}'").as_str());
    _ = DB_POOL.set(pool).or_else(|_| {
        println!("Database pool already has been initialized");
        Ok::<(), SetError<SqlitePool>>(())
    });
}

pub fn db_pool() -> &'static SqlitePool {
    &DB_POOL
        .get()
        .expect("Database pool has not been initialized")
}
