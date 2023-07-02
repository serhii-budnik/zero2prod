use actix_web::HttpResponse;
use actix_web::http::header::LOCATION;

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}
