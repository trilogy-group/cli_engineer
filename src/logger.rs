use simplelog::{Config, LevelFilter, SimpleLogger};

pub fn init(verbose: bool) {
    let level = if verbose { LevelFilter::Info } else { LevelFilter::Warn };
    let _ = SimpleLogger::init(level, Config::default());
}
