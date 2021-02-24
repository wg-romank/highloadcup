use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Deserialize)]
struct Wallet {
    balance: u64,
    wallet: Vec<u64>,
}


#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
struct Area {
    posX: u64,
    posY: u64,
    sizeX: u64,
    sizeY: u64,
}

impl Area {
    fn from(x: u64, y: u64) -> Area {
        Area { posX: x, posY: y, sizeX: 1, sizeY: 1}
    }
    fn size(&self) -> u64 { self.sizeX * self.sizeY }
    fn divide(&self) -> Vec<Area> {
        // todo: watch out for overflow?
        let halfX = (self.sizeX as f64 / 2.).ceil() as u64;
        let halfY = (self.sizeY as f64 / 2.).ceil() as u64;

        let mut result = vec![];
        if halfX > 0 || halfY > 0 {
            result.push(
                Area { posX: self.posX, posY: self.posY, sizeX: halfX, sizeY: halfY }
            );
        }
        if halfX > 0 && self.sizeX - halfX > 0 {
            result.push(
                Area { posX: self.posX + halfX, posY: self.posY, sizeX: self.sizeX - halfX, sizeY: halfY }
            );
        }
        if halfY > 0 && self.sizeY - halfY > 0 {
            result.push(
            Area { posX: self.posX, posY: self.posY + halfY, sizeX: halfX, sizeY: self.sizeY - halfY }
            );
        }
        if halfX > 0 && self.sizeX - halfX > 0 && halfY > 0 && self.sizeY - halfY > 0 {
            result.push(
                Area { posX: self.posX + halfX, posY: self.posY + halfY, sizeX: self.sizeX - halfX, sizeY: self.sizeY - halfY }
            );
        };

        result
    }

    fn hash(&self) -> String {
        format!("[{}, {}; {}, {}]", self.posX, self.posY, self.sizeX, self.sizeY)
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Explore {
    area: Area,
    amount: u64,
}

impl Explore {
    // todo: should this be f64?
    fn density(&self) -> u64 { self.amount / self.area.size() }
}

impl Ord for Explore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.density().cmp(&other.density())
    }
}

impl PartialOrd for Explore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Deserialize)]
struct License {
    id: u64,
    digAllowed: u8,
    digUsed: u8,
}

#[derive(Debug, Serialize)]
struct Dig {
    licenseID: u64,
    posX: u64,
    posY: u64,
    depth: u8,
}

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

    fn deeper(&self, remaining: u64) -> Option<PendingDig> {
        Some(PendingDig {
                current_depth: self.current_depth + 1,
                remaining: self.remaining - remaining,
                ..*self })
        .filter(|d| d.current_depth <= 10 && d.remaining > 0)
    }

    fn hardness(&self) -> u8 {
        10 - self.current_depth
    }
}

impl Ord for PendingDig {
    fn cmp(&self, other: &Self) -> Ordering {
        self.remaining.cmp(&other.remaining)
            .then(self.hardness().cmp(&other.hardness()))
    }
}

impl PartialOrd for PendingDig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

#[derive(Debug, Deserialize)]
struct Treasure {
    treasures: Vec<String>
}

#[derive(Debug, PartialEq, Eq)]
struct PendingCash {
    depth: u8,
    treasures: Vec<String>,
}

impl Ord for PendingCash {
    fn cmp(&self, other: &Self) -> Ordering {
        // todo: other kind of priority
        self.depth.cmp(&other.depth)
    }
}

impl PartialOrd for PendingCash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

type Response<T> = Result<T, reqwest::Error>;

async fn get_balance(address: &str) -> Response<Wallet> {
    reqwest::get( &(address.to_owned() + "/balance"))
        .await?
        .json::<Wallet>()
        .await
}

async fn explore(client: &Client, address: &str, area: &Area) -> Response<Explore> {
    client.post(&(address.to_owned() + "/explore"))
        .json(area)
        .send()
        .await?
        .json::<Explore>()
        .await
}

async fn get_license(client: &Client, address: &str, coins: Vec<u64>) -> Response<License> {
    client.post(&(address.to_owned() + "/licenses"))
        .json(&coins)
        .send()
        .await?
        .json::<License>()
        .await
}

async fn dig(client: &Client, address: &str, dig: &Dig) -> Response<Treasure> {
    client.post(&(address.to_owned() + "/dig"))
        .json(dig)
        .send()
        .await?
        .json::<Treasure>()
        .await
}

async fn cash(client: &Client, address: &str, treasure: String) -> Response<Vec<u64>> {
    client.post(&(address.to_owned() + "/cash"))
        .json(&treasure)
        .send()
        .await?
        .json::<Vec<u64>>()
        .await
}

#[tokio::main(worker_threads = 1)]
async fn main() ->  Result<(), Box<dyn std::error::Error>> {
    println!("Started");
    let address = std::env::var("ADDRESS")?;
    let base_url = format!("http://{}:8000", address);

    println!("Base url {}", base_url);
    let client = reqwest::Client::new();
    println!("Created client");

    let mut coins: Vec<u64> = vec![];
    let area = Area { posX: 0, posY: 0, sizeX: 10, sizeY: 10 };
    let result = explore(&client, &base_url, &area).await?;

    let mut explore_heap = BinaryHeap::from(vec![result]);
    let mut license: Option<License> = None;
    let mut dig_heap: BinaryHeap<PendingDig> = BinaryHeap::new();
    let mut treasure_heap: BinaryHeap<PendingCash> = BinaryHeap::new();

    loop {
        if let Some(pending_cash) = treasure_heap.pop() {
            println!("cash {:#?}", pending_cash);
            for treasure in pending_cash.treasures.into_iter() {
                let got_coins = cash(&client, &base_url, treasure).await?;
                coins.extend(got_coins);
            }
        }
        if let Some(ar) = explore_heap.pop() {
            println!("explore {:#?}", ar);
            match ar.area.size() {
                1 => dig_heap.push(
                    PendingDig::new(ar.area.posX, ar.area.posY, ar.amount)
                ),
                // todo: speculative digging here?
                _ => for a in ar.area.divide().into_iter() {
                    let res = explore(&client, &base_url, &a).await?;
                    explore_heap.push(Explore { area: a, amount: res.amount });
                }
            }
        }

        // todo: ordering
        if !dig_heap.is_empty() {
            license = match license {
                Some(lic) if lic.digUsed < lic.digAllowed => {
                    println!("license {:#?}", lic);
                    // dig
                    if let Some(pending_dig) = dig_heap.pop() {
                        println!("dig {:#?}", pending_dig);
                        let treasure = dig(&client, &base_url, &pending_dig.to_dig(lic.id)).await?;

                        if let Some(next_level) = pending_dig.deeper(
                            treasure.treasures.len() as u64
                        ) {
                            dig_heap.push(next_level);
                        }

                        treasure_heap.push(PendingCash {
                            depth: pending_dig.current_depth,
                            treasures: treasure.treasures
                        });
                    }

                    Some(License { digUsed: lic.digUsed - 1, ..lic })
                },
                _ => Some(
                    if let Some(c) = coins.pop() {
                        get_license(&client, &base_url, vec![c]).await?
                    } else {
                        get_license(&client, &base_url, vec![]).await?
                    }
                ),
            };
        }
    }

    Ok(())
}


#[test]
fn test_area_divide() {
    let a = Area { posX: 0, posY: 0, sizeX: 10, sizeY: 10 };

    let division = a.divide();

    let items = division.iter().map(|a| a.hash()).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 5, 5]",
            "[5, 0; 5, 5]",
            "[0, 5; 5, 5]",
            "[5, 5; 5, 5]"
        ],
        items
    );

    let division2 = division[0].divide();

    let items2 = division2.iter().map(|a| a.hash()).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 3, 3]",
            "[3, 0; 2, 3]",
            "[0, 3; 3, 2]",
            "[3, 3; 2, 2]",
        ],
        items2
    );

    let b = Area { posX: 0, posY: 0, sizeX: 1, sizeY: 2 };

    let items3 = b.divide().iter().map(|a| a.hash()).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 1, 1]",
            "[0, 1; 1, 1]",
        ],
        items3
    );
}