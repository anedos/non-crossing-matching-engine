use super::*;

// Independent reference: scan a flat insertion-ordered list, without maps or links.
fn top(orders: &[NewOrder], symbol: &str, side: Side) -> Option<(u64, u64)> {
    let prices = orders
        .iter()
        .filter(|o| o.symbol.as_ref() == symbol && o.side == side)
        .map(|o| o.price);
    let price = match side {
        Side::Bid => prices.max(),
        Side::Ask => prices.min(),
    }?;
    let total = orders
        .iter()
        .filter(|o| o.symbol.as_ref() == symbol && o.side == side && o.price == price)
        .map(|o| o.quantity)
        .sum();
    Some((price, total))
}

fn expected_events(
    user: UserId,
    id: UserOrderId,
    side: Side,
    before: Option<(u64, u64)>,
    after: Option<(u64, u64)>,
) -> String {
    let mut output = format!("A, {}, {}\n", user.0, id.0);
    if before != after {
        let token = if side == Side::Bid { "B" } else { "S" };
        match after {
            Some((price, quantity)) => {
                output.push_str(&format!("B, {token}, {price}, {quantity}\n"))
            }
            None => output.push_str(&format!("B, {token}, -, -\n")),
        }
    }
    output
}

fn assert_state(engine: &OrderBookEngine, reference: &[NewOrder]) {
    assert_eq!(engine.order_locations.len(), reference.len());
    let symbols: std::collections::HashSet<_> =
        reference.iter().map(|o| o.symbol.as_ref()).collect();
    assert_eq!(engine.books.len(), symbols.len());
    for symbol in symbols {
        let book = &engine.books[symbol];
        for side in [Side::Bid, Side::Ask] {
            assert_eq!(
                book.get_top(side).map(|t| t.inspect()),
                top(reference, symbol, side)
            );
            let prices: std::collections::HashSet<_> = reference
                .iter()
                .filter(|o| o.symbol.as_ref() == symbol && o.side == side)
                .map(|o| o.price)
                .collect();
            assert_eq!(book.get_by_side(side).len(), prices.len());
            for price in prices {
                let orders: Vec<_> = reference
                    .iter()
                    .filter(|o| o.symbol.as_ref() == symbol && o.side == side && o.price == price)
                    .collect();
                let indices: Vec<_> = orders
                    .iter()
                    .map(|o| {
                        let location =
                            &engine.order_locations[&OrderKey::new(o.user_id, o.user_order_id)];
                        assert_eq!(
                            (location.symbol.as_ref(), location.side, location.price),
                            (symbol, side, price)
                        );
                        location.order_index
                    })
                    .collect();
                assert_eq!(
                    book.get_by_side(side)[&price].inspect(),
                    (
                        orders.iter().map(|o| o.quantity).sum(),
                        indices.first().copied(),
                        indices.last().copied()
                    )
                );
                for (i, order) in orders.iter().enumerate() {
                    assert_eq!(
                        *engine.orders.get(indices[i]),
                        StoredOrder::new(
                            order.quantity,
                            i.checked_sub(1).map(|j| indices[j]),
                            indices.get(i + 1).copied()
                        )
                    );
                }
            }
        }
    }
}

#[test]
fn mixed_sequences_match_flat_reference_and_preserve_fifo() {
    for seed in [1_u64, 17, 982_451_653, u64::MAX] {
        let mut random = seed;
        let mut next = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            random
        };
        let mut engine = OrderBookEngine::new();
        let mut reference: Vec<NewOrder> = Vec::new();
        for step in 0..2_000 {
            let user = UserId(next() % 3);
            let id = UserOrderId(next() % 32);
            let kind = next() % 100;
            let (result, expected) = if kind == 0 {
                reference.clear();
                (engine.process(Command::Flush), String::new())
            } else if kind < 40 {
                let result = engine.process(Command::Cancel(CancelOrder {
                    user_id: user,
                    user_order_id: id,
                }));
                if let Some(position) = reference
                    .iter()
                    .position(|o| o.user_id == user && o.user_order_id == id)
                {
                    let old = &reference[position];
                    let before = top(&reference, &old.symbol, old.side);
                    let removed = reference.remove(position);
                    let after = top(&reference, &removed.symbol, removed.side);
                    (
                        result,
                        expected_events(user, id, removed.side, before, after),
                    )
                } else {
                    assert!(
                        matches!(result, Err(EngineError::UnknownOrder(key)) if key == OrderKey::new(user, id))
                    );
                    assert_state(&engine, &reference);
                    continue;
                }
            } else {
                let symbol = ["BTC", "ETH", "SOL"][next() as usize % 3];
                let side = if next() % 2 == 0 {
                    Side::Bid
                } else {
                    Side::Ask
                };
                let price = next() % 12;
                let quantity = next() % 10; // Includes live zero-quantity orders.
                let make = || NewOrder {
                    user_id: user,
                    user_order_id: id,
                    symbol: Symbol::from(symbol),
                    side,
                    price,
                    quantity,
                };
                let result = engine.process(Command::New(make()));
                if reference
                    .iter()
                    .any(|o| o.user_id == user && o.user_order_id == id)
                {
                    assert!(
                        matches!(result, Err(EngineError::DuplicateOrder(key)) if key == OrderKey::new(user, id))
                    );
                    assert_state(&engine, &reference);
                    continue;
                }
                let crosses = reference.iter().any(|o| {
                    o.symbol.as_ref() == symbol
                        && o.side != side
                        && match side {
                            Side::Bid => price >= o.price,
                            Side::Ask => price <= o.price,
                        }
                });
                if crosses {
                    (result, format!("R, {}, {}\n", user.0, id.0))
                } else {
                    let before = top(&reference, symbol, side);
                    reference.push(make());
                    (
                        result,
                        expected_events(user, id, side, before, top(&reference, symbol, side)),
                    )
                }
            };
            let mut actual = Vec::new();
            for event in result.unwrap() {
                crate::io::output::write_event(&mut actual, &event, ", ").unwrap();
            }
            assert_eq!(
                String::from_utf8(actual).unwrap(),
                expected,
                "seed {seed}, step {step}"
            );
            assert_state(&engine, &reference);
        }
    }
}
