use crate::dto::*;

use std::time::Instant;

use reqwest::Error;
use tokio::sync::mpsc;

use crate::stats::StatsMessage;
use crate::stats::StatsMessage::{RecordCash, RecordDig, RecordExplore, RecordLicense};

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
    explore_url: String,
    licenses_url: String,
    dig_url: String,
    cash_url: String,
    stats_handler: mpsc::Sender<StatsMessage>,
}

impl Client {
    pub fn new(address: &str, stats_handler: mpsc::Sender<StatsMessage>) -> Client {
        let client = reqwest::Client::new();
        let base_url = format!("http://{}:8000", address);
        println!("Base url {}", base_url);
        Client {
            client,
            explore_url: base_url.clone() + "/explore",
            licenses_url: base_url.clone() + "/licenses",
            dig_url: base_url.clone() + "/dig",
            cash_url: base_url + "/cash",
            stats_handler,
        }
    }
}

pub type ClientResponse<T> = Result<T, DescriptiveError>;

#[derive(Debug)]
pub struct DescriptiveError {
    pub message: String,
}

impl DescriptiveError {
    fn new(endpoint: &str, status_code: reqwest::StatusCode, message: String) -> DescriptiveError {
        DescriptiveError {
            message: format!("{} /{}: {}", status_code, endpoint, message),
        }
    }
}

impl std::convert::From<Error> for DescriptiveError {
    fn from(e: Error) -> Self {
        DescriptiveError {
            message: format!("{}", e),
        }
    }
}

impl std::fmt::Display for DescriptiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "err: {}", &self.message)
    }
}

impl Client {
    pub async fn explore(&self, area: &Area) -> ClientResponse<Explore> {
        let now = Instant::now();
        let response = self
            .client
            .post(&self.explore_url)
            .json(area)
            .send()
            .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats_handler
                    .send(RecordExplore {
                        area_size: area.size(),
                        duration: elapsed,
                        status: None,
                    })
                    .await;
                Ok(response.json::<Explore>().await?)
            }
            status => {
                self.stats_handler
                    .send(RecordExplore {
                        area_size: area.size(),
                        duration: elapsed,
                        status: Some(status),
                    })
                    .await;
                Err(DescriptiveError::new(
                    "explore",
                    status,
                    response.text().await?,
                ))
            }
        }
    }

    pub async fn get_license(&self, coins: Vec<u64>) -> ClientResponse<License> {
        let now = Instant::now();
        let l = coins.len() as u64;
        let response = self
            .client
            .post(&self.licenses_url)
            .json(&coins)
            .send()
            .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                let lic = response.json::<License>().await?;
                self.stats_handler
                    .send(RecordLicense {
                        duration: elapsed,
                        coins: l,
                        allowed: lic.dig_allowed,
                        status: None,
                    })
                    .await;
                Ok(lic)
            }
            status => {
                self.stats_handler
                    .send(RecordLicense {
                        duration: elapsed,
                        coins: l,
                        allowed: 0,
                        status: Some(status),
                    })
                    .await;
                Err(DescriptiveError::new(
                    "license",
                    status,
                    response.text().await?,
                ))
            }
        }
    }

    pub async fn dig(&self, dig: &Dig) -> ClientResponse<Vec<String>> {
        let now = Instant::now();
        let response = self.client.post(&self.dig_url).json(dig).send().await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats_handler
                    .send(RecordDig {
                        depth: dig.depth,
                        x: dig.pos_x,
                        y: dig.pos_y,
                        found: true,
                        duration: elapsed,
                        status: None,
                    })
                    .await;
                Ok(response.json::<Vec<String>>().await?)
            }
            reqwest::StatusCode::NOT_FOUND => {
                self.stats_handler
                    .send(RecordDig {
                        depth: dig.depth,
                        x: dig.pos_x,
                        y: dig.pos_y,
                        found: false,
                        duration: elapsed,
                        status: None,
                    })
                    .await;
                Ok(vec![])
            }
            status => {
                self.stats_handler
                    .send(RecordDig {
                        depth: dig.depth,
                        x: dig.pos_x,
                        y: dig.pos_y,
                        found: false,
                        duration: elapsed,
                        status: Some(status),
                    })
                    .await;
                Err(DescriptiveError::new(
                    "dig",
                    status,
                    format!("{} {} {}", dig.pos_x, dig.pos_y, dig.depth) + &response.text().await?,
                ))
            }
        }
    }

    pub async fn cash(&self, depth: u8, treasure: String) -> ClientResponse<Vec<u64>> {
        let now = Instant::now();
        let response = self
            .client
            .post(&self.cash_url)
            .json(&treasure)
            .send()
            .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                let coins = response.json::<Vec<u64>>().await?;
                self.stats_handler
                    .send(RecordCash {
                        amount: coins.len() as u64,
                        depth,
                        duration: elapsed,
                        status: None,
                    })
                    .await;
                Ok(coins)
            }
            status => {
                self.stats_handler
                    .send(RecordCash {
                        amount: 0,
                        depth,
                        duration: elapsed,
                        status: Some(status),
                    })
                    .await;
                Err(DescriptiveError::new(
                    "cash",
                    status,
                    response.text().await?,
                ))
            }
        }
    }
}

impl Client {
    pub async fn plain_cash(&self, depth: u8, treasure: String) -> Vec<u64> {
        loop {
            if let Ok(coins) = self.cash(depth, treasure.clone()).await {
                break coins;
            }
        }
    }

    pub async fn plain_license(&self, coins: Vec<u64>) -> License {
        loop {
            if let Ok(lic) = self.get_license(coins.clone()).await {
                break lic;
            }
        }
    }
}
