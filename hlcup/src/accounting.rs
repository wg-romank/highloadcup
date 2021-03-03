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
