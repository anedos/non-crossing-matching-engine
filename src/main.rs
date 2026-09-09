mod logging;

fn main() -> anyhow::Result<()> {
    logging::init();
    non_crossing_matching_engine::run(std::io::stdin().lock(), std::io::stdout().lock())
}
