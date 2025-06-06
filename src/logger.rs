use log::LevelFilter;
use simplelog::{SimpleLogger, Config};
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Utc;

pub fn init(verbose: bool) {
    let level = if verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Warn
    };
    let _ = SimpleLogger::init(level, Config::default());
}

pub fn init_with_file_logging(verbose: bool) {
    let level = if verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Warn
    };
    
    // Create log filename with timestamp
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let log_filename = format!("cli_engineer_{}.log", timestamp);
    
    // Initialize console logger
    let _ = SimpleLogger::init(level, Config::default());
    
    // Log the start of the session to file
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_filename)
    {
        let _ = writeln!(file, "\n=== CLI Engineer Session Started: {} ===", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
        log::info!("Verbose logging enabled. Session details will be logged to: {}", log_filename);
    }
}
