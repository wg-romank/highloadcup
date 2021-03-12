use std::collections::{BTreeMap, HashSet};
use reqwest::StatusCode;
use tokio::sync::mpsc;
use histogram::Histogram;
use tokio::runtime::Runtime;


pub enum StatsMessage {
    ShowStats,
    RecordExplore {area_size: u64, duration: u64, status: Option<StatusCode> },
    RecordDig {depth: u8, duration: u64, found: bool, status: Option<StatusCode>},
    RecordCash {depth: u8, amount: u64, duration: u64, status: Option<StatusCode> },
    RecordLicense { duration: u64, status: Option<StatusCode> },
}

pub struct StatsHandler {
    pub tx: mpsc::Sender<StatsMessage>
}

impl StatsHandler {
    pub fn new(rt: &Runtime) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        rt.spawn(async move {
            StatsActor { stats: Stats::new(), rx: rx }.run().await
        });
        Self { tx }
    }
}

pub struct StatsActor {
    stats: Stats,
    rx: mpsc::Receiver<StatsMessage>,
}

impl StatsActor {
    pub async fn run(&mut self) {
        use StatsMessage::*;
        while let Some(msg) = self.rx.recv().await {
            match msg {
                ShowStats => println!("{}", self.stats),
                RecordExplore { area_size, duration, status } => self.stats.record_explore(area_size, duration, status),
                RecordDig { depth, duration, found, status } => self.stats.record_dig(duration, depth, found, status),
                RecordCash { duration, depth, amount, status } => self.stats.record_cash(duration, depth, amount, status),
                RecordLicense { duration, status } => self.stats.record_license(duration, status),
            }
        }
    }
}

pub struct Stats {
    total: f64,
    dig: EpMetric,
    dig_found: f64,
    dig_found_per_depth: BTreeMap<u8, (f64, f64)>,
    cash: EpMetric,
    cash_at_depth: EpMetric,
    license: EpMetric,
    explore: EpMetric,
}

pub struct EpMetric {
    total: f64,
    err: f64,
    err_codes: HashSet<String>,
    histograms: BTreeMap<u8, Histogram>,
}

impl EpMetric {
    fn new() -> EpMetric {
        EpMetric { total: 0., err: 0., err_codes: HashSet::new(), histograms: BTreeMap::new() }
    }

    fn inc(&mut self, map_key: u8, duration: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.histograms.entry(map_key).or_insert(Histogram::new()).increment(duration);
        // .map_err(|e| println!("hist err: {}", e));
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
        // print percentiles from the histogram
        for (depth, histogram) in self.histograms.iter() {
            write!(f, "({}) - percentiles: p50: {} ns p90: {} ns p99: {} ns p999: {}\n",
                   depth,
                   histogram.percentile(50.0).unwrap(),
                   histogram.percentile(90.0).unwrap(),
                   histogram.percentile(99.0).unwrap(),
                   histogram.percentile(99.9).unwrap(),
            )?;
            write!(f, "({}) - latency (ns): Min: {} Avg: {} Max: {} StdDev: {}\n",
                   depth,
                   histogram.minimum().unwrap(),
                   histogram.mean().unwrap(),
                   histogram.maximum().unwrap(),
                   histogram.stddev().unwrap(),
            )?;
        }
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
        let dig_stats: String = self.dig_found_per_depth
            .iter()
            .map(|(k, v)| format!("{}:{:.3}", k, v.1 / v.0))
            .collect::<Vec<String>>().join(", ");
        write!(f, "rate at depth {}\n", dig_stats)?;

        write!(f, "cash: {}", self.cash)?;
        write!(f, "cash at depth: {}\n", self.cash_at_depth)?;

        write!(f, "license: {}", self.license)
    }
}

impl Stats {
    fn new() -> Stats { Stats {
        total: 0.,
        dig: EpMetric::new(),
        dig_found: 0.,
        dig_found_per_depth: BTreeMap::new(),
        cash: EpMetric::new(),
        cash_at_depth: EpMetric::new(),
        license: EpMetric::new(),
        explore: EpMetric::new(),
    } }

    fn record_dig(&mut self, duration: u64, depth: u8, found: bool, err: Option<StatusCode>) {
        self.total += 1.;
        self.dig.inc(depth, duration, err);

        self.dig_found_per_depth.entry(depth).or_insert((0., 0.)).0 += 1.;
        if found {
            self.dig_found += 1.;
            self.dig_found_per_depth.entry(depth).or_insert((0., 0.)).1 += 1.;
        }
    }

    fn record_cash(&mut self, duration: u64, depth: u8, amount: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.cash.inc(depth, duration, err);
        self.cash_at_depth.inc(depth, amount, err);
    }

    fn record_license(&mut self, duration: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.license.inc(0, duration, err);
    }

    fn record_explore(&mut self, area_size: u64, duration: u64, err: Option<StatusCode>) {
        self.total += 1.;
        self.explore.inc(if area_size == 1 { 1 } else { 2 }, duration, err);
    }
}
