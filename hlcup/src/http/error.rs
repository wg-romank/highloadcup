use reqwest::Error;

#[derive(Debug)]
pub struct DescriptiveError {
    pub message: String,
}

impl DescriptiveError {
    pub fn new(endpoint: &str, status_code: reqwest::StatusCode, message: String) -> DescriptiveError {
        DescriptiveError {
            message: format!("{} /{}: {}", status_code, endpoint, message),
        }
    }
}

impl std::convert::From<Error> for DescriptiveError {
    fn from(e: Error) -> Self {
        DescriptiveError {
            message: format!("{}", e),
        }
    }
}

impl std::fmt::Display for DescriptiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "err: {}", &self.message)
    }
}
