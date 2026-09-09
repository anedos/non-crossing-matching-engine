use std::io::{self, Write};
use std::process::{Command, Stdio};

#[path = "support/logging.rs"]
mod test_logging;

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
            Err(io::Error::other("flush failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn run_propagates_buffered_write_and_flush_failures() {
    test_logging::init_test_logging();
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
    test_logging::init_test_logging();
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

#[test]
fn csv_whitespace_quoted_symbols_crlf_and_missing_final_newline() {
    test_logging::init_test_logging();
    let input = b"\r\n N , 1 ,\"BTC,USD\", 10 , 2 , B , 1\r\nC,1,1\r\nF";
    let mut output = Vec::new();
    non_crossing_matching_engine::run(input.as_slice(), &mut output).unwrap();
    assert_eq!(output, b"A, 1, 1\nB, B, 10, 2\nA, 1, 1\nB, B, -, -\n");
    for input in ["", "\n\r\n", "F\nF"] {
        let mut output = Vec::new();
        non_crossing_matching_engine::run(input.as_bytes(), &mut output).unwrap();
        assert!(output.is_empty());
    }
}

#[test]
fn invalid_commands_report_record_number_and_stop_processing() {
    test_logging::init_test_logging();
    for (bad, expected) in [
        ("N,1,BTC,bad,2,B,2", "invalid command in CSV record 2"),
        ("C,1,99", "failed to process command 2"),
        ("N,1,BTC,10,2,B,1", "failed to process command 2"),
    ] {
        let input = format!("N,1,BTC,10,2,B,1\n{bad}\nN,2,ETH,5,1,B,3\n");
        let mut output = Vec::new();
        let error = non_crossing_matching_engine::run(input.as_bytes(), &mut output).unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(output, b"A, 1, 1\nB, B, 10, 2\n");
    }
}

#[test]
fn read_failure_and_invalid_utf8_are_reported() {
    test_logging::init_test_logging();
    struct FailedReader;
    impl io::Read for FailedReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "input disconnected",
            ))
        }
    }
    let error = non_crossing_matching_engine::run(FailedReader, Vec::new()).unwrap_err();
    assert_eq!(error.to_string(), "failed to read CSV record 1");
    assert!(format!("{error:#}").contains("input disconnected"));
    let error = non_crossing_matching_engine::run(&b"N,1,\xff,10,2,B,1\n"[..], Vec::new()).unwrap_err();
    assert_eq!(error.to_string(), "failed to read CSV record 1");
}

#[test]
fn buffered_write_failure_during_processing_is_reported() {
    test_logging::init_test_logging();
    let input = "N,1,BTC,10,1,B,1\nC,1,1\n".repeat(1_000);
    let error = non_crossing_matching_engine::run(
        input.as_bytes(),
        FailingWriter {
            remaining: 0,
            fail_flush: false,
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "failed to write output record");
    assert!(error.chain().any(|cause| cause
        .downcast_ref::<io::Error>()
        .is_some_and(|cause| cause.kind() == io::ErrorKind::BrokenPipe)));
}

#[test]
fn executable_returns_failure_and_diagnostics_for_invalid_input() {
    test_logging::init_test_logging();
    let mut child = Command::new(env!("CARGO_BIN_EXE_non_crossing_matching_engine"))
        .env_remove("RUST_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"X\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("invalid command in CSV record 1"));
}
