pub mod engine;
pub mod io;
pub mod model;

use std::io::{Read, Write};

use anyhow::{Context, Result};
use csv::{ReaderBuilder, WriterBuilder};

use engine::OrderBookEngine;
use io::input::parse_command;
use io::output::write_event;

pub fn run<R, W>(reader: R, writer: W) -> Result<()>
where
    R: Read,
    W: Write,
{
    let mut engine = OrderBookEngine::new();
    let mut line = String::new();

    // in a loop, read a line from reader
    // parse the command from the line
    // process the command, return Events
    // iterate the events and write them to writer
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(reader);

    let mut writer = std::io::BufWriter::new(writer);

    let mut engine = OrderBookEngine::new();

    for (record_index, record) in reader.records().enumerate() {
        let record_number = record_index + 1;

        let record = record
            .with_context(|| format!("failed to read CSV record {record_number}"))?;

        let command = parse_command(&record)
            .with_context(|| format!("invalid command in CSV record {record_number}"))?;
        
        // let events = engine
        //     .process(command)
        //     .with_context(|| format!("failed to process command {record_number}"))?;
        //
        // for event in events {
        //     write_event(&mut writer, &event)
        //         .context("failed to write output record")?;
        // }
    }

    writer.flush().context("failed to flush output")
}