use crate::http::dto::*;
use crate::models::data::Treasure;
use futures::stream::FuturesUnordered;
use futures::{Future, FutureExt, StreamExt};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BinaryHeap;

use std::time::Instant;

use reqwest::Error;
use tokio::sync::mpsc;

use crate::models::messages::StatsMessage;
use crate::models::messages::StatsMessage::*;

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
    async fn send_stats(&self, message: StatsMessage) {
        self.stats_handler
            .send(message)
            .await
            .expect("failed to send stats");
    }

    async fn call<Payload: Serialize, Response: DeserializeOwned + Default>(
        &self,
        endpoint: &str,
        payload: &Payload,
        stats_success: impl Fn(&Response, u64) -> StatsMessage,
        stats_failure: impl Fn(Option<StatusCode>, u64) -> StatsMessage,
        error_info: Option<String>,
    ) -> ClientResponse<Response> {
        let now = Instant::now();
        let response = self.client.post(endpoint).json(payload).send().await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                let res = response.json::<Response>().await?;
                self.send_stats(stats_success(&res, elapsed)).await;
                Ok(res)
            }
            reqwest::StatusCode::NOT_FOUND => {
                self.send_stats(stats_failure(None, elapsed)).await;
                Ok(Response::default())
            }
            status => {
                self.send_stats(stats_failure(Some(status), elapsed)).await;
                let error_text = response.text().await?;
                Err(DescriptiveError::new(
                    endpoint,
                    status,
                    error_info.map(|s| s + &error_text).unwrap_or(error_text),
                ))
            }
        }
    }

    pub async fn explore(&self, area: &Area) -> ClientResponse<Explore> {
        self.call(
            &self.explore_url,
            area,
            |_, elapsed| RecordExplore {
                area_size: area.size(),
                duration: elapsed,
                status: None,
            },
            |status, elapsed| RecordExplore {
                area_size: area.size(),
                duration: elapsed,
                status,
            },
            None,
        )
        .await
    }

    pub async fn get_license(&self, coins: Vec<u64>) -> ClientResponse<License> {
        let l = coins.len() as u64;
        self.call(
            &self.licenses_url,
            &coins,
            |lic: &License, elapsed| RecordLicense {
                duration: elapsed,
                coins: l,
                allowed: lic.dig_allowed,
                status: None,
            },
            |status, elapsed| RecordLicense {
                duration: elapsed,
                coins: l,
                allowed: 0,
                status,
            },
            None,
        )
        .await
    }

    pub async fn dig(&self, dig: &Dig) -> ClientResponse<Vec<String>> {
        self.call(
            &self.dig_url,
            dig,
            |_, elapsed| RecordDig {
                depth: dig.depth,
                x: dig.pos_x,
                y: dig.pos_y,
                found: true,
                duration: elapsed,
                status: None,
            },
            |status, elapsed| RecordDig {
                depth: dig.depth,
                x: dig.pos_x,
                y: dig.pos_y,
                found: false,
                duration: elapsed,
                status,
            },
            Some(format!("{} {} {}", dig.pos_x, dig.pos_y, dig.depth)),
        )
        .await
    }

    pub async fn cash(&self, depth: u8, treasure: String) -> ClientResponse<Vec<u64>> {
        self.call(
            &self.cash_url,
            &treasure,
            |coins: &Vec<u64>, elapsed| RecordCash {
                amount: coins.len() as u64,
                depth,
                duration: elapsed,
                status: None,
            },
            |status, elapsed| RecordCash {
                amount: 0,
                depth,
                duration: elapsed,
                status,
            },
            None,
        )
        .await
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

fn claim_treasure(client: &Client, t: Treasure) -> Vec<impl Future<Output = Vec<u64>>> {
    let depth = t.depth;
    t.treasures
        .into_iter()
        .map(|tt| {
            let cl = client.clone();
            tokio::spawn(async move { cl.plain_cash(depth, tt).await })
                .map(|r| r.ok().unwrap_or_default())
        })
        .collect()
}

fn claim_treasures(
    client: &Client,
    treasures: &mut BinaryHeap<Treasure>,
) -> FuturesUnordered<impl Future<Output = Vec<u64>>> {
    treasures
        .drain()
        .flat_map(|t| claim_treasure(client, t))
        .collect()
}

pub async fn claim_all(client: &Client, treasures: &mut BinaryHeap<Treasure>) -> Vec<u64> {
    let cc = claim_treasures(client, treasures)
        .collect::<Vec<Vec<u64>>>()
        .await;
    cc.into_iter().flatten().collect::<Vec<u64>>()
}
