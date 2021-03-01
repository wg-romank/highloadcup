use crate::client::Client;
use crate::client::ClientResponse;
use crate::model::Treasure;
use crate::dto::License;

use std::collections::BinaryHeap;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

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
        addr: String,
        rx: mpsc::Receiver<MessageForAccounting>,
        tx: mpsc::Sender<MessageFromAccounting>
    ) -> Accounting {
        Accounting {
            client: Client::new(&addr),
            rx: rx,
            tx: tx,
            treasures: BinaryHeap::new(),
            active_licenses: 0,
            licenses: vec![],
            coins: vec![],
        }
    }

    fn accounting_log(message: String) {
        // println!("[accounting]: {}", message);
    }

    pub async fn main(&mut self) -> ClientResponse<()> {
        loop {
            // todo: recover here
            timeout(Duration::from_millis(10), self.rx.recv()).await.map(
                |msg| if let Some(message) = msg {
                    match message {
                        MessageForAccounting::TreasureToClaim(tid) => self.treasures.push(tid),
                        MessageForAccounting::LicenseExpired => self.active_licenses -= 1,
                    }
                }
            );

            match self.step().await {
                Ok(_) => (),
                Err(e) => Accounting::accounting_log(e.to_string()),
            };
        }
    }

    async fn step(&mut self) -> ClientResponse<()> {
        while let Some(lic) = self.licenses.pop() {
            let tx2 = self.tx.clone();
            tokio::spawn(
                async move {
                    tx2.send(MessageFromAccounting::LicenseToUse(lic)).await
                        .map_err(|e| Accounting::accounting_log(format!("tx send err {}", e)));
                }
            );
        }

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
