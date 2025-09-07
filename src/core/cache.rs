use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use anyhow::Result;
use bytes::Bytes;
use der::Encode;
use moka::{Expiry, sync::Cache};
use x509_cert::crl::CertificateList;

use super::{
    config::{Config, config},
    util::{crl_hash, duration_from_creation, gzip_der},
};

const CACHE_REFRESH_FACTOR: f32 = 0.95;

static CRL_CACHE: OnceLock<CrlCache> = OnceLock::new();

struct CrlExpPolicy {
    max_ttl: Duration,
}

impl CrlExpPolicy {
    pub fn new(max_ttl_sec: u32) -> Self {
        Self {
            max_ttl: Duration::from_secs(max_ttl_sec as u64),
        }
    }
}

impl Expiry<String, CrlCacheEntry> for CrlExpPolicy {
    #[inline(always)]
    fn expire_after_create(
        &self,
        _key: &String,
        value: &CrlCacheEntry,
        created_at: Instant,
    ) -> Option<Duration> {
        value
            .crl
            .tbs_cert_list
            .next_update
            .map_or(Some(self.max_ttl), |t| {
                duration_from_creation(created_at, t, CACHE_REFRESH_FACTOR)
                    .map(|d| d.min(self.max_ttl))
            })
    }
}

#[derive(Clone)]
pub struct CrlCacheEntry {
    pub crl: CertificateList,
    pub der: Bytes,
    pub der_gz: Bytes,
    pub hash: String,
}

impl CrlCacheEntry {
    pub fn from_crl(crl: CertificateList) -> Result<Self> {
        let der: Bytes = crl.to_der()?.into();
        let der_gz: Bytes = gzip_der(&der)?.into();
        let hash = crl_hash(&der);
        Ok(CrlCacheEntry {
            crl,
            der,
            der_gz,
            hash,
        })
    }
}

pub struct CrlCache {
    cache: Cache<String, CrlCacheEntry>,
}

impl CrlCache {
    pub fn new(capacity: u64, max_ttl_sec: u32) -> Self {
        let cache = Cache::builder()
            .max_capacity(capacity)
            .expire_after(CrlExpPolicy::new(max_ttl_sec))
            .build();
        Self { cache }
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<CrlCacheEntry> {
        self.cache.get(key)
    }

    #[inline]
    pub fn insert(&self, key: &str, crl_entry: CrlCacheEntry) {
        self.cache.insert(key.to_string(), crl_entry);
    }
}

#[inline]
pub fn init_crl_cache() {
    let Config {
        cache_cap,
        max_cache_ttl,
        ..
    } = config();
    _ = CRL_CACHE
        .set(CrlCache::new(*cache_cap, *max_cache_ttl))
        .or_else(|_| {
            println!("CrlCache already has been initialized");
            Ok::<(), CrlCache>(())
        });
}

#[inline]
pub fn crl_cache() -> &'static CrlCache {
    &CRL_CACHE.get().expect("Cache has not been initialized")
}
