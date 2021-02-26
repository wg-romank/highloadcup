use crate::dto::*;

use reqwest::Error;

pub struct Client {
    client: reqwest::Client,
    explore_url: String,
    licenses_url: String,
    dig_url: String,
    cash_url: String,
}

impl Client {
    pub fn new(address: &str) -> Client {
        let client = reqwest::Client::new();
        let base_url = format!("http://{}:8000", address);
        println!("Base url {}", base_url);
        Client {
            client,
            explore_url: base_url.clone() + "/explore",
            licenses_url: base_url.clone() + "/licenses",
            dig_url: base_url.clone() + "/dig",
            cash_url: base_url.clone() + "/cash"
        }
    }
}

pub type ClientResponse<T> = Result<T, DescriptiveError>;

#[derive(Debug)]
pub struct DescriptiveError {
    message: String
}

impl std::convert::From<Error> for DescriptiveError {
    fn from(e: Error) -> Self {
        DescriptiveError { message: format!("{}", e) }
    }
}

impl std::fmt::Display for DescriptiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "err: {}\n", &self.message)
    }
}

impl Client {
    pub async fn explore(&self, area: &Area) -> ClientResponse<Explore> {
        let response = self.client.post(&self.explore_url)
                .json(area)
                .send()
                .await?;

        Ok(response.json::<Explore>().await?)
    }

    pub async fn get_license(&self, coins: Vec<u64>) -> ClientResponse<License> {
        let response = self.client.post(&self.licenses_url)
            .json(&coins)
            .send()
            .await?;

        Ok(response.json::<License>().await?)
    }

    pub async fn dig(&self, dig: &Dig) -> ClientResponse<Vec<String>> {
        let response = self.client.post(&self.dig_url)
            .json(dig)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json::<Vec<String>>().await?),
            reqwest::StatusCode::NOT_FOUND => Ok(vec![]),
            _ => Ok(response.json::<Vec<String>>().await?),
        }
    }

    pub async fn cash(&self, treasure: String) -> ClientResponse<Vec<u64>> {
        let response = self.client.post(&self.cash_url)
            .json(&treasure)
            .send()
            .await?;

        Ok(response.json::<Vec<u64>>().await?)
    }
}
