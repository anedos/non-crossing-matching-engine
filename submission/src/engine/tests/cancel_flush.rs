use super::*;
use crate::io::input::parse_command;

fn insert(engine: &mut OrderBookEngine, input: NewOrder) -> OrderIndex {
    let key = OrderKey::new(input.user_id, input.user_order_id);
    let _events = engine.process(Command::New(input)).unwrap();
    engine.order_locations[&key].order_index
}

fn cancel(engine: &mut OrderBookEngine, id: u64) -> Events {
    engine.process(Command::Cancel(CancelOrder {
        user_id: UserId(7),
        user_order_id: UserOrderId(id),
    })).unwrap()
}

fn assert_empty_top(events: Events, id: u64, side: Side) {
    match events {
        Events::Two(accepted, Event::TopChanged { side: actual_side, top: None }) => {
            assert_accepted(accepted, id);
            assert_eq!(actual_side, side);
        }
        _ => panic!("expected Accepted followed by an empty top"),
    }
}

fn assert_fifo(
    engine: &OrderBookEngine,
    symbol: &str,
    side: Side,
    price: Price,
    expected: &[(OrderIndex, Quantity)],
) {
    let level = &engine.books[symbol].get_by_side(side)[&price];
    assert_eq!(level.inspect(), (
        expected.iter().map(|(_, quantity)| quantity).sum(),
        expected.first().map(|(idx, _)| *idx),
        expected.last().map(|(idx, _)| *idx),
    ));
    for (position, &(idx, quantity)) in expected.iter().enumerate() {
        let previous = position.checked_sub(1).map(|i| expected[i].0);
        let next = expected.get(position + 1).map(|(idx, _)| *idx);
        assert_eq!(*engine.orders.get(idx), StoredOrder::new(quantity, previous, next));
    }
}

#[test]
fn cancelling_first_middle_or_last_order_preserves_remaining_fifo() {
    for side in [Side::Bid, Side::Ask] {
        for removed in 0..3 {
            let mut engine = OrderBookEngine::new();
            let quantities = [2, 3, 5];
            let indices = [0, 1, 2].map(|i| {
                insert(&mut engine, order(i as u64 + 1, "BTC", side, 100, quantities[i]))
            });
            let events = cancel(&mut engine, removed as u64 + 1);
            assert_events(events, removed as u64 + 1, side, Some((100, 10 - quantities[removed])));
            let remaining: Vec<_> = indices.into_iter().zip(quantities).enumerate()
                .filter_map(|(i, item)| (i != removed).then_some(item))
                .collect();
            assert_fifo(&engine, "BTC", side, 100, &remaining);
            assert_eq!(engine.order_locations.len(), 2);
            assert!(!engine.order_locations.contains_key(
                &OrderKey::new(UserId(7), UserOrderId(removed as u64 + 1))
            ));
        }
    }
}

#[test]
fn cancelling_best_level_promotes_next_price_then_empties_the_book() {
    for (side, worse) in [(Side::Bid, 90), (Side::Ask, 110)] {
        let mut engine = OrderBookEngine::new();
        insert(&mut engine, order(1, "BTC", side, 100, 2));
        let remaining = insert(&mut engine, order(2, "BTC", side, worse, 3));

        let events = cancel(&mut engine, 1);
        assert_events(events, 1, side, Some((worse, 3)));
        assert!(!engine.books["BTC"].get_by_side(side).contains_key(&100));
        assert_fifo(&engine, "BTC", side, worse, &[(remaining, 3)]);

        let events = cancel(&mut engine, 2);
        assert_empty_top(events, 2, side);
        assert!(engine.books.is_empty());
        assert!(engine.order_locations.is_empty());
    }
}

#[test]
fn cancelling_at_a_worse_price_emits_only_accepted() {
    for (side, worse) in [(Side::Bid, 90), (Side::Ask, 110)] {
        let mut engine = OrderBookEngine::new();
        let best = insert(&mut engine, order(1, "BTC", side, 100, 2));
        insert(&mut engine, order(2, "BTC", side, worse, 3));
        let other = insert(&mut engine, order(3, "BTC", side, worse, 5));

        let events = cancel(&mut engine, 2);
        assert_events(events, 2, side, None);
        assert_fifo(&engine, "BTC", side, worse, &[(other, 5)]);
        let events = cancel(&mut engine, 3);
        assert_events(events, 3, side, None);
        assert!(!engine.books["BTC"].get_by_side(side).contains_key(&worse));
        assert_fifo(&engine, "BTC", side, 100, &[(best, 2)]);
    }
}

#[test]
fn cancellation_keeps_other_symbols_sides_and_levels_unchanged() {
    for (side, opposite, other_price, worse) in [
        (Side::Bid, Side::Ask, 110, 90),
        (Side::Ask, Side::Bid, 90, 110),
    ] {
        let mut engine = OrderBookEngine::new();
        insert(&mut engine, order(1, "BTC", side, 100, 2));
        let opposite_idx = insert(&mut engine, order(2, "BTC", opposite, other_price, 3));
        let other_idx = insert(&mut engine, order(3, "ETH", side, 100, 5));
        insert(&mut engine, order(4, "BTC", side, worse, 7));
        let other_book = engine.books["ETH"].clone();
        let other_side = engine.books["BTC"].get_by_side(opposite).clone();

        let events = cancel(&mut engine, 1);
        assert_events(events, 1, side, Some((worse, 7)));
        let events = cancel(&mut engine, 4);
        assert_empty_top(events, 4, side);
        assert!(engine.books["BTC"].get_by_side(side).is_empty());
        assert_eq!(engine.books["BTC"].get_by_side(opposite), &other_side);
        assert_eq!(engine.books["ETH"], other_book);
        assert_eq!(*engine.orders.get(opposite_idx), StoredOrder::new(3, None, None));
        assert_eq!(*engine.orders.get(other_idx), StoredOrder::new(5, None, None));
        assert_eq!(engine.order_locations.len(), 2);

        // The cancelled best price no longer participates in crossing checks.
        let incoming_price = match side { Side::Bid => 95, Side::Ask => 105 };
        let events = engine.process_new(order(5, "BTC", opposite, incoming_price, 1)).unwrap();
        assert_events(events, 5, opposite, Some((incoming_price, 1)));
    }
}

#[test]
fn unknown_repeated_and_wrong_user_cancellation_do_not_mutate_state() {
    let mut engine = OrderBookEngine::new();
    insert(&mut engine, order(1, "BTC", Side::Bid, 100, 2));
    let before = snapshot(&engine);
    for (user, id) in [(7, 99), (8, 1)] {
        let key = OrderKey::new(UserId(user), UserOrderId(id));
        let result = engine.process(Command::Cancel(CancelOrder {
            user_id: key.user_id,
            user_order_id: key.user_order_id,
        }));
        assert!(matches!(result, Err(EngineError::UnknownOrder(actual)) if actual == key));
        assert_eq!(snapshot(&engine), before);
    }

    let events = cancel(&mut engine, 1);
    assert_empty_top(events, 1, Side::Bid);
    // Reuse the released slot under a different identity; the old identity must stay invalid.
    insert(&mut engine, order(2, "ETH", Side::Ask, 200, 3));
    let before = snapshot(&engine);
    let result = engine.process(Command::Cancel(CancelOrder {
        user_id: UserId(7),
        user_order_id: UserOrderId(1),
    }));
    assert!(matches!(result, Err(EngineError::UnknownOrder(key))
        if key == OrderKey::new(UserId(7), UserOrderId(1))));
    assert_eq!(snapshot(&engine), before);
}

#[test]
fn zero_total_level_remains_until_its_last_order_is_cancelled() {
    for side in [Side::Bid, Side::Ask] {
        let mut engine = OrderBookEngine::new();
        insert(&mut engine, order(1, "BTC", side, 100, 5));
        let zero = insert(&mut engine, order(2, "BTC", side, 100, 0));
        let another_zero = insert(&mut engine, order(3, "BTC", side, 100, 0));

        let events = cancel(&mut engine, 1);
        assert_events(events, 1, side, Some((100, 0)));
        assert_fifo(&engine, "BTC", side, 100, &[(zero, 0), (another_zero, 0)]);
        assert!(!engine.books["BTC"].get_by_side(side)[&100].is_empty());
        let events = cancel(&mut engine, 2);
        assert_events(events, 2, side, None);
        assert_fifo(&engine, "BTC", side, 100, &[(another_zero, 0)]);
        let events = cancel(&mut engine, 3);
        assert_empty_top(events, 3, side);
        assert!(engine.books.is_empty());
    }
}

#[test]
fn freed_slots_are_reused_in_reverse_removal_order_with_fresh_links() {
    for side in [Side::Bid, Side::Ask] {
        let mut engine = OrderBookEngine::new();
        let first = insert(&mut engine, order(1, "BTC", side, 100, 2));
        let middle = insert(&mut engine, order(2, "BTC", side, 100, 3));
        let last = insert(&mut engine, order(3, "BTC", side, 100, 5));
        assert_events(cancel(&mut engine, 2), 2, side, Some((100, 7)));
        assert_events(cancel(&mut engine, 1), 1, side, Some((100, 5)));

        // Reuse an identity and slot, but append to the FIFO rather than the old position.
        let reused_first = insert(&mut engine, order(1, "BTC", side, 100, 7));
        assert_eq!(reused_first, first);
        assert_fifo(&engine, "BTC", side, 100, &[(last, 5), (first, 7)]);
        // The next free slot can belong to an entirely different symbol without stale links.
        let reused_middle = insert(&mut engine, order(2, "ETH", side, 200, 11));
        assert_eq!(reused_middle, middle);
        assert_fifo(&engine, "ETH", side, 200, &[(middle, 11)]);
        let appended = insert(&mut engine, order(4, "BTC", side, 100, 13));
        assert_eq!(appended, OrderIndex(3));
        assert_fifo(&engine, "BTC", side, 100, &[(last, 5), (first, 7), (appended, 13)]);

        // Removing again after reuse still repairs both links and quantities.
        assert_events(cancel(&mut engine, 1), 1, side, Some((100, 18)));
        assert_fifo(&engine, "BTC", side, 100, &[(last, 5), (appended, 13)]);
    }
}

#[test]
fn cancellation_from_maximum_total_allows_new_quantity() {
    for side in [Side::Bid, Side::Ask] {
        let mut engine = OrderBookEngine::new();
        insert(&mut engine, order(1, "BTC", side, 100, Quantity::MAX - 5));
        insert(&mut engine, order(2, "BTC", side, 100, 5));
        assert_events(cancel(&mut engine, 2), 2, side, Some((100, Quantity::MAX - 5)));
        let events = engine.process_new(order(2, "BTC", side, 100, 5)).unwrap();
        assert_events(events, 2, side, Some((100, Quantity::MAX)));
    }
}

#[test]
fn flush_is_silent_and_resets_empty_or_populated_engines() {
    for populated in [false, true] {
        let mut engine = OrderBookEngine::new();
        if populated {
            insert(&mut engine, order(1, "BTC", Side::Bid, 100, 2));
            insert(&mut engine, order(2, "BTC", Side::Ask, 110, 3));
            insert(&mut engine, order(3, "ETH", Side::Bid, 200, 5));
            insert(&mut engine, order(4, "ETH", Side::Ask, 210, 7));
            assert_empty_top(cancel(&mut engine, 2), 2, Side::Ask);
            assert_empty_top(cancel(&mut engine, 3), 3, Side::Bid);
        }
        for _ in 0..2 {
            assert!(matches!(engine.process(Command::Flush).unwrap(), Events::None));
            assert_eq!(snapshot(&engine), snapshot(&OrderBookEngine::new()));
        }

        let missing = engine.process(Command::Cancel(CancelOrder {
            user_id: UserId(7),
            user_order_id: UserOrderId(1),
        }));
        assert!(matches!(missing, Err(EngineError::UnknownOrder(_))));
        // Reuse a pre-flush identity at a price that would have crossed the old book.
        let events = engine.process_new(order(1, "BTC", Side::Ask, 90, 11)).unwrap();
        assert_events(events, 1, Side::Ask, Some((90, 11)));
        let idx = assert_location(&engine, 1, "BTC", Side::Ask, 90);
        assert_eq!(idx, OrderIndex(0));
        assert_fifo(&engine, "BTC", Side::Ask, 90, &[(idx, 11)]);
    }
}

fn event_record(event: Event) -> csv::StringRecord {
    let fields = match event {
        Event::Accepted { user_id, user_order_id } => {
            vec!["A".to_owned(), user_id.0.to_string(), user_order_id.0.to_string()]
        }
        Event::Rejected { user_id, user_order_id } => {
            vec!["R".to_owned(), user_id.0.to_string(), user_order_id.0.to_string()]
        }
        Event::TopChanged { side, top } => {
            let side = match side { Side::Bid => "B", Side::Ask => "S" };
            let (price, quantity) = match top {
                Some(top) => {
                    let (price, quantity) = top.inspect();
                    (price.to_string(), quantity.to_string())
                }
                None => ("-".to_owned(), "-".to_owned()),
            };
            vec!["B".to_owned(), side.to_owned(), price, quantity]
        }
    };
    csv::StringRecord::from(fields)
}

#[test]
fn engine_events_match_all_twelve_supplied_fixtures() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    for case in 1..=12 {
        let path = fixtures.join(case.to_string());
        let mut input = csv::ReaderBuilder::new()
            .has_headers(false).flexible(true).trim(csv::Trim::All)
            .from_path(path.join("in.csv")).unwrap();
        let mut engine = OrderBookEngine::new();
        let mut actual = Vec::new();
        for (line, record) in input.records().enumerate() {
            let command = parse_command(&record.unwrap()).unwrap();
            let events = engine.process(command)
                .unwrap_or_else(|error| panic!("fixture {case}, record {}: {error:?}", line + 1));
            match events {
                Events::None => {}
                Events::One(event) => actual.push(event_record(event)),
                Events::Two(first, second) => {
                    actual.push(event_record(first));
                    actual.push(event_record(second));
                }
            }
        }
        let expected: Vec<_> = csv::ReaderBuilder::new()
            .has_headers(false).flexible(true).trim(csv::Trim::All)
            .from_path(path.join("out.csv")).unwrap()
            .records().collect::<Result<_, _>>().unwrap();
        assert_eq!(actual, expected, "fixture {case}");
        assert_eq!(snapshot(&engine), snapshot(&OrderBookEngine::new()), "fixture {case} must end flushed");
    }
}
