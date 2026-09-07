/// Configure diagnostics at the application boundary, leaving stdout for CSV events.
pub(crate) fn init() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Off)
        .parse_env(env_logger::Env::default().default_filter_or("off"))
        .target(env_logger::Target::Stderr)
        .write_style(env_logger::WriteStyle::Never)
        .format_timestamp(None)
        .init();
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process::Command;

    // Run only in a child process, isolating the global logger and environment from other tests.
    #[test]
    fn logging_probe() {
        if std::env::var_os("ORDERBOOK_LOGGING_PROBE").is_none() {
            return;
        }
        super::init();
        let mut engine = non_crossing_matching_engine::engine::OrderBookEngine::new();
        let commands = [
            format!("N,7,BTC,100,{},B,1", u64::MAX - 5),
            "N,7,BTC,100,5,B,2".to_owned(),
            "N,7,BTC,100,1,B,1".to_owned(), // Duplicate identity.
            "N,7,BTC,100,1,B,3".to_owned(), // Quantity overflow.
            "N,7,BTC,100,1,S,4".to_owned(), // Crossing rejection.
            "C,7,1".to_owned(),
            "C,7,99".to_owned(), // Unknown identity.
            "F".to_owned(),
        ];
        let mut stdout = std::io::stdout().lock();
        for command in commands {
            let record = csv::StringRecord::from(command.split(',').collect::<Vec<_>>());
            let command = non_crossing_matching_engine::io::input::parse_command(&record).unwrap();
            // Use a test-only event representation; production CSV wiring is outside this change.
            writeln!(stdout, "EVENT {:?}", engine.process(command)).unwrap();
        }
        log::error!(target: "non_crossing_matching_engine::engine", "probe error diagnostic");
    }

    fn probe(filter: Option<&str>) -> (String, String) {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args(["--exact", "logging::tests::logging_probe", "--nocapture"])
            .env("ORDERBOOK_LOGGING_PROBE", "1")
            .env_remove("RUST_LOG")
            .env_remove("RUST_LOG_STYLE");
        if let Some(filter) = filter {
            child.env("RUST_LOG", filter);
        }
        let output = child.output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "filter={filter:?}\nstdout={stdout}\nstderr={stderr}");
        assert!(!stdout.contains("[DEBUG"), "debug logs leaked to stdout: {stdout}");
        assert!(!stdout.contains("[WARN"), "warning logs leaked to stdout: {stdout}");
        assert!(!stdout.contains("probe error diagnostic"), "error log leaked to stdout");
        // Exclude test-harness progress and timing lines from the event comparison.
        let events = stdout.lines().filter(|line| line.starts_with("EVENT "))
            .collect::<Vec<_>>().join("\n");
        assert_eq!(events.lines().count(), 8);
        (events, stderr)
    }

    #[test]
    fn logging_is_opt_in_stderr_only_and_preserves_events() {
        let (expected_events, stderr) = probe(None);
        assert!(stderr.is_empty(), "logging must be silent by default: {stderr}");
        for filter in ["", "off"] {
            let (events, stderr) = probe(Some(filter));
            assert_eq!(events, expected_events);
            assert!(stderr.is_empty(), "filter={filter:?}: {stderr}");
        }

        for filter in ["debug", "non_crossing_matching_engine=debug", "warn"] {
            let (events, stderr) = probe(Some(filter));
            assert_eq!(events, expected_events, "logging changed events with filter={filter}");
            for reason in ["DuplicateOrder", "QuantityOverflow", "UnknownOrder"] {
                assert_eq!(stderr.matches(&format!("reason={reason}")).count(), 1,
                    "failure should be logged exactly once: {stderr}");
            }
            assert_eq!(stderr.matches("[WARN").count(), 3);
            assert!(stderr.contains("probe error diagnostic"));
            if filter == "warn" {
                assert!(!stderr.contains("[DEBUG"));
            } else {
                for message in [
                    "processing command:", "Accepted", "Rejected", "TopChanged",
                    "cancelling order:", "flushing engine:", "symbol=BTC",
                ] {
                    assert!(stderr.contains(message), "missing {message:?}: {stderr}");
                }
            }
        }
    }
}
