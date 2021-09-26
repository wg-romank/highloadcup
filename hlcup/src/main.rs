mod http;
mod actors;
mod constants;
mod models;

#[cfg(test)]
mod tests;

use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use std::time::Instant;
use tokio::sync::mpsc;

use crate::models::messages::{MessageForAccounting, StatsMessage};
use crate::http::client::Client;
use crate::http::dto::Area;
use crate::actors::accounting::Accounting;
use crate::actors::stats::{StatsActor};
use crate::actors::Handler;
use crate::actors::worker::Worker;

#[derive(Clone)]
pub struct Rules {
    pub w: u64,
    pub h: u64,
    pub n_workers: u64,
    max_concurrent_licenses: u8,
    pub max_depth: u8,
}

impl Rules {
    pub fn new(n_workers: u64) -> Self {
        Self { w: 3500 / n_workers, h: 3500, n_workers, max_concurrent_licenses: 7, max_depth: 10 }
    }
}

async fn task(
    client: Client,
    rules: Rules,
    accounting_handle: mpsc::Sender<MessageForAccounting>,
    started: Instant,
    areas: Vec<Area>
) {
    Worker::new(client, rules, started, areas, accounting_handle)
        .await
        .run()
        .await
}

fn spawn_tasks(
    rules: Rules,
    client: Client,
    accounting_handle: mpsc::Sender<MessageForAccounting>,
    started: Instant,
) -> FuturesUnordered<impl Future<Output = ()>> {
    println!("Started threads = {}", rules.n_workers);

    (0..rules.n_workers)
        .map(|i| {
            let area = Area::initial_stripe(rules.w, rules.h, i);
            task(client.clone(), rules.clone(), accounting_handle.clone(), started, area.split_in_8())
        })
        .collect::<FuturesUnordered<_>>()
}

#[tokio::main]
async fn main() {
    let n_workers = std::env::var("WORKERS")
        .expect("missing env variable WORKERS")
        .parse::<u64>()
        .expect("malformed WORKERS variable");

    let rules = Rules::new(n_workers);
    let started = Instant::now();

    let address = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let stats_hanlder = Handler::new(StatsActor::new);
    let client = Client::new(&address, stats_hanlder.tx.clone());

    let mk_accounting = Accounting::new(&client, rules.max_concurrent_licenses);
    let accounting_handle = Handler::new(mk_accounting);

    tokio::select! {
        _ = spawn_tasks(rules, client, accounting_handle.tx, started).collect::<_>() => (),
        res = tokio::signal::ctrl_c() => {
            if res.is_ok() {
                stats_hanlder.tx.send(StatsMessage::ShowStats).await
                    .expect("failed to request showing stats");
            }
        }
    };
}
