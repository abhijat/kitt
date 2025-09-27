use bytemuck::{bytes_of, from_bytes};

pub type Byte64 = [u8; 8];
pub type Byte128 = [u8; 16];

pub(crate) enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F(f64),
    LD(f64), // There is no f128 yet
    B64(Byte64),
    B128(Byte128),
}

impl Value {
    pub fn widen(&self) -> Byte128 {
        match self {
            Value::I8(v) => *from_bytes(bytes_of(v)),
            Value::I16(v) => *from_bytes(bytes_of(v)),
            Value::I32(v) => *from_bytes(bytes_of(v)),
            Value::I64(v) => *from_bytes(bytes_of(v)),
            Value::F(v) => *from_bytes(bytes_of(v)),
            Value::LD(v) => *from_bytes(bytes_of(v)),
            Value::U8(v) => *from_bytes(bytes_of(v)),
            Value::U16(v) => *from_bytes(bytes_of(v)),
            Value::U32(v) => *from_bytes(bytes_of(v)),
            Value::U64(v) => *from_bytes(bytes_of(v)),
            Value::B64(v) => *from_bytes(bytes_of(v)),
            Value::B128(v) => *from_bytes(bytes_of(v)),
        }
    }
}
