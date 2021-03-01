use crate::client::Client;
use crate::client::ClientResponse;
use crate::model::Treasure;
use crate::dto::License;

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

pub struct Accounting {
    client: Client,
    rx: mpsc::Receiver<MessageForAccounting>,
    txes: HashMap<u8, mpsc::Sender<MessageFromAccounting>>,
    treasures: BinaryHeap<Treasure>,
    active_licenses: u8,
    worker_with_license: HashSet<u8>,
    licenses: Vec<License>,
    coins: Vec<u64>,
}

pub enum MessageForAccounting {
    TreasureToClaim(Treasure),
    LicenseExpired(u8),
    TxToUse(u8, mpsc::Sender<MessageFromAccounting>)
}

pub enum MessageFromAccounting {
    LicenseToUse(License)
}

impl Accounting {
    pub fn new(
        addr: String,
        rx: mpsc::Receiver<MessageForAccounting>,
    ) -> Accounting {
        Accounting {
            client: Client::new(&addr),
            rx: rx,
            txes: HashMap::new(),
            treasures: BinaryHeap::new(),
            active_licenses: 0,
            worker_with_license: HashSet::new(),
            licenses: vec![],
            coins: vec![],
        }
    }

    fn accounting_log(message: String) {
        // println!("[accounting]: {}", message);
    }

    async fn send_lic(license: License, tx: mpsc::Sender<MessageFromAccounting>) {
        tokio::spawn(
            async move {
                tx.send(MessageFromAccounting::LicenseToUse(license)).await
                    .map_err(|e| Accounting::accounting_log(format!("tx send err {}", e)));
            }
        );
    }

    pub async fn main(&mut self) -> ClientResponse<()> {
        loop {
            for (w, tx) in self.txes.iter() {
                if !self.worker_with_license.contains(w) {
                    while let Some(lic) = self.licenses.pop() {
                        Accounting::send_lic(lic, tx.clone()).await;
                        self.worker_with_license.insert(*w);
                    }
                }
            }

            match timeout(Duration::from_millis(10), self.rx.recv()).await {
                Ok(msg) => match msg {
                    Some(message) => match message {
                        MessageForAccounting::TxToUse(tid, tx) => { self.txes.insert(tid, tx.clone()); },
                        MessageForAccounting::TreasureToClaim(tid) => { self.treasures.push(tid); },
                        MessageForAccounting::LicenseExpired(workerid) => {
                            self.worker_with_license.remove(&workerid);
                            self.active_licenses -= 1;
                        },
                    },
                    None => (),
                }
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
        if let Some(pending_cash) = self.treasures.pop() {
            for treasure in pending_cash.treasures.into_iter() {
                match self.client.cash(pending_cash.depth, treasure.clone()).await {
                    Ok(got_coins) => self.coins.extend(got_coins),
                    Err(e) => {
                        Accounting::accounting_log(e.to_string());
                        self.treasures.push(Treasure { depth: pending_cash.depth, treasures: vec![treasure]})
                    }
                };
            }
        }

        if self.active_licenses < 10 {
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
