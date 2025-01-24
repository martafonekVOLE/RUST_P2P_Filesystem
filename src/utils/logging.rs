use log::{debug, error, info, warn, LevelFilter};
use std::env;

pub fn init_logging(log_level: LevelFilter) {
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
