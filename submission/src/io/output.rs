use super::serializer::write_record;
use crate::model::event::Event;
use crate::model::order::Side;
use anyhow::Result;
use std::io::Write;

/// Write one event with a verbatim field separator and a trailing newline.
pub fn write_event(writer: &mut impl Write, event: &Event, separator: &str) -> Result<()> {
    match event {
        Event::Accepted {
            user_id,
            user_order_id,
        } => write_record(writer, &("A", user_id.0, user_order_id.0), separator),
        Event::Rejected {
            user_id,
            user_order_id,
        } => write_record(writer, &("R", user_id.0, user_order_id.0), separator),
        Event::TopChanged { side, top } => {
            let side = match side {
                Side::Bid => "B",
                Side::Ask => "S",
            };
            match top {
                Some(top) => write_record(
                    writer,
                    &("B", side, top.price(), top.total_quantity()),
                    separator,
                ),
                None => write_record(writer, &("B", side, "-", "-"), separator),
            }
        }
    }
}
