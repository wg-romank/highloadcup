use std::cmp::Ordering;
use std::collections::BinaryHeap;

use futures::future::join_all;

use tokio::sync::mpsc;

// use rand;
// use rand::distributions::Uniform;
// use rand::{thread_rng, Rng};

mod client;
mod dto;
mod accounting;
mod model;
mod constants;

use client::Client;
use client::ClientResponse;

use accounting::Accounting;
use accounting::MessageFromAccounting;
use accounting::MessageForAccounting;

use dto::*;

use model::Treasure;
use tokio::runtime;
use tokio::time::timeout;
use std::time::{Duration, Instant};
use crate::constants::TIME_LIMIT_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingDig {
    x: u64,
    y: u64,
    depth: u8,
    remaining: u64,
}

impl PendingDig {
    fn new(x: u64, y: u64, remaining: u64) -> PendingDig {
        PendingDig { x, y, depth: 1, remaining }
    }

    fn to_dig(&self, license_id: u64) -> Dig {
        Dig {
            license_id: license_id,
            pos_x: self.x,
            pos_y: self.y,
            depth: self.depth,
        }
    }

    fn next_level(&self, excavated: u64) -> Option<PendingDig> {
        if self.depth < 10 && self.remaining > excavated {
            Some(PendingDig {
                depth: self.depth + 1,
                remaining: self.remaining - excavated,
                ..*self
            })
        } else {
            None
        }
    }
}

impl Ord for PendingDig {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.remaining * self.depth as u64)
            .cmp(&(other.remaining * other.depth as u64))
    }
}

impl PartialOrd for PendingDig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

async fn logic(
    client: &Client,
    tx: &mpsc::Sender<MessageForAccounting>,
    rx: &mut mpsc::Receiver<MessageFromAccounting>,
    license: &Option<License>,
    explore_heap: &mut BinaryHeap<Explore>,
    dig_heap: &mut BinaryHeap<PendingDig>,
) -> ClientResponse<Option<License>> {
    if let Some(ar) = explore_heap.pop() {
        // todo: if we have total we do not need to get latest from here
        // since it can be computed given previous results
        match ar.area.size() {
            1 => {
                let x = ar.area.pos_x;
                let y = ar.area.pos_y;
                dig_heap.push(PendingDig::new(x, y, ar.amount));
            }
            _ => {
                let divided = ar.area.divide();
                let mut cum = 0;
                for a in divided[..divided.len() - 1].into_iter() {
                    let res = client.explore(&a).await?;
                    if res.amount > 0 {
                        cum += res.amount;
                        explore_heap.push(res);
                        if cum == ar.amount {
                            break
                        }
                    };
                }
                if ar.amount > cum {
                    divided.last().map(|a| {
                        explore_heap.push(Explore { area: *a, amount: ar.amount - cum });
                    });
                }
                // assert_eq!(ar.amount, cum);
            }
        }
    }

    // todo: ordering
    let used_license = match license {
        Some(lic) if lic.dig_used < lic.dig_allowed => {
            if let Some(pending_dig) = dig_heap.pop() {
                let treasure = client.dig(&pending_dig.to_dig(lic.id)).await?;

                let treasures_count = treasure.len() as u64;
                if let Some(next_level) = pending_dig.next_level(treasures_count) {
                    dig_heap.push(next_level);
                }

                if treasures_count > 0 {
                    let tx2 = tx.clone();
                    tokio::spawn(
                        async move {
                            tx2.send(MessageForAccounting::TreasureToClaim(Treasure {
                                depth: pending_dig.depth,
                                treasures: treasure,
                            })).await.map_err(|r| panic!("failed to send treasure {}", r))
                        }
                    );
                }
                Some(License { dig_used: lic.dig_used + 1, ..*lic })
            } else {
                Some(*lic)
            }
        }
        otherwise => {
            if otherwise.is_some() {
                let tx2 = tx.clone();
                tokio::spawn(
                    async move {
                        tx2.send(MessageForAccounting::LicenseExpired).await
                            .map_err(|r| panic!("failed to send license expired message {}", r))
                    }
                );
            };
            match timeout(Duration::from_millis(10), rx.recv()).await {
                Ok(msg) => msg.map(|MessageFromAccounting::LicenseToUse(lic)| lic),
                Err(_) => None,
            }
        }
    };

    Ok(used_license)
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

    // println!("picking:");
    // for i in explore_heap.iter() {
    //     println!("{}", i.hash())
    // }

    Ok(explore_heap)
}

async fn _main(client: Client, started: Instant, areas: Vec<Area>) -> ClientResponse<()> {
    let mut explore_heap = init_state(&client, started, areas).await?;

    // multiple producers, single consumer? for coins
    let mut license: Option<License> = None;
    let mut dig_heap: BinaryHeap<PendingDig> = BinaryHeap::new();

    let (tx_from_accounting, mut rx_from_accounting) = mpsc::channel(20);
    let (tx, rx_for_accounting) = mpsc::channel(1000);
    let cl = client.clone();
    let ttx = tx.clone();
    tx.send(MessageForAccounting::Continue).await;
    tokio::spawn(async move {
        Accounting::new(cl, rx_for_accounting, ttx, tx_from_accounting).main().await
    });

    loop {
        match logic(
            &client,
            &tx,
            &mut rx_from_accounting,
            &license,
            &mut explore_heap,
            &mut dig_heap,
        ).await {
            Ok(used_license) => license = used_license,
            Err(e) => {
                println!("error {}", e)
            }
        };
    }
}

fn main() -> () {
    let n_workers: u64 = 4;
    let threaded_rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(n_workers as usize)
        .build()
        .expect("Could not build runtime");
    let started = Instant::now();

    println!("Started thread = {}", n_workers);

    let address  = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let client = Client::new(&address);

    let w = 3500 / n_workers;
    let h = 3500;

    threaded_rt.block_on(
        join_all((0..n_workers).map(|i| {
            let client = client.clone();
            threaded_rt.spawn(async move {
                let area = Area { pos_x: w * i, pos_y: 0, size_x: w, size_y: h };
                _main(client, started, area
                    .divide()
                    .iter()
                    .flat_map(|a| a.divide()).collect()).await
            })
        }))
    );
}


#[test]
fn test_explore_ord() {
    let mut hp = BinaryHeap::new();
    hp.push(Explore { area: Area { pos_x: 0, pos_y: 0, size_x: 100, size_y: 100 }, amount: 10 });
    hp.push(Explore { area: Area { pos_x: 0, pos_y: 0, size_x: 10, size_y: 10 }, amount: 10 });
    hp.push(Explore { area: Area { pos_x: 0, pos_y: 0, size_x: 1, size_y: 1 }, amount: 3 });

    assert_eq!(hp.pop().unwrap().area.size(), 1);
    assert_eq!(hp.pop().unwrap().area.size(), 100);
    assert_eq!(hp.pop().unwrap().area.size(), 10000);
}

#[test]
fn test_dig_ord() {
    let mut hp = BinaryHeap::new();
    hp.push(PendingDig { x: 1, y: 0, depth: 2, remaining: 11 });
    hp.push(PendingDig { x: 3, y: 0, depth: 2, remaining: 10 });
    hp.push(PendingDig { x: 2, y: 0, depth: 1, remaining: 10 });

    assert_eq!(hp.pop().unwrap().x, 1);
    assert_eq!(hp.pop().unwrap().x, 3);
    assert_eq!(hp.pop().unwrap().x, 2);
}
