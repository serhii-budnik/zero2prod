use zero2prod::configuration::get_configuration;
use zero2prod::issue_delivery_worker::run_worker_until_stopped;
use zero2prod::idempotency_key_worker;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

use std::fmt::{Debug, Display};
use tokio::task::JoinError;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = Application::build(configuration.clone()).await?;

    let application_task = tokio::spawn(application.run_until_stopped());
    let issue_delivery_worker = tokio::spawn(run_worker_until_stopped(configuration.clone()));
    let idempotency_key_worker = tokio::spawn(idempotency_key_worker::run_worker_until_stopped(configuration));

    tokio::select! {
        o = application_task => report_exit("API", o),
        o = issue_delivery_worker => report_exit("Background worker (Issue delivery)", o),
        o = idempotency_key_worker => report_exit("Background worker (Expire idempotency key)", o),
    }
    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                erorr.message = %e,
                "{} failed",
                task_name,
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name,
            )
        }
    }
}
