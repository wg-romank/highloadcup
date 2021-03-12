use crate::client::Client;
use crate::model::Treasure;
use crate::dto::License;

use std::collections::BinaryHeap;

use tokio::sync::{mpsc, oneshot};
use futures::{Future, FutureExt, StreamExt};
use futures::stream::FuturesUnordered;

use crate::constants::CONCURRENT_LICENSES;

pub struct Accounting {
    client: Client,
    rx: mpsc::Receiver<MessageForAccounting>,
    selftx: mpsc::Sender<MessageForAccounting>,
    treasures: BinaryHeap<Treasure>,
    active_licenses: u8,
    licenses: Vec<License>,
    coins: Vec<u64>,
}

pub enum MessageForAccounting {
    TreasureToClaim(Treasure),
    GetLicense(oneshot::Sender<Vec<License>>),
    LicenseExpired,
    Continue,
}

impl Accounting {
    pub fn new(
        client: Client,
        rx: mpsc::Receiver<MessageForAccounting>,
        selftx: mpsc::Sender<MessageForAccounting>,
    ) -> Accounting {
        Accounting {
            client: client,
            rx: rx,
            selftx: selftx,
            treasures: BinaryHeap::new(),
            active_licenses: 0,
            licenses: vec![],
            coins: vec![],
        }
    }

    fn claim_treasure(client: &Client, t: Treasure) -> Vec<impl Future<Output=Vec<u64>>> {
        t.treasures.into_iter().map(move |tt| {
            let cl = client.clone();
            tokio::spawn(async move {
                cl.plain_cash(tt).await
            }).map(|r| r.ok().unwrap_or(vec![]))
        }).collect()
    }

    fn claim_treasures(client: &Client, treasures: &mut BinaryHeap<Treasure>) -> FuturesUnordered<impl Future<Output=Vec<u64>>> {
        treasures.drain().flat_map(move |t| {
            Accounting::claim_treasure(client, t)
        }).collect()
    }
}

impl Accounting {
    async fn update_state(&mut self) {
        let cc= Accounting::claim_treasures(&self.client, &mut self.treasures)
            .collect::<Vec<Vec<u64>>>().await;
        let ccc = cc.into_iter().flatten().collect::<Vec<u64>>();
        self.coins.extend(ccc);

        // todo: join with futures unordered
        if self.active_licenses < CONCURRENT_LICENSES {
            let lic = if let Some(c) = self.coins.pop() {
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
                MessageForAccounting::TreasureToClaim(tid) => { self.treasures.push(tid); },
                MessageForAccounting::LicenseExpired => { self.active_licenses -= 1; },
                MessageForAccounting::GetLicense(tx) => {
                    tx.send(self.licenses.clone()).expect("failed to send licenses to worker");
                    self.licenses.clear();
                }
                MessageForAccounting::Continue => {
                    self.update_state().await;
                    assert!(self.selftx.send(MessageForAccounting::Continue).await.is_ok(), "failed to send continue");
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
            assert!(tx_clone.send(MessageForAccounting::Continue).await.is_ok(), "failed to start accounting");
            Accounting::new(cl, rx, tx_clone).run().await
        });

        Self { sender: tx }
    }
}