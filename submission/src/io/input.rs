use crate::model::command::{CancelOrder, Command, NewOrder};
use anyhow::{Result, ensure, anyhow, bail};
use csv::StringRecord;

pub fn parse_command(record: &StringRecord) -> Result<Command> {
    let command = record.get(0).ok_or_else(|| anyhow!("Empty command"))?;

    match command {
        "N" => {
            ensure!(
                record.len() == 7,
                "new order expects 7 fields, got {}",
                record.len()
            );

            let fields: StringRecord = record.iter().skip(1).collect();
            Ok(Command::New(fields.deserialize(None)?))
        }
        "C" => {
            ensure!(
                record.len() == 3,
                "cancel order expects 3 fields, got {}",
                record.len()
            );

            let fields: StringRecord = record.iter().skip(1).collect();
            Ok(Command::Cancel(fields.deserialize(None)?))
        }
        "F" => {
            ensure!(
                record.len() == 1,
                "flush expects 1 field, got {}",
                record.len()
            );

            Ok(Command::Flush)
        },
        other => bail!("invalid command: {:?}", other),
    }
}