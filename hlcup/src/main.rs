use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

use futures::future::join_all;

use tokio::sync::mpsc;

// use rand;
// use rand::distributions::Uniform;
// use rand::{thread_rng, Rng};

mod client;
mod dto;
mod accounting;
mod model;

use client::Client;
use client::ClientResponse;
use client::DescriptiveError;

use accounting::Accounting;
use accounting::MessageFromAccounting;
use accounting::MessageForAccounting;

use dto::*;

use model::Treasure;
use futures::TryFutureExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingDig {
    x: u64,
    y: u64,
    depth: u8,
    remaining: u64
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
                ..*self })
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
    id: u8,
    client: &mut Client,
    tx: &mpsc::Sender<MessageForAccounting>,
    rx: &mut mpsc::Receiver<MessageFromAccounting>,
    license: &Option<License>,
    digging_coordinates: &mut HashSet<(u64, u64)>,
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
                if !digging_coordinates.contains(&(x, y)) {
                    digging_coordinates.insert((x, y));
                    dig_heap.push(PendingDig::new(x, y, ar.amount));
                } else {
                    panic!("digging twice at {} {}", x, y);
                }
            }
            _ => {
                let total = ar.amount;
                let mut cum = 0;
                let divided = ar.area.divide();
                let ll = divided.len();
                for (idx, a) in divided.into_iter().enumerate() {
                    let res = client.explore(&a).await?;
                    if res.amount > 0 {
                        cum += res.amount;
                        explore_heap.push(res);
                    }

                    if idx + 1 < ll {
                        if cum > (total - cum) {
                            break;
                        }
                    }
                }
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
                            })).await.map_err(|e| panic!("cannot send treasure {}", e));
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
                        tx2.send(MessageForAccounting::LicenseExpired(id)).await;
                    }
                );
            };
            match rx.recv().await {
                Some(msg) => match msg {
                    MessageFromAccounting::LicenseToUse(lic) => Some(lic)
                }
                None => None
            }
        }
    };

    Ok(used_license)
}

// todo: get rid of it
async fn init_state(client: &mut Client, areas: Vec<Area>) -> ClientResponse<BinaryHeap<Explore>> {
    let mut errors = areas.clone();
    let mut explore_heap = BinaryHeap::new();
    while let Some(a) = errors.pop() {
        match client.explore(&a).await {
            Ok(result) => explore_heap.push(result),
            Err(_) => {
                // println!("area too big {:#?}", a);
                errors.extend(a.divide())
            }
        }
    };

    Ok(explore_heap)
}

async fn _main(id: u8, address: String, areas: Vec<Area>, tx: mpsc::Sender<MessageForAccounting>) -> ClientResponse<()> {
    let mut client = Client::new(&address);
    let mut explore_heap = init_state(&mut client, areas).await?;

    // multiple producers, single consumer? for coins
    let mut license: Option<License> = None;
    let mut dig_heap: BinaryHeap<PendingDig> = BinaryHeap::new();

    let (tx_from_accounting, mut rx_from_accounting) = mpsc::channel(20);

    let tt = tx.clone();
    tokio::spawn(async move {
        tt.send(MessageForAccounting::TxToUse(id, tx_from_accounting)).await
            .map_err(|r| panic!("failed to send tx to accounting {}", r));
    });

    let mut hs = HashSet::new();

    let mut iteration = 0;

    loop {
        match logic(
            id,
            &mut client,
            &tx,
            &mut rx_from_accounting,
            &license,
            &mut hs,
            &mut explore_heap,
            &mut dig_heap,
        ).await {
            Ok(used_license) => license = used_license,
            Err(e) => {
                println!("error {}", e)
            }
        };

        iteration += 1;
        if iteration % 1000 == 0 {
            println!("{}", client.stats);
        }
    }
}

#[tokio::main]
async fn main() ->  Result<(), DescriptiveError> {
    let n_workers = 1;
    println!("Started thread = {}", n_workers);

    let address = std::env::var("ADDRESS").expect("missing env variable ADDRESS");

    let w = 3500 / n_workers;
    let h = 3500;

    let (tx_for_accounting, rx_for_accounting) = mpsc::channel(1000);

    let address_clone = address.clone();
    tokio::spawn(async move {
        let mut accounting = Accounting::new(
            address_clone,
            rx_for_accounting
        );
        accounting.main().await
    });

    join_all(
        (0..n_workers).map(|i| {
            let addr = address.clone();
            let tx = tx_for_accounting.clone();
            tokio::spawn(async move {
                let area = Area { pos_x: w * i, pos_y: 0, size_x: w, size_y: h };
                _main(i as u8, addr, area
                    .divide()
                    .iter()
                    .flat_map(|a| a.divide()).collect(), tx).await
            })
        })
    ).await;

    Ok(())
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
