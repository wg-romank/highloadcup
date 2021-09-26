use reqwest::StatusCode;
use tokio::sync::oneshot;

use crate::http::dto::License;
use crate::models::data::Treasures;

#[derive(Debug)]
pub enum MessageForAccounting {
    TreasureToClaim(Treasures),
    GetLicense(oneshot::Sender<Option<License>>),
    LicenseExpired(u64),
}

#[derive(Debug)]
pub enum StatsMessage {
    ShowStats,
    RecordExplore {
        area_size: u64,
        duration: u64,
        status: Option<StatusCode>,
    },
    RecordDig {
        depth: u8,
        x: u64,
        y: u64,
        duration: u64,
        found: bool,
        status: Option<StatusCode>,
    },
    RecordCash {
        depth: u8,
        amount: u64,
        duration: u64,
        status: Option<StatusCode>,
    },
    RecordLicense {
        duration: u64,
        coins: u64,
        allowed: u8,
        status: Option<StatusCode>,
    },
}
