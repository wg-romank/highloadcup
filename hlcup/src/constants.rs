pub const MAX_DEPTH: u8 = 10;
pub const MAX_CONCURRENT_LICENSES: u8 = 10;
pub const N_WORKERS: u8 = 1;

pub const CONCURRENT_LICENSES: u8 = MAX_CONCURRENT_LICENSES / N_WORKERS;
pub const TIME_LIMIT_MS: u128 = 600 * 1000; // 1 minute

pub const AVG_DIG_MS: u128 = 2;
