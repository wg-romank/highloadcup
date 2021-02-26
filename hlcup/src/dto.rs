use std::cmp::Ordering;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub struct Area {
    pub posX: u64,
    pub posY: u64,
    pub sizeX: u64,
    pub sizeY: u64,
}

impl Area {
    pub fn size(&self) -> u64 { self.sizeX * self.sizeY }
    pub fn divide(&self) -> Vec<Area> {
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

        if result.is_empty() {
            result.push(*self);
        }

        result
    }

    fn hash(&self) -> String {
        format!("[{}, {}; {}, {}]", self.posX, self.posY, self.sizeX, self.sizeY)
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
pub struct License {
    pub id: u64,
    pub digAllowed: u8,
    pub digUsed: u8,
}

#[derive(Debug, Serialize)]
pub struct Dig {
    pub licenseID: u64,
    pub posX: u64,
    pub posY: u64,
    pub depth: u8,
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

    let c = Area { posX: 0, posY: 0, sizeX: 1, sizeY: 1};
    assert_eq!(
        c.divide(),
        vec![c]
    )
}
