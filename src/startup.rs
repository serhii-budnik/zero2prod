use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use crate::routes::{health_check, subscribe};
use std::net::TcpListener;

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| { 
        App::new()
            .route("/", web::get().to(health_check))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
