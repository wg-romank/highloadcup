use reqwest::Client;
use serde::{Serialize, Deserialize};

#[derive(Debug, Deserialize)]
struct Wallet {
    balance: u64,
    wallet: Vec<u64>,
}


#[derive(Debug, Serialize, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
struct Explore {
    area: Area,
    amount: u64,
}

type Response<T> = Result<T, reqwest::Error>;

async fn get_balance(address: &str) -> Response<Wallet> {
    reqwest::get(format!("http://{}:8000/balance", address).as_str())
        .await?
        .json::<Wallet>()
        .await
}

async fn explore(client: &Client, address: &str, area: Area) -> Response<Explore> {
    client.post(format!("http://{}:8000/explore", address).as_str())
        .json(&area)
        .send()
        .await?
        .json::<Explore>()
        .await
}

#[tokio::main(worker_threads = 1)]
async fn main() ->  Result<(), Box<dyn std::error::Error>> {
    println!("Started");
    let address = std::env::var("ADDRESS")?;
    println!("Address {}", address);
    let client = reqwest::Client::new();
    println!("Created client");

    for x in 0..3500 {
        for y in 0..3500 {
            println!("Posting to {} {}", x, y);
            let result = explore(&client, &address, Area::from(x, y)).await?;
            println!("Result {:#?}", result);
        }
    }

    Ok(())
}
