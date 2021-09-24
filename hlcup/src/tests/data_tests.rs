use crate::models::data::PendingDig;
use crate::models::data::Treasure;

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