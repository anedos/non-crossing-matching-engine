FROM rust:1.75-slim

RUN apt-get update && apt-get install -y \
    build-essential \
    bc \
    netcat-openbsd \
    python3 \
    python3-pip \
    && pip3 install --break-system-packages --no-cache-dir junit2html \
    && rm -rf /var/lib/apt/lists/*

COPY ./submission /non_crossing_matching_engine/
COPY ./test /test

WORKDIR /non_crossing_matching_engine
RUN cargo build --release

# Default test mode (can be overridden with docker run -e TEST_MODE=udp)
ENV TEST_MODE=stdin

ENTRYPOINT ["/bin/bash", "-c", "cd /test && ./run_tests.sh --mode ${TEST_MODE} --bin /non_crossing_matching_engine/target/release/non_crossing_matching_engine; EXIT_CODE=$?; if [ -d '/reports' ]; then cp report.xml report.html /reports/ 2>/dev/null || true; fi; exit $EXIT_CODE"]

