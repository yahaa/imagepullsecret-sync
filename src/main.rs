#[macro_use]
extern crate log;

mod worker;

use kube::Client;

use chrono::Local;
use std::{io::Write, sync::Arc};
use tokio::signal::unix::{signal, SignalKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::Builder::from_env(env_logger::Env::default())
        .format(|buf, record| {
            let level = { buf.default_styled_level(record.level()) };
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                level,
                record.module_path().unwrap_or("<unnamed>"),
                record.line().unwrap_or(0),
                &record.args()
            )
        })
        .init();

    let client = Client::try_default().await?;

    let worker = Arc::new(worker::SyncWorker::new(
        client,
        "default",
        "docker-registry-configs",
    ));

    let watch_ns = worker.clone();
    let watch_cfg = worker.clone();

    tokio::spawn(async move {
        if let Err(e) = watch_ns.watch_ns().await {
            panic!("sync worker watch ns err: {}", e);
        }
    });
    tokio::spawn(async move {
        if let Err(e) = watch_cfg.watch_cfg_secret().await {
            panic!("sync worker watch config secret err: {}", e);
        }
    });

    signal(SignalKind::terminate())?.recv().await;

    info!("recv SIGTERM, graceful shutdown...");

    Ok(())
}
