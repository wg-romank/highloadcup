mod accounting;
mod client;
mod constants;
mod dto;
mod model;
mod stats;
mod util;
mod worker;

use crate::accounting::Accounting;
use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use std::time::Instant;

use client::Client;

use dto::*;

use crate::constants::N_WORKERS;
use crate::stats::{StatsActor, StatsMessage};
use crate::util::Handler;
use worker::Worker;

async fn _main(client: Client, started: Instant, areas: Vec<Area>) {
    let mk_accounting = Accounting::new(&client);
    let accounting_handle = Handler::new(mk_accounting);

    Worker::new(client.clone(), started, areas, accounting_handle)
        .await
        .run()
        .await
}

fn spawn_tasks(
    n_workers: u64,
    client: &Client,
    w: u64,
    h: u64,
    started: Instant,
) -> FuturesUnordered<impl Future<Output = ()>> {
    (0..n_workers)
        .map(|i| {
            let client = client.clone();
            let area = Area {
                pos_x: w * i,
                pos_y: 0,
                size_x: w,
                size_y: h,
            };
            _main(
                client,
                started,
                area.divide().iter().flat_map(|a| a.divide()).collect(),
            )
        })
        .collect::<FuturesUnordered<_>>()
}

#[tokio::main(worker_threads = 1)]
async fn main() {
    let n_workers = N_WORKERS as u64;
    let started = Instant::now();

    println!("Started threads = {}", n_workers);

    let address = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let stats_hanlder = Handler::new(StatsActor::new);
    let client = Client::new(&address, stats_hanlder.tx.clone());

    let w = 3500 / n_workers;
    let h = 3500;

    tokio::select! {
        _ = spawn_tasks(n_workers, &client, w, h, started).collect::<_>() => (),
        res = tokio::signal::ctrl_c() => {
            if res.is_ok() {
                stats_hanlder.tx.send(StatsMessage::ShowStats).await
                    .expect("failed to request showing stats");
            }
        }
    };
}
