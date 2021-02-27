use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

// use rand;
// use rand::distributions::Uniform;
// use rand::{thread_rng, Rng};

mod client;
mod dto;

use client::Client;
use client::ClientResponse;
use client::DescriptiveError;

use dto::*;

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

#[derive(Debug, PartialEq, Eq)]
struct Treasure {
    depth: u8,
    treasures: Vec<String>,
}

impl Ord for Treasure {
    fn cmp(&self, other: &Self) -> Ordering {
        // todo: other kind of priority
        self.depth.cmp(&other.depth)
    }
}

impl PartialOrd for Treasure {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

async fn logic(
    client: &Client,
    coins: &mut Vec<u64>,
    license: &Option<License>,
    digging_coordinates: &mut HashSet<(u64, u64)>,
    explore_heap: &mut BinaryHeap<Explore>,
    dig_heap: &mut BinaryHeap<PendingDig>,
    treasure_heap: &mut BinaryHeap<Treasure>,
) -> ClientResponse<Option<License>> {
    while let Some(pending_cash) = treasure_heap.pop() {
        for treasure in pending_cash.treasures.into_iter() {
            match client.cash(treasure.clone()).await {
                Ok(got_coins) => coins.extend(got_coins),
                _ => treasure_heap.push(Treasure { depth: pending_cash.depth, treasures: vec![treasure]}),
            };
        }
    }
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
            _ => for a in ar.area.divide().into_iter() {
                let res = client.explore(&a).await?;
                if res.amount > 0 {
                    explore_heap.push(res);
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
                    treasure_heap.push(Treasure {
                        depth: pending_dig.depth,
                        treasures: treasure,
                    });
                }
                Some(License { dig_used: lic.dig_used + 1, ..*lic })
            } else {
                Some(*lic)
            }
        }
        _ => Some(
            if let Some(c) = coins.pop() {
                client.get_license(vec![c]).await
                    .map_err(|e| { coins.push(c); e })?
            } else {
                client.get_license(vec![]).await?
            }
        ),
    };

    Ok(used_license)
}

async fn init_state(client: &Client, areas: Vec<Area>) -> ClientResponse<BinaryHeap<Explore>> {
    let mut errors = areas.clone();
    let mut explore_heap = BinaryHeap::new();
    while let Some(a) = errors.pop() {
        match client.explore(&a).await {
            Ok(result) => explore_heap.push(result),
            Err(_) => {
                println!("area too big {:#?}", a);
                errors.extend(a.divide())
            }
        }
    };

    Ok(explore_heap)
}

async fn _main(address: &str, area: Area) -> ClientResponse<()> {
    let client = Client::new(&address);
    let mut explore_heap = init_state(&client, vec![area]).await?;

    // multiple producers, single consumer? for coins
    let mut coins: Vec<u64> = vec![];
    let mut license: Option<License> = None;
    let mut dig_heap: BinaryHeap<PendingDig> = BinaryHeap::new();
    let mut treasure_heap: BinaryHeap<Treasure> = BinaryHeap::new();

    let mut hs = HashSet::new();

    loop {
        match logic(
            &client,
            &mut coins,
            &license,
            &mut hs,
            &mut explore_heap,
            &mut dig_heap,
            &mut treasure_heap
        ).await {
            Ok(used_license) => license = used_license,
            Err(e) => {
                println!("error {}", e)
            }
        }
    }
}

#[tokio::main(worker_threads = 1)]
async fn main() ->  Result<(), DescriptiveError> {
    println!("Started");
    let address = std::env::var("ADDRESS").expect("missing env variable ADDRESS");
    let w = 3500;
    let h = 3500;

    let area = Area { pos_x: 0, pos_y: 0, size_x: w, size_y: h };

    tokio::spawn(async move {
        _main(&address, area).await
    }).await;

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

#[test]
fn test_treasure_ord() {
    let mut hp = BinaryHeap::new();
    hp.push(Treasure { depth: 1, treasures: vec![]});
    hp.push(Treasure { depth: 2, treasures: vec![]});

    assert_eq!(hp.pop().unwrap().depth, 2);
    assert_eq!(hp.pop().unwrap().depth, 1);
}
