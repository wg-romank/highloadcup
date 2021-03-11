use crate::client::Client;
use crate::client::ClientResponse;
use crate::model::Treasure;
use crate::dto::License;

use std::collections::BinaryHeap;

use tokio::sync::mpsc;
use futures::{Future, FutureExt, StreamExt};
use futures::stream::FuturesUnordered;

use crate::constants::CONCURRENT_LICENSES;

pub struct Accounting {
    client: Client,
    rx: mpsc::Receiver<MessageForAccounting>,
    selftx: mpsc::Sender<MessageForAccounting>,
    tx: mpsc::Sender<MessageFromAccounting>,
    treasures: BinaryHeap<Treasure>,
    active_licenses: u8,
    licenses: Vec<License>,
    coins: Vec<u64>,
}

pub enum MessageForAccounting {
    TreasureToClaim(Treasure),
    LicenseExpired,
    Continue,
}

pub enum MessageFromAccounting {
    LicenseToUse(License)
}

impl Accounting {
    pub fn new(
        client: Client,
        rx: mpsc::Receiver<MessageForAccounting>,
        selftx: mpsc::Sender<MessageForAccounting>,
        tx: mpsc::Sender<MessageFromAccounting>,
    ) -> Accounting {
        Accounting {
            client: client,
            rx: rx,
            selftx: selftx,
            tx: tx,
            treasures: BinaryHeap::new(),
            active_licenses: 0,
            licenses: vec![],
            coins: vec![],
        }
    }

    fn accounting_log(_message: String) {
        // println!("[accounting]: {}", message);
    }

    async fn send_lic(licenses: &mut Vec<License>, tx: mpsc::Sender<MessageFromAccounting>) {
        if let Some(lic) = licenses.pop() {
            tokio::spawn(
                async move {
                    tx.send(MessageFromAccounting::LicenseToUse(lic)).await
                        .map_err(|e| panic!("tx send err {}", e))
                }
            );
        }
    }

    fn claim_treasure(client: &Client, t: Treasure) -> Vec<impl Future<Output = Vec<u64>>> {
        t.treasures.into_iter().map(move |tt| {
            let cl = client.clone();
            tokio::spawn(async move {
                cl.plain_cash(tt).await
            }).map(|r| r.ok().unwrap_or(vec![]))
        }).collect()
    }

    fn claim_treasures(client: &Client, treasures: &mut BinaryHeap<Treasure>) -> FuturesUnordered<impl Future<Output = Vec<u64>>> {
        treasures.drain().flat_map(move |t| {
            Accounting::claim_treasure(client, t)
        }).collect()
    }

    pub async fn main(&mut self) -> ClientResponse<()> {
        while let Some(message) = self.rx.recv().await {
            match message {
                MessageForAccounting::TreasureToClaim(tid) => { self.treasures.push(tid); },
                MessageForAccounting::LicenseExpired => { self.active_licenses -= 1; },
                MessageForAccounting::Continue => {
                    self.purchase_license().await;
                    Accounting::send_lic(&mut self.licenses, self.tx.clone()).await;
                    self.selftx.send(MessageForAccounting::Continue).await
                        .map_err(|e| panic!("failed to continue {}", e));
                }
            }
        };

        Ok(())
    }

    async fn purchase_license(&mut self) {
        let cc= Accounting::claim_treasures(&self.client, &mut self.treasures)
            .collect::<Vec<Vec<u64>>>().await;
        let ccc = cc.into_iter().flatten().collect::<Vec<u64>>();
        self.coins.extend(ccc);

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
}