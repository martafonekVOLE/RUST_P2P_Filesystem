use log::{debug, error, info, warn, LevelFilter};
use std::env;

pub fn init_logging(debug_mode: bool) {
    let log_level = if debug_mode {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    env::set_var("RUST_LOG", format!("my_crate={}", log_level));
    env_logger::init();
}

pub fn log_error(message: &str) {
    error!("{}", message);
}

pub fn log_warn(message: &str) {
    warn!("{}", message);
}

pub fn log_info(message: &str) {
    info!("{}", message);
}

pub fn log_debug(message: &str) {
    debug!("{}", message);
}
