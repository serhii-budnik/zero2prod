use crate::{
    configuration::Settings,
    idempotency::remove_stale_idempontecy_keys,
    startup::get_connection_pool,
};

use sqlx::PgPool;
use std::time::Duration;

const TEN_MINUTES: u64 = 60 * 10;

pub async fn run_worker_until_stopped(
    configuration: Settings
) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);

    worker_loop(connection_pool).await
}

async fn worker_loop(
    pool: PgPool,
) -> Result<(), anyhow::Error> {
    loop {
        remove_stale_idempontecy_keys(&pool).await?;
        tokio::time::sleep(Duration::from_secs(TEN_MINUTES)).await;
    }
}
