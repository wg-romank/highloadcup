use crate::constants::{AVG_DIG_MS, TIME_LIMIT_MS};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub pos_x: u64,
    pub pos_y: u64,
    pub size_x: u64,
    pub size_y: u64,
}

impl Area {
    pub fn initial_stripe(w: u64, h: u64, i: u64) -> Self {
        Self { pos_x: w * i, pos_y: 0, size_x: w, size_y: h }
    }

    pub fn split_in_8(self) -> Vec<Area> {
        self.divide().into_iter().flat_map(|a| a.divide()).collect()
    }

    pub fn size(&self) -> u64 {
        self.size_x * self.size_y
    }
    pub fn split_x(self) -> Vec<Area> {
        let half_x = (self.size_x as f64 / 2.).floor() as u64;

        let mut result = vec![];
        if half_x > 0 && self.size_x > half_x {
            result.push(Area {
                pos_x: self.pos_x,
                pos_y: self.pos_y,
                size_x: half_x,
                size_y: self.size_y,
            });
            result.push(Area {
                pos_x: self.pos_x + half_x,
                pos_y: self.pos_y,
                size_x: self.size_x - half_x,
                size_y: self.size_y,
            });

            result
        } else {
            vec![self]
        }
    }

    pub fn split_y(self) -> Vec<Area> {
        let half_y = (self.size_y as f64 / 2.).floor() as u64;

        let mut result = vec![];
        if half_y > 0 && self.size_y > half_y {
            result.push(Area {
                pos_x: self.pos_x,
                pos_y: self.pos_y,
                size_x: self.size_x,
                size_y: half_y,
            });
            result.push(Area {
                pos_x: self.pos_x,
                pos_y: self.pos_y + half_y,
                size_x: self.size_x,
                size_y: self.size_y - half_y,
            });

            result
        } else {
            vec![self]
        }
    }

    pub fn divide(self) -> Vec<Area> {
        self.split_x()
            .into_iter()
            .flat_map(|a| a.split_y())
            .collect::<Vec<Area>>()
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Default)]
pub struct Explore {
    pub area: Area,
    pub amount: u64,
}

impl Explore {
    pub fn cost(&self, max_depth: u8) -> u128 {
        // todo: constants are no good here
        self.area.size() as u128 * (max_depth as u128 / 3) * AVG_DIG_MS
    }

    pub fn is_managable(&self, started: Instant, max_depth: u8) -> bool {
        let time_since_started_ms = started.elapsed().as_millis();
        let remaining_time_ms = TIME_LIMIT_MS - time_since_started_ms;
        self.cost(max_depth) < remaining_time_ms
    }
}

impl Ord for Explore {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.amount / self.area.size()).cmp(&(other.amount / other.area.size()))
        // .then(self.area.size().cmp(&other.area.size()).reverse())
    }
}

impl PartialOrd for Explore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub id: u64,
    pub dig_allowed: u8,
    pub dig_used: u8,
}

impl License {
    pub fn increment(&mut self) -> bool {
        self.dig_used += 1;
        self.dig_allowed > self.dig_used
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Dig {
    #[serde(rename = "licenseID")]
    pub license_id: u64,
    pub pos_x: u64,
    pub pos_y: u64,
    pub depth: u8,
}
