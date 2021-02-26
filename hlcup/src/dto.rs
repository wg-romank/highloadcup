use std::cmp::Ordering;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub pos_x: u64,
    pub pos_y: u64,
    pub size_x: u64,
    pub size_y: u64,
}

impl Area {
    pub fn size(&self) -> u64 { self.size_x * self.size_y }
    fn split_x(&self) -> Vec<Area> {
        if self.size_x > 2 {
            let half_x = (self.size_x as f64 / 2.).floor() as u64;

            let mut result = vec![];
            if half_x > 0 && self.size_x > half_x + 1 {
                result.push(
                    Area {
                        pos_x: self.pos_x,
                        pos_y: self.pos_y,
                        size_x: half_x,
                        size_y: self.size_y
                    }
                );
                result.push(
                    Area {
                        pos_x: self.pos_x + half_x + 1,
                        pos_y: self.pos_y,
                        size_x: self.size_x - half_x - 1,
                        size_y: self.size_y,
                    }
                )
            }

            result
        } else if self.size_x == 2 {
            vec![
                Area { pos_x: self.pos_x, pos_y: self.pos_y, size_x: 1, size_y: 1 },
                Area { pos_x: self.pos_x + 1, pos_y: self.pos_y, size_x: 1, size_y: 1 },
            ]
        } else {
            vec![*self]
        }
    }

    fn split_y(&self) -> Vec<Area> {
        if self.size_y > 2 {
            let half_y = (self.size_y as f64 / 2.).floor() as u64;

            let mut result = vec![];
            if half_y > 0 && self.size_y > half_y + 1 {
                result.push(
                    Area {
                        pos_x: self.pos_x,
                        pos_y: self.pos_y,
                        size_x: self.size_x,
                        size_y: half_y
                    }
                );
                result.push(
                    Area {
                        pos_x: self.pos_x,
                        pos_y: self.pos_y + half_y + 1,
                        size_x: self.size_x,
                        size_y: self.size_y - half_y - 1,
                    }
                )
            }

            result
        } else if self.size_y == 2 {
            vec![
                Area { pos_x: self.pos_x, pos_y: self.pos_y, size_x: 1, size_y: 1 },
                Area { pos_x: self.pos_x, pos_y: self.pos_y + 1, size_x: 1, size_y: 1 },
                // Area { pos_x: self.pos_x, pos_y: self.pos_y + 2, size_x: 1, size_y: 1 }
            ]
        } else {
            vec![*self]
        }
    }

    pub fn divide(&self) -> Vec<Area> {
        self.split_x().into_iter().flat_map(|a| a.split_y()).collect::<Vec<Area>>()
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Explore {
    pub area: Area,
    pub amount: u64,
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

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub id: u64,
    pub dig_allowed: u8,
    pub dig_used: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dig {
    #[serde(rename = "licenseID")]
    pub license_id: u64,
    pub pos_x: u64,
    pub pos_y: u64,
    pub depth: u8,
}


#[test]
fn test_area_divide() {
    fn hash(area: &Area) -> String {
        format!("[{}, {}; {}, {}]", area.pos_x, area.pos_y, area.size_x, area.size_y)
    }

    let a = Area { pos_x: 0, pos_y: 0, size_x: 10, size_y: 10 };

    let division = a.divide();

    let items = division.iter().map(|a| hash(a)).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 5, 5]",
            "[0, 6; 5, 4]",
            "[6, 0; 4, 5]",
            "[6, 6; 4, 4]"
        ],
        items
    );

    let division2 = division[0].divide();

    let items2 = division2.iter().map(|a| hash(a)).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 2, 2]",
            "[0, 3; 2, 2]",
            "[3, 0; 2, 2]",
            "[3, 3; 2, 2]",
        ],
        items2
    );

    let b = Area { pos_x: 0, pos_y: 0, size_x: 1, size_y: 2 };

    let items3 = b.divide().iter().map(|a| hash(a)).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 1, 1]",
            "[0, 1; 1, 1]",
        ],
        items3
    );

    let c = Area { pos_x: 0, pos_y: 0, size_x: 1, size_y: 1};
    assert_eq!(
        c.divide(),
        vec![c]
    )
}
