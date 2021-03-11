use crate::client::Client;
use crate::client::ClientResponse;
use crate::model::Treasure;
use crate::dto::License;

use std::collections::BinaryHeap;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;
use crate::constants::CONCURRENT_LICENSES;
use futures::{Future, FutureExt, StreamExt};

pub struct Accounting {
    client: Client,
    rx: mpsc::Receiver<MessageForAccounting>,
    tx: mpsc::Sender<MessageFromAccounting>,
    treasures: BinaryHeap<Treasure>,
    active_licenses: u8,
    licenses: Vec<License>,
    coins: Vec<u64>,
}

pub enum MessageForAccounting {
    TreasureToClaim(Treasure),
    LicenseExpired,
}

pub enum MessageFromAccounting {
    LicenseToUse(License)
}

impl Accounting {
    pub fn new(
        client: Client,
        rx: mpsc::Receiver<MessageForAccounting>,
        tx: mpsc::Sender<MessageFromAccounting>,
    ) -> Accounting {
        Accounting {
            client: client,
            rx: rx,
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

    async fn send_lic(license: License, tx: mpsc::Sender<MessageFromAccounting>) {
        tokio::spawn(
            async move {
                tx.send(MessageFromAccounting::LicenseToUse(license)).await
                    .map_err(|e| panic!("tx send err {}", e))
            }
        );
    }

    fn claim_treasure(client: Client, t: Treasure) -> Vec<impl Future<Output = Vec<u64>>> {
        t.treasures.into_iter().map(move |tt| {
            let cl = client.clone();
            tokio::spawn(async move {
                cl.plain_cash(tt).await
            }).map(|r| r.ok().unwrap_or(vec![]))
        }).collect()
    }

    fn claim_treasures(&mut self) -> FuturesUnordered<impl Future<Output = Vec<u64>>> {
        let client = self.client.clone();
        self.treasures.drain().flat_map(move |t| {
            Accounting::claim_treasure(client.clone(), t)
        }).collect()
    }

    pub async fn main(&mut self) -> ClientResponse<()> {
        loop {
            if let Some(lic) = self.licenses.pop() {
                Accounting::send_lic(lic, self.tx.clone()).await;
            }

            match timeout(Duration::from_millis(10), self.rx.recv()).await {
                Ok(msg) => { msg.map(
                    |message| match message {
                        MessageForAccounting::TreasureToClaim(tid) => { self.treasures.push(tid); },
                        MessageForAccounting::LicenseExpired => { self.active_licenses -= 1; },
                    }); },
                Err(_) => (),
            };

            match self.step().await {
                Ok(_) => (),
                Err(e) => Accounting::accounting_log(e.to_string()),
            };
        }
    }

    async fn step(&mut self) -> ClientResponse<()> {
        // todo: tradeoff between claiming and getting new licenses
        self.claim_treasures().collect::<Vec<Vec<u64>>>().await
            .into_iter().for_each(|ar| self.coins.extend(ar));

        if self.active_licenses < CONCURRENT_LICENSES {
            let license = if let Some(c) = self.coins.pop() {
                self.client.get_license(vec![c]).await
                    .map_err(|e| { self.coins.push(c); e })?
            } else {
                self.client.get_license(vec![]).await?
            };
            self.licenses.push(license);
            self.active_licenses += 1;
        };

        Ok(())
    }
}

use futures::stream::FuturesUnordered;

async fn do_work(i: usize) -> Result<String, Vec<u64>> {
    unimplemented!()
}

async fn do_work_plain(i: usize) -> Vec<u64> {
    unimplemented!()
}


// async fn claim(&mut self) -> Vec<Box<dyn Future<Output = ClientResponse<License>>>> {
// fn compose_work(n: usize) -> Vec<Box<dyn Future<Output=Result<String, u64>>>> {
//    (0..n).map(|i| Box::new(do_work(i))).collect()
// }

fn compose_work(n: usize) -> FuturesUnordered<impl Future> {
    (0..n).map(|i| do_work_plain(i)).collect()
}