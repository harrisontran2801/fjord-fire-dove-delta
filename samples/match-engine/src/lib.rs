//! Tiny deterministic matching engine used as the native Quench ELF sample.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Order {
    pub id: u32,
    pub price: i32,
    pub qty: i32,
    pub buy: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fill {
    pub buy_id: u32,
    pub sell_id: u32,
    pub price: i32,
    pub qty: i32,
}

pub fn match_orders(orders: &[Order]) -> (Vec<Fill>, i64) {
    let mut buys: Vec<Order> = Vec::new();
    let mut sells: Vec<Order> = Vec::new();
    let mut fills = Vec::new();
    let mut checksum: i64 = 0;

    for &incoming in orders {
        if incoming.buy {
            let mut qty = incoming.qty;
            sells.sort_by(|a, b| a.price.cmp(&b.price).then(a.id.cmp(&b.id)));
            let mut i = 0;
            while i < sells.len() && qty > 0 {
                if sells[i].price > incoming.price {
                    break;
                }
                let take = qty.min(sells[i].qty);
                fills.push(Fill {
                    buy_id: incoming.id,
                    sell_id: sells[i].id,
                    price: sells[i].price,
                    qty: take,
                });
                checksum = checksum.wrapping_add(i64::from(take) * i64::from(sells[i].price));
                checksum = checksum.wrapping_add(i64::from(incoming.id ^ sells[i].id));
                sells[i].qty -= take;
                qty -= take;
                if sells[i].qty == 0 {
                    sells.remove(i);
                } else {
                    i += 1;
                }
            }
            if qty > 0 {
                buys.push(Order { qty, ..incoming });
            }
        } else {
            let mut qty = incoming.qty;
            buys.sort_by(|a, b| b.price.cmp(&a.price).then(a.id.cmp(&b.id)));
            let mut i = 0;
            while i < buys.len() && qty > 0 {
                if buys[i].price < incoming.price {
                    break;
                }
                let take = qty.min(buys[i].qty);
                fills.push(Fill {
                    buy_id: buys[i].id,
                    sell_id: incoming.id,
                    price: incoming.price,
                    qty: take,
                });
                checksum = checksum.wrapping_add(i64::from(take) * i64::from(incoming.price));
                checksum = checksum.wrapping_add(i64::from(buys[i].id ^ incoming.id));
                buys[i].qty -= take;
                qty -= take;
                if buys[i].qty == 0 {
                    buys.remove(i);
                } else {
                    i += 1;
                }
            }
            if qty > 0 {
                sells.push(Order { qty, ..incoming });
            }
        }
    }

    (fills, checksum)
}

pub fn synthetic_book(n: u32, seed: u64) -> Vec<Order> {
    let mut s = seed | 1;
    let mut orders = Vec::with_capacity(n as usize);
    for i in 0..n {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        let price = 90 + ((s >> 33) % 21) as i32;
        let qty = 1 + ((s >> 17) % 8) as i32;
        orders.push(Order {
            id: i + 1,
            price,
            qty,
            buy: (s & 1) == 0,
        });
    }
    orders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_crossing_orders() {
        let orders = [
            Order { id: 1, price: 100, qty: 5, buy: true },
            Order { id: 2, price: 99, qty: 3, buy: false },
        ];
        let (fills, checksum) = match_orders(&orders);
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].qty, 3);
        assert_eq!(fills[0].price, 99);
        assert!(checksum != 0);
    }

    #[test]
    fn leaves_resting_liquidity() {
        let orders = [
            Order { id: 1, price: 101, qty: 2, buy: true },
            Order { id: 2, price: 102, qty: 2, buy: false },
        ];
        let (fills, _) = match_orders(&orders);
        assert!(fills.is_empty());
    }

    #[test]
    fn synthetic_book_is_deterministic() {
        let a = synthetic_book(256, 7);
        let b = synthetic_book(256, 7);
        assert_eq!(a, b);
        let (fa, ca) = match_orders(&a);
        let (fb, cb) = match_orders(&b);
        assert_eq!(fa, fb);
        assert_eq!(ca, cb);
        assert!(!fa.is_empty());
    }
}
