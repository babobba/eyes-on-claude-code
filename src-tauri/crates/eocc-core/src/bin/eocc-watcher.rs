use eocc_core::watcher::{self, WatcherConfig};
use std::sync::mpsc;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = WatcherConfig::default();
    log::info!("eocc-watcher: watching {:?}", config.eocc_dir);

    // The stop channel is never sent to — the watcher runs until the process is killed
    let (_stop_tx, stop_rx) = mpsc::channel();
    watcher::run(config, stop_rx);
}
