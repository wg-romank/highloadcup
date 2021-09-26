use std::time::Duration;
use std::collections::{BinaryHeap, HashMap};

use futures::stream::FuturesUnordered;
use futures::{Future, FutureExt, StreamExt};

use tokio::sync::mpsc;

use lazy_static::lazy_static;

use crate::MessageForAccounting;
use crate::http::client::Client;
use crate::http::dto::License;
use crate::models::data::Treasure;
use crate::actors::Actor;

// const COINS_MAX: usize = 21;

lazy_static! {
    // todo: add max value?
    static ref COINS: HashMap<usize, u64> = vec![
        (0, 3),
        (1, 5),
        (6, 10),
        (11, 20),
        (21, 40),
    ].into_iter().collect();
}

pub struct Accounting {
    client: Client,
    rx: mpsc::Receiver<MessageForAccounting>,
    treasures: BinaryHeap<Treasure>,
    // coins_to_use: usize,
    digs_pending: u64,
    active_licenses: u8,
    licenses: Vec<License>,
    coins: Vec<u64>,
    max_concurrent_licenses: u8,
}

impl Accounting {
    pub fn new(c: &Client, max_concurrent_licenses: u8) -> impl FnOnce(mpsc::Receiver<MessageForAccounting>) -> Self {
        let client = c.clone();
        move |rx| Self {
            client,
            rx,
            treasures: BinaryHeap::new(),
            // coins_to_use: 2,
            digs_pending: 0,
            active_licenses: 0,
            licenses: vec![],
            coins: vec![],
            max_concurrent_licenses,
        }
    }
}

impl Accounting {
    fn claim_treasures(
        client: &Client,
        treasures: &mut BinaryHeap<Treasure>,
    ) -> FuturesUnordered<impl Future<Output = Result<Vec<u64>, Treasure>>> {
        treasures
            .drain()
            .map(|t| {
                let cl = client.clone();
                async move { (cl.cash(&t).await, t) }
            })
            .map(|future|
                future.map(|(res, t)| res.map_err(|_| t))
            )
            .collect()
    }

    async fn claim_all(client: &Client, treasures: &mut BinaryHeap<Treasure>) -> Vec<u64> {
        let results = Accounting::claim_treasures(client, treasures)
            .collect::<Vec<Result<Vec<u64>, Treasure>>>()
            .await;

        results.into_iter().fold(vec![], |mut coins, r| {
            match r {
                Ok(c) => coins.extend(c),
                Err(t) => treasures.push(t),
            };
            coins
        })
    }

    fn fetch_licenses(
        client: &Client,
        amount: u8,
        coins: &mut Vec<u64>
    ) -> FuturesUnordered<impl Future<Output=Result<License, Vec<u64>>>> {
        (0..amount)
            .map(|_| {
                let cl = client.clone();
                let coin = if let Some(c) = coins.pop() {
                    vec![c]
                } else {
                    vec![]
                };
                async move { (cl.get_license(&coin).await, coin) }
            })
            .map(|future|
                future.map(|(res, coin)| res.map_err(|_| coin))
            )
            .collect()
    }

    async fn fetch_and_update(client: &Client, amount: u8, coins: &mut Vec<u64>) -> Vec<License> {
        let licenses = Accounting::fetch_licenses(client, amount, coins)
            .collect::<Vec<Result<License, Vec<u64>>>>()
            .await;

        licenses.into_iter().fold(vec![], |mut acc, item| {
            match item {
                Ok(lic) => acc.push(lic),
                Err(c) => coins.extend(c),
            };
            acc
        })
    }

    async fn cash_out(&mut self) {
        self.coins.extend(
            Accounting::claim_all(&self.client, &mut self.treasures).await);
    }

    async fn prep_licenses(&mut self) {
        let to_prep = self.max_concurrent_licenses - self.active_licenses;
        if to_prep > 0 {
            let licenses = Accounting::fetch_and_update(
                &self.client,
                to_prep,
                &mut self.coins
            ).await;
            self.active_licenses += licenses.len() as u8;
            self.licenses.extend(licenses);
        }
    }
}

impl Accounting {
    pub async fn run(&mut self) {
        loop {
            match tokio::time::timeout(Duration::from_millis(9), self.rx.recv()).await {
                Ok(Some(message)) => match message {
                    MessageForAccounting::TreasureToClaim(tid) => {
                        let depth = tid.depth;
                        tid.treasures.into_iter()
                            .for_each(|t| self.treasures.push(Treasure::new(depth, t)));
                        self.cash_out().await;
                    }
                    MessageForAccounting::LicenseExpired(digs_pending) => {
                        self.active_licenses -= 1;
                        self.digs_pending = digs_pending;
                        self.prep_licenses().await;
                    }
                    MessageForAccounting::GetLicense(tx) => {
                        tx.send(self.licenses.clone())
                            .expect("failed to send licenses to worker");
                        self.licenses.clear();
                    },
                },
                _ => {
                    self.cash_out().await;
                    self.prep_licenses().await;
                },
            }
        }
    }
}

impl Actor for Accounting {
    fn start(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }
}
