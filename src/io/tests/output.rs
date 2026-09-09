use std::io::{self, Write};

use crate::engine::OrderBookEngine;
use crate::io::output::write_event;
use crate::model::command::NewOrder;
use crate::model::event::Event;
use crate::model::order::{Side, Symbol, UserId, UserOrderId};

#[test]
fn every_event_shape_supports_custom_separators() {
    crate::test_logging::init_test_logging();
    for separator in [", ", "|", "\t"] {
        for (side, side_token) in [(Side::Bid, "B"), (Side::Ask, "S")] {
            let mut engine = OrderBookEngine::new();
            let events = engine
                .process(crate::model::command::Command::New(NewOrder {
                    user_id: UserId(u64::MAX),
                    user_order_id: UserOrderId(u64::MAX),
                    symbol: Symbol::from("BTC"),
                    side,
                    price: u64::MAX,
                    quantity: u64::MAX,
                }))
                .unwrap();
            let mut output = Vec::new();
            for event in events {
                write_event(&mut output, &event, separator).unwrap();
            }
            write_event(
                &mut output,
                &Event::Rejected {
                    user_id: UserId(0),
                    user_order_id: UserOrderId(0),
                },
                separator,
            )
            .unwrap();
            write_event(
                &mut output,
                &Event::TopChanged { side, top: None },
                separator,
            )
            .unwrap();
            let max = u64::MAX;
            assert_eq!(
                String::from_utf8(output).unwrap(),
                format!(
                    "A{separator}{max}{separator}{max}\n\
                 B{separator}{side_token}{separator}{max}{separator}{max}\n\
                 R{separator}0{separator}0\n\
                 B{separator}{side_token}{separator}-{separator}-\n"
                )
            );
        }
    }
}

struct FailingWriter {
    remaining: usize,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"));
        }
        let written = bytes.len().min(self.remaining);
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn write_errors_preserve_the_io_cause_at_every_byte_boundary() {
    crate::test_logging::init_test_logging();
    let event = Event::Accepted {
        user_id: UserId(1),
        user_order_id: UserOrderId(2),
    };
    for remaining in 0..b"A, 1, 2\n".len() {
        let error = write_event(
            &mut FailingWriter { remaining },
            &event,
            ", ",
        )
        .unwrap_err();
        let cause = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<io::Error>())
            .unwrap();
        assert_eq!(cause.kind(), io::ErrorKind::BrokenPipe);
    }
}
