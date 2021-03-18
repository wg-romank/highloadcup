mod client;
mod dto;
mod accounting;
mod model;
mod constants;
mod worker;
mod stats;

use std::time::Instant;

use tokio::runtime;
use futures::future::join_all;

use client::Client;
use client::ClientResponse;

use dto::*;

use accounting::AccountingHandle;
use worker::Worker;
use crate::stats::{StatsHandler, StatsMessage};
use tokio::time::Duration;
use base64::display::Base64Display;


async fn _main(client: Client, started: Instant, areas: Vec<Area>) {
    let accounting_handle = AccountingHandle::new(&client);

    Worker::new(client.clone(), started, areas, accounting_handle)
        .await
        .run()
        .await
}

fn main() -> () {
    let n_workers: u64 = 4;
    let threaded_rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(n_workers as usize)
        .build()
        .expect("Could not build runtime");
    let started = Instant::now();

    println!("Started thread = {}", n_workers);

    let address  = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let stats_hanlder = StatsHandler::new(&threaded_rt);
    let client = Client::new(&address, stats_hanlder.tx.clone());

    // todo: nicer way
    threaded_rt.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(550)).await;
            stats_hanlder.tx.send(StatsMessage::ShowStats).await;
        }
    });

    let w = 3500 / n_workers;
    let h = 3500;

    threaded_rt.block_on(
        join_all((0..n_workers).map(|i| {
            let client = client.clone();
            threaded_rt.spawn(async move {
                let area = Area { pos_x: w * i, pos_y: 0, size_x: w, size_y: h };
                _main(client, started, area
                    .divide()
                    .iter()
                    .flat_map(|a| a.divide()).collect()).await
            })
        }))
    );
}
