use crate::model::command::{CancelOrder, Command, NewOrder};
use anyhow::{Result, ensure, anyhow, bail};
use csv::StringRecord;

pub fn parse_command(record: &StringRecord) -> Result<Command> {
    ensure!(
        matches!(record.len(), 1 | 3 | 7),
        "invalid field count: expected 1, 3, or 7, got {}",
        record.len()
    );

    match record.get(0) {
        Some("N") => {
            let record: StringRecord = record.iter().skip(1).collect();
            let new_order: NewOrder = record.deserialize(None)?;
            Ok(Command::New(new_order))
        }
        Some("C") => {
            let record: StringRecord = record.iter().skip(1).collect();
            let cancel_order: CancelOrder = record.deserialize(None)?;
            Ok(Command::Cancel(cancel_order))
        }
        Some("F") => Ok(Command::Flush),
        other => bail!("invalid command: {:?}", other),
    }
}