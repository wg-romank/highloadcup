use std::collections::BinaryHeap;
use std::time::Instant;

use tokio::sync::oneshot;

use crate::accounting::{MessageForAccounting, AccountingHandle};
use crate::dto::{License, Explore, Area};
use crate::client::{Client, ClientResponse};
use crate::model::{Treasure, PendingDig};
use crate::constants::TIME_LIMIT_MS;

pub struct Worker {
    client: Client,
    licenses: Vec<License>,
    explore_heap: BinaryHeap<Explore>,
    dig_heap: BinaryHeap<PendingDig>,
    accounting_handle: AccountingHandle,
}

impl Worker {
    pub async fn run(&mut self) {
        loop {
            match self.logic().await {
                Ok(_) => (),
                Err(e) => {
                    println!("error {}", e)
                },
            }
        }
    }

    pub async fn new(client: Client, started: Instant, areas: Vec<Area>, accounting_handle: AccountingHandle) -> Self {
        let explore_heap = Worker::init_state(&client, started, areas).await.unwrap();

        Self {
            client: client,
            licenses: vec![],
            explore_heap: explore_heap,
            dig_heap: BinaryHeap::<PendingDig>::new(),
            accounting_handle: accounting_handle
        }
    }

    // todo: get rid of it
    async fn init_state(client: &Client, started: Instant, areas: Vec<Area>) -> ClientResponse<BinaryHeap<Explore>> {
        let mut errors = BinaryHeap::new();
        areas.clone().iter().for_each(|a| {
            errors.push(Explore { area: *a, amount: u64::max_value() })
        });
        let mut explore_heap = BinaryHeap::new();
        let mut cum_cost = 0;
        while let Some(a) = errors.pop() {
            match client.explore(&a.area).await {
                Ok(result) if result.is_managable(started) => {
                    cum_cost += result.cost();
                    explore_heap.push(result);
                    // todo: multiple?
                    let time_since_started_ms = started.elapsed().as_millis();
                    let remaining_time_ms = TIME_LIMIT_MS - time_since_started_ms;
                    if cum_cost > remaining_time_ms {
                        break
                    }
                },
                Ok(result) => {
                    errors.extend(result.area.divide().into_iter().map(|a| Explore { area: a, amount: result.amount }))
                }
                Err(_) => {
                    // println!("area too big {:#?}", a);
                    errors.extend(a.area.divide().into_iter().map(|a| Explore { area: a, amount: u64::max_value() }))

                }
            }
        };

        println!("picking:");
        for i in explore_heap.iter() {
            println!("{}", i.hash())
        }

        Ok(explore_heap)
    }

    async fn logic(&mut self) -> ClientResponse<()> {
        if let Some(ar) = self.explore_heap.pop() {
            // todo: if we have total we do not need to get latest from here
            // since it can be computed given previous results
            match ar.area.size() {
                1 => { self.dig_heap.push(PendingDig::new(ar.area.pos_x, ar.area.pos_y, ar.amount)); }
                _ => {
                    let divided = ar.area.divide();
                    let mut cum = 0;
                    for a in divided[..divided.len() - 1].into_iter() {
                        let res = self.client.explore(&a).await?;
                        if res.amount > 0 {
                            cum += res.amount;
                            self.explore_heap.push(res);
                            if cum == ar.amount {
                                break
                            }
                        };
                    }
                    if ar.amount > cum {
                        divided.last().map(|a| {
                            self.explore_heap.push(Explore { area: *a, amount: ar.amount - cum });
                        });
                    }
                    // todo: checks
                    // assert_eq!(ar.amount, cum);
                }
            }
        }

        // todo: ordering
        if let Some(pending_dig) = self.dig_heap.pop() {
            if let Some(mut lic) = self.licenses.pop() {
                let treasure = self.client.dig(&pending_dig.to_dig(lic.id)).await?;

                let treasures_count = treasure.len() as u64;
                if let Some(next_level) = pending_dig.next_level(treasures_count) {
                    self.dig_heap.push(next_level);
                }

                if treasures_count > 0 {
                    self.accounting_handle.sender.send(MessageForAccounting::TreasureToClaim(Treasure {
                        depth: pending_dig.depth,
                        treasures: treasure,
                    })).await.map_err(|r| panic!("failed to send treasure {}", r));
                }
                lic.dig_used += 1;
                if lic.is_still_valid() {
                    self.licenses.push(lic)
                } else {
                    self.accounting_handle.sender.send(MessageForAccounting::LicenseExpired).await;
                }
            } else {
                self.dig_heap.push(pending_dig);
                let (tx, rx) = oneshot::channel();
                self.accounting_handle.sender.send(MessageForAccounting::GetLicense(tx)).await;
                self.licenses.extend(rx.await.unwrap())
            }
        };

        Ok(())
    }
}
