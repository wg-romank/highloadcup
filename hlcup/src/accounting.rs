use crate::client::Client;
use crate::client::ClientResponse;
use crate::model::Treasure;
use crate::dto::License;

use std::collections::BinaryHeap;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;
use crate::constants::CONCURRENT_LICENSES;

pub struct Accounting {
    client: Client,
    rx: mpsc::Receiver<MessageForAccounting>,
    tx: mpsc::Sender<MessageFromAccounting>,
    treasures: BinaryHeap<Treasure>,
    coins_to_use: u8,
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
            coins_to_use: 0,
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
        if let Some(pending_cash) = self.treasures.pop() {
            for treasure in pending_cash.treasures.into_iter() {
                match self.client.cash(treasure.clone()).await {
                    Ok(got_coins) => self.coins.extend(got_coins),
                    Err(e) => {
                        Accounting::accounting_log(e.to_string());
                        self.treasures.push(Treasure { depth: pending_cash.depth, treasures: vec![treasure]})
                    }
                };
            }
        }

        if self.active_licenses < CONCURRENT_LICENSES {
            let license = if !self.coins.is_empty() {
                if self.coins.len() < 1000 {
                    let c = self.coins.pop().map(|c| vec![c]).unwrap_or(vec![]);
                    loop {
                        if let Some(lic) = self.client.get_license(c.clone()).await.ok() {
                            break lic
                        }
                    }
                } else {
                    let mut i = 0;
                    let mut coins = vec![];
                    while let Some(c) = self.coins.pop() {
                        coins.push(c);
                        i += 1;
                        if i == self.coins_to_use {
                            break;
                        }
                    }
                    let lic = loop {
                        if let Some(lic) = self.client.get_license(coins.clone()).await.ok() {
                            break lic
                        }
                    };
                    println!("num coins {} -> digs allowed {}", coins.len(), lic.dig_allowed);
                    self.coins_to_use += 1;
                    lic
                }
            } else {
                self.client.get_license(vec![]).await?
            };
            self.licenses.push(license);
            self.active_licenses += 1;
        };

        Ok(())
    }
}
