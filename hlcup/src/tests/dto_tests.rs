use crate::http::dto::Explore;
use crate::http::dto::Area;

#[test]
fn test_area_divide() {
    fn hash(area: &Area) -> String {
        format!(
            "[{}, {}; {}, {}]",
            area.pos_x, area.pos_y, area.size_x, area.size_y
        )
    }

    let a = Area {
        pos_x: 0,
        pos_y: 0,
        size_x: 10,
        size_y: 10,
    };

    let division = a.divide();

    let items = division.iter().map(|a| hash(a)).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 5, 5]",
            "[0, 5; 5, 5]",
            "[5, 0; 5, 5]",
            "[5, 5; 5, 5]"
        ],
        items
    );

    let division2 = division[0].clone().divide();

    let items2 = division2.iter().map(|a| hash(a)).collect::<Vec<String>>();

    assert_eq!(
        vec![
            "[0, 0; 2, 2]",
            "[0, 2; 2, 3]",
            "[2, 0; 3, 2]",
            "[2, 2; 3, 3]",
        ],
        items2
    );

    let b = Area {
        pos_x: 0,
        pos_y: 0,
        size_x: 1,
        size_y: 2,
    };

    let items3 = b.divide().iter().map(|a| hash(a)).collect::<Vec<String>>();

    assert_eq!(vec!["[0, 0; 1, 1]", "[0, 1; 1, 1]",], items3);

    let c = Area {
        pos_x: 0,
        pos_y: 0,
        size_x: 1,
        size_y: 1,
    };
    assert_eq!(c.clone().divide(), vec![c])
}

#[test]
fn test_explore_ord() {
    use std::collections::BinaryHeap;
    let mut hp = BinaryHeap::new();
    hp.push(Explore {
        area: Area {
            pos_x: 0,
            pos_y: 0,
            size_x: 100,
            size_y: 100,
        },
        amount: 10,
    });
    hp.push(Explore {
        area: Area {
            pos_x: 0,
            pos_y: 0,
            size_x: 10,
            size_y: 10,
        },
        amount: 10,
    });
    hp.push(Explore {
        area: Area {
            pos_x: 0,
            pos_y: 0,
            size_x: 1,
            size_y: 1,
        },
        amount: 3,
    });

    assert_eq!(hp.pop().unwrap().area.size(), 1);
    assert_eq!(hp.pop().unwrap().area.size(), 100);
    assert_eq!(hp.pop().unwrap().area.size(), 10000);
}