use log::LevelFilter;
use std::env;

pub fn init_logging(log_level: LevelFilter) {
    let crate_name = env!("CARGO_PKG_NAME");
    env::set_var("RUST_LOG", format!("{}={}", crate_name, log_level));
    env_logger::init();
}
