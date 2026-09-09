//! A small Serde format for flat records of strings and unsigned integers.
//! Fields are written verbatim: this is not a general CSV encoder.

use std::fmt;
use std::io::{self, Write};

use serde::ser::{self, Impossible, Serialize, SerializeTuple};

#[derive(Debug)]
enum Error {
    Io(io::Error),
    Unsupported(&'static str),
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Unsupported(kind) => write!(f, "unsupported record type: {kind}"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Message(message.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub(super) fn write_record<T: Serialize + ?Sized>(
    writer: &mut impl Write,
    value: &T,
    separator: &str,
) -> anyhow::Result<()> {
    value.serialize(&mut Serializer {
        writer,
        separator,
        first: true,
        in_tuple: false,
    })?;
    writer.write_all(b"\n")?;
    Ok(())
}

struct Serializer<'a, W> {
    writer: &'a mut W,
    separator: &'a str,
    first: bool,
    in_tuple: bool,
}

impl<W: Write> Serializer<'_, W> {
    fn field(&mut self, value: impl fmt::Display) -> Result<(), Error> {
        if !self.first {
            self.writer.write_all(self.separator.as_bytes())?;
        }
        self.first = false;
        write!(self.writer, "{value}")?;
        Ok(())
    }
}

macro_rules! unsigned {
    ($($method:ident: $ty:ty),* $(,)?) => {
        $(fn $method(self, value: $ty) -> Result<(), Error> {
            self.field(value)
        })*
    };
}

macro_rules! unsupported_scalar {
    ($($method:ident: $ty:ty),* $(,)?) => {
        $(fn $method(self, _value: $ty) -> Result<(), Error> {
            Err(Error::Unsupported(stringify!($ty)))
        })*
    };
}

impl<W: Write> ser::Serializer for &mut Serializer<'_, W> {
    type Ok = ();
    type Error = Error;
    type SerializeTuple = Self;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    unsigned! {
        serialize_u8: u8, serialize_u16: u16, serialize_u32: u32,
        serialize_u64: u64, serialize_u128: u128,
    }

    unsupported_scalar! {
        serialize_bool: bool, serialize_i8: i8, serialize_i16: i16,
        serialize_i32: i32, serialize_i64: i64, serialize_i128: i128,
        serialize_f32: f32, serialize_f64: f64, serialize_char: char,
        serialize_bytes: &[u8],
    }

    fn serialize_str(self, value: &str) -> Result<(), Error> {
        self.field(value)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        if self.in_tuple {
            return Err(Error::Unsupported("nested tuple"));
        }
        self.in_tuple = true;
        Ok(self)
    }

    fn serialize_none(self) -> Result<(), Error> {
        Err(Error::Unsupported("option"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<(), Error> {
        Err(Error::Unsupported("option"))
    }

    fn serialize_unit(self) -> Result<(), Error> {
        Err(Error::Unsupported("unit"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        Err(Error::Unsupported("unit struct"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<(), Error> {
        Err(Error::Unsupported("unit variant"))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<(), Error> {
        Err(Error::Unsupported("newtype struct"))
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<(), Error> {
        Err(Error::Unsupported("newtype variant"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(Error::Unsupported("sequence"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(Error::Unsupported("tuple struct"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(Error::Unsupported("tuple variant"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(Error::Unsupported("map"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(Error::Unsupported("struct"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(Error::Unsupported("struct variant"))
    }
}

impl<W: Write> SerializeTuple for &mut Serializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        self.in_tuple = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_support_string_separators_and_unsigned_extremes() {
        crate::test_logging::init_test_logging();
        for separator in [", ", "|", "\t"] {
            let mut output = Vec::new();
            write_record(&mut output, &("A", 0_u64, u64::MAX), separator).unwrap();
            write_record(&mut output, &("B", "S", "-", "-"), separator).unwrap();
            assert_eq!(
                String::from_utf8(output).unwrap(),
                format!(
                    "A{separator}0{separator}{}\nB{separator}S{separator}-{separator}-\n",
                    u64::MAX,
                )
            );
        }
    }

    #[test]
    fn unsupported_values_return_errors() {
        crate::test_logging::init_test_logging();
        fn check(value: impl Serialize, kind: &str) {
            let error = write_record(&mut Vec::new(), &value, ", ").unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("unsupported record type: {kind}")
            );
        }
        check(true, "bool");
        check(-1_i64, "i64");
        check(Some(1_u64), "option");
        check(vec![1_u64], "sequence");
        check(((1_u64, 2_u64),), "nested tuple");
    }
}
