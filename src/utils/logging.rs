use dotenvy::dotenv;
use log::{info, LevelFilter};
use std::env;

pub fn init_logging(log_level: LevelFilter) {
    let crate_name = env!("CARGO_PKG_NAME");
    env::set_var("RUST_LOG", format!("{}={}", crate_name, log_level));
    env_logger::init();

    dotenv().ok();
}

#[macro_export]
macro_rules! development_info {
    ($($arg:tt)*) => {
        if std::env::var("APP_ENV").unwrap_or("production".to_string()) != "production" {
            println!("\x1b[32m[INFO]\x1b[0m {}", format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! development_info_with_separator {
($($arg:tt)*) => {
        development_info!($($arg)*);
        println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    };
}
