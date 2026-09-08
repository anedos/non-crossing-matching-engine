use std::io::{self, Write};
use std::process::{Command, Stdio};

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
