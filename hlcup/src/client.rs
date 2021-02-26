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

impl DescriptiveError {
    fn new(endpoint: &str, status_code: reqwest::StatusCode, message: String) -> DescriptiveError {
        DescriptiveError {
            message: format!("{} /{}: {}", status_code, endpoint, message)
        }
    }
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

        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json::<Explore>().await?),
            status => Err(DescriptiveError::new("explore",status, response.text().await?)),
        }
    }

    pub async fn get_license(&self, coins: Vec<u64>) -> ClientResponse<License> {
        let response = self.client.post(&self.licenses_url)
            .json(&coins)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json::<License>().await?),
            status => Err(DescriptiveError::new("license",status, response.text().await?)),
        }

    }

    pub async fn dig(&self, dig: &Dig) -> ClientResponse<Vec<String>> {
        let response = self.client.post(&self.dig_url)
            .json(dig)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json::<Vec<String>>().await?),
            reqwest::StatusCode::NOT_FOUND => Ok(vec![]),
            status => Err(DescriptiveError::new(
                "dig",
                status,
                format!("{} {} {}", dig.pos_x, dig.pos_y, dig.depth) + &response.text().await?)),
        }
    }

    pub async fn cash(&self, treasure: String) -> ClientResponse<Vec<u64>> {
        let response = self.client.post(&self.cash_url)
            .json(&treasure)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json::<Vec<u64>>().await?),
            status => Err(DescriptiveError::new("cash",status, response.text().await?)),
        }
    }
}
