use std::io::{self, Write};
use std::process::{Command, Stdio};

use non_crossing_matching_engine::engine::OrderBookEngine;
use non_crossing_matching_engine::io::output::write_event;
use non_crossing_matching_engine::model::command::NewOrder;
use non_crossing_matching_engine::model::event::Event;
use non_crossing_matching_engine::model::order::{Side, Symbol, UserId, UserOrderId};

#[test]
fn every_event_shape_supports_custom_separators() {
    for separator in [", ", "|", "\t"] {
        for (side, side_token) in [(Side::Bid, "B"), (Side::Ask, "S")] {
            let mut engine = OrderBookEngine::new();
            let events = engine
                .process(non_crossing_matching_engine::model::command::Command::New(NewOrder {
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
    fail_flush: bool,
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
        if self.fail_flush {
            Err(io::Error::new(io::ErrorKind::Other, "flush failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn write_errors_preserve_the_io_cause_at_every_byte_boundary() {
    let event = Event::Accepted {
        user_id: UserId(1),
        user_order_id: UserOrderId(2),
    };
    for remaining in 0..b"A, 1, 2\n".len() {
        let error = write_event(
            &mut FailingWriter {
                remaining,
                fail_flush: false,
            },
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

#[test]
fn run_propagates_buffered_write_and_flush_failures() {
    for writer in [
        FailingWriter {
            remaining: 0,
            fail_flush: false,
        },
        FailingWriter {
            remaining: usize::MAX,
            fail_flush: true,
        },
    ] {
        let error = non_crossing_matching_engine::run(&b"N,1,BTC,10,2,B,1\n"[..], writer).unwrap_err();
        assert_eq!(error.to_string(), "failed to flush output");
        assert!(error.chain().any(|cause| cause.is::<io::Error>()));
    }
}

#[test]
fn executable_matches_all_twelve_fixtures_byte_for_byte() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    for case in 1..=12 {
        let path = fixtures.join(case.to_string());
        let input = std::fs::read(path.join("in.csv")).unwrap();
        let expected = std::fs::read(path.join("out.csv")).unwrap();

        let mut library_output = Vec::new();
        non_crossing_matching_engine::run(input.as_slice(), &mut library_output).unwrap();
        assert_eq!(library_output, expected, "library fixture {case}");

        let child = Command::new(env!("CARGO_BIN_EXE_non_crossing_matching_engine"))
            .env_remove("RUST_LOG")
            .stdin(Stdio::from(
                std::fs::File::open(path.join("in.csv")).unwrap(),
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "fixture {case}: {:?}",
            output.stderr
        );
        assert!(
            output.stderr.is_empty(),
            "fixture {case}: unexpected diagnostics"
        );
        assert_eq!(output.stdout, expected, "executable fixture {case}");
    }
}
