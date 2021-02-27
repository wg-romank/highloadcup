use crate::dto::*;

use reqwest::Error;
use reqwest::StatusCode;
use std::collections::HashSet;

pub struct Client {
    client: reqwest::Client,
    explore_url: String,
    licenses_url: String,
    dig_url: String,
    cash_url: String,
    pub stats: Stats,
}

pub struct Stats {
    total: f64,
    dig: EpMetric,
    dig_found: f64,
    cash: EpMetric,
    license: EpMetric,
    explore: EpMetric,
}

pub struct EpMetric {
    total: f64,
    err: f64,
    err_codes: HashSet<String>,
}

impl EpMetric {
    fn new() -> EpMetric {
        EpMetric { total: 0., err: 0., err_codes: HashSet::new() }
    }

    fn inc(&mut self, err: Option<StatusCode>) {
        self.total += 1.;
        match err {
            Some(status) => {
                self.err += 1.;
                self.err_codes.insert(status.to_string());
            }
            None => ()
        }
    }
}

impl std::fmt::Display for EpMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} / {}, error rate {:.3}\n", self.total, self.err, self.err / self.total)?;
        if !self.err_codes.is_empty() {
            write!(f, "codes {}\n", self.err_codes.clone().into_iter().collect::<Vec<String>>().join("|"))?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "total: {}\n", self.total)?;
        write!(f, "explore: {}", self.explore)?;
        write!(f, "digs: {}found {}, found rate {}\n", self.dig, self.dig_found, self.dig_found / self.dig.total)?;
        write!(f, "cash: {}", self.cash)?;
        write!(f, "license: {}", self.license)
    }
}

impl Stats {
    fn new() -> Stats { Stats {
        total: 0.,
        dig: EpMetric::new(),
        dig_found: 0.,
        cash: EpMetric::new(),
        license: EpMetric::new(),
        explore: EpMetric::new(),
    } }

    fn record_dig(&mut self, found: bool, err: Option<StatusCode>) {
        self.total += 1.;
        self.dig.inc(err);

        if found {
            self.dig_found += 1.
        }
    }

    fn record_cash(&mut self, err: Option<StatusCode>) {
        self.total += 1.;
        self.cash.inc(err);
    }

    fn record_license(&mut self, err: Option<StatusCode>) {
        self.total += 1.;
        self.license.inc(err);
    }

    fn record_explore(&mut self, err: Option<StatusCode>) {
        self.total += 1.;
        self.explore.inc(err);
    }
}

impl Client {
    pub fn new(address: &str) -> Client {
        let client = reqwest::Client::new();
        let base_url = format!("http://{}:8000", address);
        println!("Base url {}", base_url);
        Client {
            client,
            explore_url: base_url.clone() + "/explore",
            licenses_url: base_url.clone() + "/licenses",
            dig_url: base_url.clone() + "/dig",
            cash_url: base_url.clone() + "/cash",
            stats: Stats::new()
        }
    }
}

pub type ClientResponse<T> = Result<T, DescriptiveError>;

#[derive(Debug)]
pub struct DescriptiveError {
    message: String
}

impl DescriptiveError {
    fn new(endpoint: &str, status_code: reqwest::StatusCode, message: String) -> DescriptiveError {
        DescriptiveError {
            message: format!("{} /{}: {}", status_code, endpoint, message)
        }
    }
}

impl std::convert::From<Error> for DescriptiveError {
    fn from(e: Error) -> Self {
        DescriptiveError { message: format!("{}", e) }
    }
}

impl std::fmt::Display for DescriptiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "err: {}\n", &self.message)
    }
}

impl Client {
    pub async fn explore(&mut self, area: &Area) -> ClientResponse<Explore> {
        let response = self.client.post(&self.explore_url)
                .json(area)
                .send()
                .await?;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_explore(None);
                Ok(response.json::<Explore>().await?)
            },
            status => {
                self.stats.record_explore(Some(status));
                Err(DescriptiveError::new("explore",status, response.text().await?))
            },
        }
    }

    pub async fn get_license(&mut self, coins: Vec<u64>) -> ClientResponse<License> {
        let response = self.client.post(&self.licenses_url)
            .json(&coins)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_license(None);
                Ok(response.json::<License>().await?)
            },
            status => {
                self.stats.record_license(Some(status));
                Err(DescriptiveError::new("license",status, response.text().await?))
            },
        }

    }

    pub async fn dig(&mut self, dig: &Dig) -> ClientResponse<Vec<String>> {
        let response = self.client.post(&self.dig_url)
            .json(dig)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_dig(true, None);
                Ok(response.json::<Vec<String>>().await?)
            },
            reqwest::StatusCode::NOT_FOUND => {
                self.stats.record_dig(false, None);
                Ok(vec![])
            },
            status => {
                self.stats.record_dig(false, Some(status));
                Err(DescriptiveError::new(
                    "dig",
                    status,
                    format!("{} {} {}", dig.pos_x, dig.pos_y, dig.depth) + &response.text().await?))
            },
        }
    }

    pub async fn cash(&mut self, treasure: String) -> ClientResponse<Vec<u64>> {
        let response = self.client.post(&self.cash_url)
            .json(&treasure)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_cash(None);
                Ok(response.json::<Vec<u64>>().await?)
            },
            status => {
                self.stats.record_cash(Some(status));
                Err(DescriptiveError::new("cash",status, response.text().await?))
            },
        }
    }
}
