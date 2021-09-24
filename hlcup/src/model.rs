use crate::dto::Dig;
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Treasure {
    pub depth: u8,
    pub treasures: Vec<String>,
}

impl Ord for Treasure {
    fn cmp(&self, other: &Self) -> Ordering {
        // todo: other kind of priority
        self.depth.cmp(&other.depth)
    }
}

impl PartialOrd for Treasure {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDig {
    pub x: u64,
    pub y: u64,
    pub depth: u8,
    pub remaining: u64,
}

impl PendingDig {
    pub fn new(x: u64, y: u64, remaining: u64) -> PendingDig {
        PendingDig {
            x,
            y,
            depth: 1,
            remaining,
        }
    }

    pub fn to_dig(self, license_id: u64) -> Dig {
        Dig {
            license_id,
            pos_x: self.x,
            pos_y: self.y,
            depth: self.depth,
        }
    }

    pub fn next_level(&self, max_depth: u8, excavated: u64) -> Option<PendingDig> {
        if self.depth < max_depth && self.remaining > excavated {
            Some(PendingDig {
                depth: self.depth + 1,
                remaining: self.remaining - excavated,
                ..*self
            })
        } else {
            None
        }
    }
}

impl Ord for PendingDig {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.remaining * self.depth as u64).cmp(&(other.remaining * other.depth as u64))
    }
}

impl PartialOrd for PendingDig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[test]
fn test_treasure_ord() {
    use std::collections::BinaryHeap;

    let mut hp = BinaryHeap::new();
    hp.push(Treasure {
        depth: 1,
        treasures: vec![],
    });
    hp.push(Treasure {
        depth: 2,
        treasures: vec![],
    });

    assert_eq!(hp.pop().unwrap().depth, 2);
    assert_eq!(hp.pop().unwrap().depth, 1);
}

#[test]
fn test_dig_ord() {
    use std::collections::BinaryHeap;
    let mut hp = BinaryHeap::new();
    hp.push(PendingDig {
        x: 1,
        y: 0,
        depth: 2,
        remaining: 11,
    });
    hp.push(PendingDig {
        x: 3,
        y: 0,
        depth: 2,
        remaining: 10,
    });
    hp.push(PendingDig {
        x: 2,
        y: 0,
        depth: 1,
        remaining: 10,
    });

    assert_eq!(hp.pop().unwrap().x, 1);
    assert_eq!(hp.pop().unwrap().x, 3);
    assert_eq!(hp.pop().unwrap().x, 2);
}
