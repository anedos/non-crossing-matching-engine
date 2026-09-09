use crate::model::command::{CancelOrder, Command, NewOrder};
use crate::model::order::{Price, Quantity, Side, Symbol, UserId, UserOrderId};
use anyhow::{anyhow, bail, ensure, Result};
use csv::StringRecord;
use serde::de::IgnoredAny;

pub fn parse_command(record: &StringRecord) -> Result<Command> {
    let command = record.get(0).ok_or_else(|| anyhow!("Empty command"))?;

    match command {
        "N" => {
            ensure!(
                record.len() == 7,
                "new order expects 7 fields, got {}",
                record.len()
            );

            let (_, user_id, symbol, price, quantity, side, user_order_id): (
                IgnoredAny,
                UserId,
                Symbol,
                Price,
                Quantity,
                Side,
                UserOrderId,
            ) = record.deserialize(None)?;
            Ok(Command::New(NewOrder {
                user_id,
                symbol,
                price,
                quantity,
                side,
                user_order_id,
            }))
        }
        "C" => {
            ensure!(
                record.len() == 3,
                "cancel order expects 3 fields, got {}",
                record.len()
            );

            let (_, user_id, user_order_id): (IgnoredAny, UserId, UserOrderId) =
                record.deserialize(None)?;
            Ok(Command::Cancel(CancelOrder {
                user_id,
                user_order_id,
            }))
        }
        "F" => {
            ensure!(
                record.len() == 1,
                "flush expects 1 field, got {}",
                record.len()
            );

            Ok(Command::Flush)
        }
        other => bail!("invalid command: {:?}", other),
    }
}
