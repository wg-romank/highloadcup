mod accounting;
mod client;
mod constants;
mod dto;
mod model;
mod stats;
mod worker;
mod util;

use futures::{Future, StreamExt};
use futures::stream::FuturesUnordered;
use crate::accounting::Accounting;
use std::time::Instant;

use tokio::runtime;

use client::Client;

use dto::*;

use crate::constants::N_WORKERS;
use crate::util::Handler;
use crate::stats::{StatsMessage, StatsActor};
use tokio::time::Duration;
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
) -> FuturesUnordered<impl Future<Output=()>> {
    (0..n_workers).map(|i| {
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
    }).collect::<FuturesUnordered<_>>()
}

#[tokio::main(worker_threads = 1)]
async fn main() {
    let n_workers = N_WORKERS as u64;
    let started = Instant::now();

    println!("Started thread = {}", n_workers);

    let address = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let stats_hanlder = Handler::new(StatsActor::new);
    let client = Client::new(&address, stats_hanlder.tx.clone());

    // todo: nicer way
    // threaded_rt.spawn(async move {
    //     tokio::time::sleep(Duration::from_secs(400)).await;
    //     stats_hanlder
    //         .tx
    //         .send(StatsMessage::ShowStats)
    //         .await
    //         .expect("failed to request showing stats")
    // });

    let w = 3500 / n_workers;
    let h = 3500;

    spawn_tasks(n_workers, &client, w, h, started).collect::<Vec<_>>().await;
}
