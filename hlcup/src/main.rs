use rand;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use rand::distributions::Uniform;
use rand::{thread_rng, Rng};

mod client;
mod dto;

use client::Client;
use client::Response;

use dto::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingDig {
    x: u64,
    y: u64,
    current_depth: u8,
    remaining: u64
}

impl PendingDig {
    fn new(x: u64, y: u64, remaining: u64) -> PendingDig {
        PendingDig { x, y, current_depth: 1, remaining }
    }

    fn to_dig(&self, license_id: u64) -> Dig {
        Dig {
            licenseID: license_id,
            posX: self.x,
            posY: self.y,
            depth: self.current_depth,
        }
    }

    fn next_level(&self, excavated: u64) -> Option<PendingDig> {
        if self.current_depth < 10 && self.remaining > excavated {
            Some(PendingDig {
                current_depth: self.current_depth + 1,
                remaining: self.remaining - excavated,
                ..*self })
        } else {
            None
        }
    }

}

impl Ord for PendingDig {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.remaining * self.current_depth as u64)
            .cmp(&(other.remaining * other.current_depth as u64))
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
    explore_heap: &mut BinaryHeap<Explore>,
    dig_heap: &mut BinaryHeap<PendingDig>,
    treasure_heap: &mut BinaryHeap<Treasure>,
) -> Response<Option<License>> {
    while let Some(pending_cash) = treasure_heap.pop() {
        println!("cash {:#?}", pending_cash);
        for treasure in pending_cash.treasures.into_iter() {
            match client.cash(treasure.clone()).await {
                Ok(got_coins) => coins.extend(got_coins),
                Err(e) => treasure_heap.push(Treasure { depth: pending_cash.depth, treasures: vec![treasure]}),
            };
        }
    }
    if let Some(ar) = explore_heap.pop() {
        // println!("explore {:#?}", ar);
        for a in ar.area.divide().into_iter() {
            let res = client.explore(&a).await?;

            if res.amount > 0 && res.area.size() == 1 {
                dig_heap.push(PendingDig::new(ar.area.posX, ar.area.posY, ar.amount));
            } else if res.amount > 0 {
                explore_heap.push(res);
            }
        }
    }

    // todo: ordering
    let used_license = match license {
        Some(lic) if lic.digUsed < lic.digAllowed => {
            // println!("license {:#?}", lic);
            if let Some(pending_dig) = dig_heap.pop() {
                // println!("dig {:#?}", pending_dig);
                let treasure = client.dig(&pending_dig.to_dig(lic.id)).await?;

                println!("treasure {:#?}", treasure);
                if let Some(next_level) = pending_dig.next_level(treasure.len() as u64) {
                    dig_heap.push(next_level);
                }

                if treasure.len() > 0 {
                    treasure_heap.push(Treasure {
                        depth: pending_dig.current_depth,
                        treasures: treasure,
                    });
                }
                Some(License { digUsed: lic.digUsed + 1, ..*lic })
            } else {
                Some(*lic)
            }
        },
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

#[tokio::main(worker_threads = 1)]
async fn main() ->  Result<(), Box<dyn std::error::Error>> {
    println!("Started");
    let address = std::env::var("ADDRESS")?;
    let client = Client::new(&address);

    // // testing explore
    // let mut rng = thread_rng();
    // let dist = Uniform::new(0, 3400);
    //
    // for i in [10, 20, 30, 40, 50, 60, 70, 80, 90, 100].iter() {
    //     for _ in 0..10 {
    //         let x = rng.sample(dist);
    //         let y = rng.sample(dist);
    //
    //         let area = Area { posX: x, posY: y, sizeX: *i as u64, sizeY: *i as u64};
    //
    //         match explore(&client, &base_url, &area).await {
    //             Ok(r) => println!("({}, {}); {} success", x, y, i),
    //             Err(e) => println!("({}, {}); {} error {}", x, y, i, e),
    //         }
    //     }
    // }
    // testing explore

    // multiple producers, single consumer? for coins
    let mut coins: Vec<u64> = vec![];

    let mut explore_heap = BinaryHeap::new();
    let area = Area { posX: 0, posY: 0, sizeX: 3500, sizeY: 3500};
    let explore = client.explore(&area).await?;
    explore_heap.push(explore);

    let mut license: Option<License> = None;
    let mut dig_heap: BinaryHeap<PendingDig> = BinaryHeap::new();
    let mut treasure_heap: BinaryHeap<Treasure> = BinaryHeap::new();

    loop {
        println!("explore size {}", explore_heap.len());
        match logic(
            &client,
            &mut coins,
            &license,
            &mut explore_heap,
            &mut dig_heap,
            &mut treasure_heap
        ).await {
            Ok(used_license) => license = used_license,
            Err(e) => {
                // println!("licenses {:#?}", get_licenses(&client, &base_url).await?);
                println!("error {}", e)
            }
        }
    }

    Ok(())
}


#[test]
fn test_explore_ord() {
    let mut hp = BinaryHeap::new();
    hp.push(Explore { area: Area { posX: 0, posY: 0, sizeX: 100, sizeY: 100 }, amount: 10 });
    hp.push(Explore { area: Area { posX: 0, posY: 0, sizeX: 10, sizeY: 10 }, amount: 10 });
    hp.push(Explore { area: Area { posX: 0, posY: 0, sizeX: 1, sizeY: 1 }, amount: 3 });

    assert_eq!(hp.pop().unwrap().area.size(), 1);
    assert_eq!(hp.pop().unwrap().area.size(), 100);
    assert_eq!(hp.pop().unwrap().area.size(), 10000);
}

#[test]
fn test_dig_ord() {
    let mut hp = BinaryHeap::new();
    hp.push(PendingDig { x: 1, y: 0, current_depth: 2, remaining: 11 });
    hp.push(PendingDig { x: 3, y: 0, current_depth: 2, remaining: 10 });
    hp.push(PendingDig { x: 2, y: 0, current_depth: 1, remaining: 10 });

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