mod accounting;
mod client;
mod constants;
mod dto;
mod model;
mod stats;
mod util;
mod worker;

use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use std::time::Instant;

use crate::client::Client;
use crate::dto::*;
use crate::accounting::Accounting;
use crate::stats::{StatsActor, StatsMessage};
use crate::util::Handler;
use crate::worker::Worker;

#[derive(Clone)]
pub struct Rules {
    pub w: u64,
    pub h: u64,
    pub n_workers: u64,
    max_concurrent_licenses: u8,
    pub max_depth: u8,
}

impl Rules {
    pub fn new() -> Self {
        Self { w: 3500, h: 3500, n_workers: 1, max_concurrent_licenses: 10, max_depth: 10 }
    }

    pub fn concurrent_licenses(&self) -> u8 {
        (self.max_concurrent_licenses as u64 / self.n_workers) as u8
    }
}

impl Default for Rules {
    fn default() -> Self { Self::new() }
}

async fn task(client: Client, rules: Rules, started: Instant, areas: Vec<Area>) {
    let mk_accounting = Accounting::new(&client, rules.concurrent_licenses());
    let accounting_handle = Handler::new(mk_accounting);

    Worker::new(client, rules, started, areas, accounting_handle)
        .await
        .run()
        .await
}

fn spawn_tasks(
    rules: Rules,
    client: &Client,
    started: Instant,
) -> FuturesUnordered<impl Future<Output = ()>> {
    println!("Started threads = {}", rules.n_workers);

    (0..rules.n_workers)
        .map(|i| {
            let area = Area::initial_stripe(rules.w, rules.h, i);
            task(client.clone(), rules.clone(), started, area.split_in_8())
        })
        .collect::<FuturesUnordered<_>>()
}

#[tokio::main]
async fn main() {
    let rules = Rules::default();
    let started = Instant::now();

    let address = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let stats_hanlder = Handler::new(StatsActor::new);
    let client = Client::new(&address, stats_hanlder.tx.clone());

    tokio::select! {
        _ = spawn_tasks(rules, &client, started).collect::<_>() => (),
        res = tokio::signal::ctrl_c() => {
            if res.is_ok() {
                stats_hanlder.tx.send(StatsMessage::ShowStats).await
                    .expect("failed to request showing stats");
            }
        }
    };
}
