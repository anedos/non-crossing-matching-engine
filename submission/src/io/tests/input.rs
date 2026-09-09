use crate::io::input::parse_command;
use crate::model::command::Command;
use crate::model::order::Side;
use csv::StringRecord;

#[test]
fn parses_all_commands_and_unsigned_boundaries() {
    let max = u64::MAX.to_string();
    for (token, expected_side) in [("B", Side::Bid), ("S", Side::Ask)] {
        let record = StringRecord::from(vec!["N", &max, "BTC,USD", "0", &max, token, &max]);
        let Command::New(order) = parse_command(&record).unwrap() else {
            panic!("new order expected")
        };
        assert_eq!(
            (order.user_id.0, order.user_order_id.0),
            (u64::MAX, u64::MAX)
        );
        assert_eq!(order.symbol.as_ref(), "BTC,USD");
        assert_eq!(
            (order.price, order.quantity, order.side),
            (0, u64::MAX, expected_side)
        );
    }
    let Command::Cancel(order) = parse_command(&StringRecord::from(vec!["C", "0", &max])).unwrap()
    else {
        panic!("cancel expected")
    };
    assert_eq!((order.user_id.0, order.user_order_id.0), (0, u64::MAX));
    assert!(matches!(
        parse_command(&StringRecord::from(vec!["F"])).unwrap(),
        Command::Flush
    ));
}

#[test]
fn rejects_invalid_shapes_types_and_numeric_overflow() {
    for fields in [
        vec![],
        vec![""],
        vec!["X"],
        vec!["F", "extra"],
        vec!["C", "1"],
        vec!["C", "1", "2", "extra"],
        vec!["N", "1", "BTC", "10", "2", "B"],
        vec!["N", "1", "BTC", "10", "2", "B", "1", "extra"],
    ] {
        assert!(
            parse_command(&StringRecord::from(fields.clone())).is_err(),
            "{fields:?}"
        );
    }
    for index in [1, 3, 4, 6] {
        for invalid in ["", "-1", "1.5", "abc", "18446744073709551616"] {
            let mut fields = vec!["N", "1", "BTC", "10", "2", "B", "1"];
            fields[index] = invalid;
            assert!(
                parse_command(&StringRecord::from(fields)).is_err(),
                "field {index}: {invalid}"
            );
        }
    }
    for side in ["", "b", "s", "BUY", "0"] {
        assert!(parse_command(&StringRecord::from(vec![
            "N", "1", "BTC", "10", "2", side, "1"
        ]))
        .is_err());
    }
    for fields in [vec!["C", "-1", "2"], vec!["C", "1", "18446744073709551616"]] {
        assert!(parse_command(&StringRecord::from(fields)).is_err());
    }
}
