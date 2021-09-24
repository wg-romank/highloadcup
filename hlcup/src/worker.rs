use crate::Rules;
use std::collections::BinaryHeap;
use std::time::Instant;

use tokio::sync::oneshot;
use tokio::sync::mpsc;

use crate::accounting::MessageForAccounting;
use crate::client::{Client, ClientResponse};
use crate::constants::TIME_LIMIT_MS;
use crate::dto::{Area, Explore, License};
use crate::model::{PendingDig, Treasure};

pub struct Worker {
    client: Client,
    rules: Rules,
    licenses: Vec<License>,
    explore_heap: BinaryHeap<Explore>,
    dig_heap: BinaryHeap<PendingDig>,
    accounting_handle: mpsc::Sender<MessageForAccounting>,
}

impl Worker {
    pub async fn run(&mut self) {
        loop {
            match self.logic().await {
                Ok(_) => (),
                Err(e) => {
                    println!("error {}", e)
                }
            }
        }
    }

    pub async fn new(
        client: Client,
        rules: Rules,
        started: Instant,
        areas: Vec<Area>,
        accounting_handle: mpsc::Sender<MessageForAccounting>,
    ) -> Self {
        let explore_heap = Worker::init_state(&client, &rules, started, areas)
            .await
            .expect("failed to initialize worker state");

        Self {
            client,
            rules,
            licenses: vec![],
            explore_heap,
            dig_heap: BinaryHeap::<PendingDig>::new(),
            accounting_handle,
        }
    }

    // todo: get rid of it
    async fn init_state(
        client: &Client,
        rules: &Rules,
        started: Instant,
        areas: Vec<Area>,
    ) -> ClientResponse<BinaryHeap<Explore>> {
        let mut errors = BinaryHeap::new();
        areas.clone().iter().for_each(|a| {
            errors.push(Explore {
                area: *a,
                amount: u64::max_value(),
            })
        });
        let mut explore_heap = BinaryHeap::new();
        while let Some(a) = errors.pop() {
            match client.explore(&a.area).await {
                Ok(result) if result.is_managable(started, rules.max_depth) => {
                    explore_heap.push(result);
                }
                Ok(result) => errors.extend(result.area.divide().into_iter().map(|a| Explore {
                    area: a,
                    amount: result.amount,
                })),
                Err(_) => errors.extend(a.area.divide().into_iter().map(|a| Explore {
                    area: a,
                    amount: u64::max_value(),
                })),
            }
        }

        let mut ff = BinaryHeap::new();
        let mut cum_cost = 0;
        while let Some(e) = explore_heap.pop() {
            // todo: skip this if
            if e.is_managable(started, rules.max_depth) {
                cum_cost += e.cost(rules.max_depth);
                ff.push(e);

                let time_since_started_ms = started.elapsed().as_millis();
                let remaining_time_ms = TIME_LIMIT_MS - time_since_started_ms;
                if cum_cost > remaining_time_ms {
                    break;
                }
            }
        }

        // todo: multiple?

        // println!("picking:");
        // for i in explore_heap.iter() {
        //     println!("{}", i.hash())
        // }

        Ok(ff)
    }

    async fn logic(&mut self) -> ClientResponse<()> {
        if let Some(ar) = self.explore_heap.pop() {
            // todo: if we have total we do not need to get latest from here
            // since it can be computed given previous results
            match ar.area.size() {
                1 => {
                    self.dig_heap
                        .push(PendingDig::new(ar.area.pos_x, ar.area.pos_y, ar.amount));
                }
                _ => {
                    let divided = ar.area.divide();
                    let mut cum = 0;
                    for a in divided[..divided.len() - 1].iter() {
                        let res = self.client.explore(a).await?;
                        if res.amount > 0 {
                            cum += res.amount;
                            self.explore_heap.push(res);
                            if cum == ar.amount {
                                break;
                            }
                        };
                    }
                    if ar.amount > cum {
                        if let Some(a) = divided.last() {
                            self.explore_heap.push(Explore {
                                area: *a,
                                amount: ar.amount - cum,
                            });
                        }
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
                if let Some(next_level) = pending_dig.next_level(self.rules.max_depth, treasures_count) {
                    self.dig_heap.push(next_level);
                }

                if treasures_count > 0 {
                    self.accounting_handle
                        .send(MessageForAccounting::TreasureToClaim(Treasure {
                            depth: pending_dig.depth,
                            treasures: treasure,
                        }))
                        .await
                        .expect("failed to send treasure");
                }
                lic.dig_used += 1;
                if lic.is_still_valid() {
                    self.licenses.push(lic)
                } else {
                    self.accounting_handle
                        .send(MessageForAccounting::LicenseExpired(self.pending_digs()))
                        .await
                        .expect("failed to notify for license expiration");
                }
            } else {
                self.dig_heap.push(pending_dig);
                let (tx, rx) = oneshot::channel();
                self.accounting_handle
                    .send(MessageForAccounting::GetLicense(tx))
                    .await
                    .expect("failed to request license");
                self.licenses.extend(rx.await.expect("failed to receive license"))
            }
        };

        Ok(())
    }

    fn pending_digs(&self) -> u64 {
        self.dig_heap
            .iter()
            .map(|pd| (self.rules.max_depth + 1 - pd.depth) as u64)
            .sum()
    }
}
