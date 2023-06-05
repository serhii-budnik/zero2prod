use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use tracing_actix_web::TracingLogger;
use crate::routes::{health_check, subscribe};
use sqlx::PgPool;
use std::net::TcpListener;

pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    let db_pool = web::Data::new(db_pool);

    let server = HttpServer::new(move || { 
        App::new()
            .wrap(TracingLogger::default())
            .route("/", web::get().to(health_check))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(db_pool.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
