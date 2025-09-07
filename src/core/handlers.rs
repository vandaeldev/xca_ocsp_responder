use actix_web::{
    HttpRequest, HttpResponse, Responder,
    http::header::{self, ContentEncoding},
    web::Path,
};

use super::{
    cache::{CrlCacheEntry, crl_cache},
    database::fetch_crl,
    util::{accepts_gzip, crl_expiration, etag_matches, since_matches},
};

pub async fn send_crl(crl_file: Path<String>, req: HttpRequest) -> impl Responder {
    let crl_name = crl_file.into_inner();
    let cache = crl_cache();
    let crl_result = if let Some(cached) = cache.get(&crl_name) {
        Ok(Some(cached))
    } else {
        fetch_crl(&crl_name)
            .await
            .and_then(|crl_opt| match crl_opt {
                Some(crl) => {
                    let crl_entry = CrlCacheEntry::from_crl(crl)?;
                    cache.insert(&crl_name, crl_entry.clone());
                    Ok(Some(crl_entry))
                }
                None => Ok(None),
            })
    };
    match crl_result {
        Ok(Some(crl_entry)) => {
            let (max_age, exp_date, last_modified) = crl_expiration(crl_entry.crl.tbs_cert_list);
            if etag_matches(&req, &crl_entry.hash) || since_matches(&req, last_modified) {
                return HttpResponse::NotModified().finish();
            };
            let mut res = HttpResponse::Ok();
            let res = res
                .content_type("application/pkix-crl")
                .insert_header(header::CacheControl(vec![
                    header::CacheDirective::Public,
                    header::CacheDirective::MustRevalidate,
                    header::CacheDirective::NoTransform,
                    header::CacheDirective::MaxAge(max_age),
                ]))
                .insert_header(header::Expires(exp_date.into()))
                .insert_header(header::ETag(header::EntityTag::new_strong(crl_entry.hash)))
                .insert_header(header::LastModified(last_modified.into()));
            if accepts_gzip(&req) {
                let res = res.insert_header((header::CONTENT_ENCODING, ContentEncoding::Gzip));
                res.body(crl_entry.der_gz)
            } else {
                res.body(crl_entry.der)
            }
        }
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
