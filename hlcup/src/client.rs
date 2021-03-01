use crate::dto::*;

use reqwest::Error;
use reqwest::StatusCode;
use std::collections::{HashSet, HashMap};
use histogram::Histogram;
use std::time::Instant;

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
    dig_found_per_depth: HashMap<u8, (f64, f64)>,
    cash: EpMetric,
    cash_found_per_depth: HashMap<u8, u64>,
    license: EpMetric,
    explore: EpMetric,
}

pub struct EpMetric {
    total: f64,
    err: f64,
    err_codes: HashSet<String>,
    histogram: Histogram,
}

impl EpMetric {
    fn new() -> EpMetric {
        EpMetric { total: 0., err: 0., err_codes: HashSet::new(), histogram: Histogram::new() }
    }

    fn inc(&mut self, duration: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.histogram.increment(duration)
            .map_err(|e| println!("hist err: {}", e));
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
        write!(f, "{} / {}, error rate {:.3}", self.total, self.err, self.err / self.total)?;

        write!(f, "\n- percentiles:")?;
        self.histogram.percentile(50.0).map(|r| { write!(f, "p50:  {} ms ", r as f64 / 100.) });
        self.histogram.percentile(90.0).map(|r| { write!(f, "p90:  {} ms ", r as f64 / 100.) });
        self.histogram.percentile(99.0).map(|r| { write!(f, "p99:  {} ms ", r as f64 / 100.) });
        self.histogram.percentile(99.9).map(|r| { write!(f, "p999: {} ms ", r as f64 / 100.) });

        write!(f, "\n- latency (ms)")?;
        self.histogram.minimum().map(|r| write!(f, "min: {} ", r as f64 / 100.));
        self.histogram.maximum().map(|r| write!(f, "max: {} ", r as f64 / 100.));
        self.histogram.mean().map(|r| write!(f, "mean: {} ", r as f64 / 100.));
        self.histogram.stddev().map(|r| write!(f, "std: {} ", r as f64 / 100.));
        self.histogram.mean().map(|r| write!(f, "\n- cumm (s): {:.3}", r as f64 * self.total / 1000. / 1000.));

        if !self.err_codes.is_empty() {
            write!(f, " - err codes {}\n", self.err_codes.clone().into_iter().collect::<Vec<String>>().join("|"))?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "total: {}\n", self.total)?;
        write!(f, "/explore: {}", self.explore)?;

        write!(f, "/dig: {} (*) found {}, found rate {:.3}\n", self.dig, self.dig_found, self.dig_found / self.dig.total)?;
        write!(f, " (*) rate at depth {}\n",
               Stats::format_hm(&self.dig_found_per_depth, |v| format!("{:.3}", v.1 / v.0)))?;

        write!(f, "/cash: {}", self.cash)?;
        write!(f, " (*) cash at depth {}\n",
               Stats::format_hm(&self.cash_found_per_depth, |v| v.to_string()))?;

        write!(f, "/license: {}", self.license)
    }
}

impl Stats {
    fn new() -> Stats { Stats {
        total: 0.,
        dig: EpMetric::new(),
        dig_found: 0.,
        dig_found_per_depth: HashMap::new(),
        cash: EpMetric::new(),
        cash_found_per_depth: HashMap::new(),
        license: EpMetric::new(),
        explore: EpMetric::new(),
    } }

    fn format_hm<T>(hm: &HashMap<u8, T>, f: fn(&T) -> String) -> String {
        let mut res = hm.iter()
            .map(|(k, v)| (*k, format!("{}:{}", k, f(v))))
            .collect::<Vec<(u8, String)>>();
        res.sort_by(|a, b| a.0.cmp(&b.0));
        res.into_iter().map(|(_, b)| b).collect::<Vec<String>>().join(", ")
    }

    fn record_dig(&mut self, duration: u64, depth: u8, found: bool, err: Option<StatusCode>) {
        self.total += 1.;
        self.dig.inc(duration, err);

        self.dig_found_per_depth.entry(depth).or_insert((0., 0.)).0 += 1.;
        if found {
            self.dig_found += 1.;
            self.dig_found_per_depth.entry(depth).or_insert((0., 0.)).1 += 1.;
        }
    }

    fn record_cash(&mut self, duration: u64, depth: u8, amount: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.cash.inc(duration, err);
        *self.cash_found_per_depth.entry(depth).or_insert(0) += amount;
    }

    fn record_license(&mut self, duration: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.license.inc(duration, err);
    }

    fn record_explore(&mut self, duration: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.explore.inc(duration, err);
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
    pub message: String
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
        let now = Instant::now();
        let response = self.client.post(&self.explore_url)
                .json(area)
                .send()
                .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_explore(elapsed, None);
                Ok(response.json::<Explore>().await?)
            },
            status => {
                self.stats.record_explore(elapsed, Some(status));
                Err(DescriptiveError::new("explore",status, response.text().await?))
            },
        }
    }

    pub async fn get_license(&mut self, coins: Vec<u64>) -> ClientResponse<License> {
        let now = Instant::now();
        let response = self.client.post(&self.licenses_url)
            .json(&coins)
            .send()
            .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_license(elapsed, None);
                Ok(response.json::<License>().await?)
            },
            status => {
                self.stats.record_license(elapsed, Some(status));
                Err(DescriptiveError::new("license",status, response.text().await?))
            },
        }

    }

    pub async fn dig(&mut self, dig: &Dig) -> ClientResponse<Vec<String>> {
        let now = Instant::now();
        let response = self.client.post(&self.dig_url)
            .json(dig)
            .send()
            .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                self.stats.record_dig(elapsed, dig.depth, true, None);
                Ok(response.json::<Vec<String>>().await?)
            },
            reqwest::StatusCode::NOT_FOUND => {
                self.stats.record_dig(elapsed, dig.depth, false, None);
                Ok(vec![])
            },
            status => {
                self.stats.record_dig(elapsed, dig.depth, false, Some(status));
                Err(DescriptiveError::new(
                    "dig",
                    status,
                    format!("{} {} {}", dig.pos_x, dig.pos_y, dig.depth) + &response.text().await?))
            },
        }
    }

    pub async fn cash(&mut self, depth: u8, treasure: String) -> ClientResponse<Vec<u64>> {
        let now = Instant::now();
        let response = self.client.post(&self.cash_url)
            .json(&treasure)
            .send()
            .await?;
        let elapsed = now.elapsed().as_micros() as u64;

        match response.status() {
            reqwest::StatusCode::OK => {
                let coins = response.json::<Vec<u64>>().await?;
                self.stats.record_cash(elapsed, depth, coins.len() as u64, None);
                Ok(coins)
            },
            status => {
                self.stats.record_cash(elapsed, depth, 0, Some(status));
                Err(DescriptiveError::new("cash",status, response.text().await?))
            },
        }
    }
}
