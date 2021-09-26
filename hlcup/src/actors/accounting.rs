use crate::MessageForAccounting;
use crate::http::client::claim_all;
use crate::http::client::Client;
use crate::http::dto::License;
use crate::models::data::Treasure;
use crate::actors::Actor;
use std::time::Duration;

use std::collections::{BinaryHeap, HashMap};

use tokio::sync::mpsc;
use tokio::time::timeout;

use lazy_static::lazy_static;

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
    async fn update_state(&mut self) {
        let ccc = claim_all(&self.client, &mut self.treasures).await;
        self.coins.extend(ccc);

        // todo: join with futures unordered
        if self.active_licenses < self.max_concurrent_licenses {
            let lic = //if self.coins.len() > 1000 {
            //     let coins_to_use = COINS
            //         .iter()
            //         .find(|(&k, &v)| v >= self.digs_pending )
            //         .map(|(&k, _)| k)
            //         .unwrap_or(COINS_MAX) as usize;
            //     let cc = self.coins.drain(0..coins_to_use).collect::<Vec<u64>>();
            //     // if self.coins_to_use < 50 {
            //     //     self.coins_to_use += 1;
            //     // };
            //     self.client.plain_license(cc)
            // } else
            if let Some(c) = self.coins.pop() {
                self.client.plain_license(vec![c])
            } else {
                self.client.plain_license(vec![])
            }.await;
            self.licenses.push(lic);
            self.active_licenses += 1;
        };
    }

    pub async fn run(&mut self) {
        loop {
            match timeout(Duration::from_millis(9), self.rx.recv()).await {
                Ok(Some(message)) => match message {
                    MessageForAccounting::TreasureToClaim(tid) => {
                        let depth = tid.depth;
                        tid.treasures.into_iter()
                            .for_each(|t| self.treasures.push(Treasure::new(depth, t)));
                    }
                    MessageForAccounting::LicenseExpired(digs_pending) => {
                        self.active_licenses -= 1;
                        self.digs_pending = digs_pending
                    }
                    MessageForAccounting::GetLicense(tx) => {
                        tx.send(self.licenses.clone())
                            .expect("failed to send licenses to worker");
                        self.licenses.clear();
                    }
                },
                _ => self.update_state().await,
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
