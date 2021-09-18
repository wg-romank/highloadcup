use crate::client::Client;
use crate::dto::License;
use crate::model::Treasure;

use std::collections::{BinaryHeap, HashMap};

use futures::stream::FuturesUnordered;
use futures::{Future, FutureExt, StreamExt};
use tokio::sync::{mpsc, oneshot};

use lazy_static::lazy_static;

use crate::constants::CONCURRENT_LICENSES;

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
    selftx: mpsc::Sender<MessageForAccounting>,
    treasures: BinaryHeap<Treasure>,
    // coins_to_use: usize,
    digs_pending: u64,
    active_licenses: u8,
    licenses: Vec<License>,
    coins: Vec<u64>,
}

#[derive(Debug)]
pub enum MessageForAccounting {
    TreasureToClaim(Treasure),
    GetLicense(oneshot::Sender<Vec<License>>),
    LicenseExpired(u64),
    Continue,
}

impl Accounting {
    pub fn new(
        client: Client,
        rx: mpsc::Receiver<MessageForAccounting>,
        selftx: mpsc::Sender<MessageForAccounting>,
    ) -> Accounting {
        Accounting {
            client,
            rx,
            selftx,
            treasures: BinaryHeap::new(),
            // coins_to_use: 2,
            digs_pending: 0,
            active_licenses: 0,
            licenses: vec![],
            coins: vec![],
        }
    }

    fn claim_treasure(client: &Client, t: Treasure) -> Vec<impl Future<Output = Vec<u64>>> {
        let depth = t.depth;
        t.treasures
            .into_iter()
            .map(move |tt| {
                let cl = client.clone();
                tokio::spawn(async move { cl.plain_cash(depth, tt).await })
                    .map(|r| r.ok().unwrap_or_default())
            })
            .collect()
    }

    fn claim_treasures(
        client: &Client,
        treasures: &mut BinaryHeap<Treasure>,
    ) -> FuturesUnordered<impl Future<Output = Vec<u64>>> {
        treasures
            .drain()
            .flat_map(move |t| Accounting::claim_treasure(client, t))
            .collect()
    }
}

impl Accounting {
    async fn update_state(&mut self) {
        let cc = Accounting::claim_treasures(&self.client, &mut self.treasures)
            .collect::<Vec<Vec<u64>>>()
            .await;
        let ccc = cc.into_iter().flatten().collect::<Vec<u64>>();
        self.coins.extend(ccc);

        // todo: join with futures unordered
        if self.active_licenses < CONCURRENT_LICENSES {
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
        while let Some(message) = self.rx.recv().await {
            match message {
                MessageForAccounting::TreasureToClaim(tid) => {
                    self.treasures.push(tid);
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
                MessageForAccounting::Continue => {
                    self.update_state().await;
                    assert!(
                        self.selftx
                            .send(MessageForAccounting::Continue)
                            .await
                            .is_ok(),
                        "failed to send continue"
                    );
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct AccountingHandle {
    pub sender: mpsc::Sender<MessageForAccounting>,
}

impl AccountingHandle {
    pub fn new(client: &Client) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        let cl = client.clone();

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            assert!(
                tx_clone.send(MessageForAccounting::Continue).await.is_ok(),
                "failed to start accounting"
            );
            Accounting::new(cl, rx, tx_clone).run().await
        });

        Self { sender: tx }
    }
}
