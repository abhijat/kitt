use crate::process::Process;
use crate::reginfo::{lookup_register_info_by_id, RegisterFormat, RegisterId, RegisterInfo};
use crate::types::{Byte128, Byte64};
use anyhow::{bail, Result};
use nix::libc::user;
use std::slice;

struct Registers {
    data: Option<user>,
}

enum Value {
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

fn to_slice<T>(t: &T) -> &[u8] {
    unsafe { slice::from_raw_parts(t as *const T as *const u8, size_of::<T>()) }
}

impl Value {
    pub fn to_bytes(&self) -> &[u8] {
        match self {
            Value::U8(v) => to_slice(v),
            Value::U16(v) => to_slice(v),
            Value::U32(v) => to_slice(v),
            Value::U64(v) => to_slice(v),
            Value::I8(v) => to_slice(v),
            Value::I16(v) => to_slice(v),
            Value::I32(v) => to_slice(v),
            Value::I64(v) => to_slice(v),
            Value::F(v) => to_slice(v),
            Value::LD(v) => to_slice(v),
            Value::B64(v) => to_slice(v),
            Value::B128(v) => to_slice(v),
        }
    }
}

fn read_value<T>(data: &[u8]) -> T
where
    T: Copy,
{
    unsafe { *(data.as_ptr() as *const T) }
}

impl Registers {
    pub fn new() -> Self {
        Self { data: None }
    }

    fn read_value<T>(&self, offset: usize) -> T
    where
        T: Copy,
    {
        let p = &self.data.unwrap() as *const user;
        let p = p as *const u8;
        unsafe { read_value(slice::from_raw_parts(p.add(offset), size_of::<T>())) }
    }

    fn read(&self, info: &RegisterInfo) -> Result<Value> {
        let v = match info.format {
            RegisterFormat::Uint => match info.size {
                1 => Value::U8(self.read_value(info.offset)),
                2 => Value::U16(self.read_value(info.offset)),
                4 => Value::U32(self.read_value(info.offset)),
                8 => Value::U64(self.read_value(info.offset)),
                size => bail!("unexpected size of register: {size}"),
            },
            RegisterFormat::DoubleFloat => Value::F(self.read_value(info.offset)),
            RegisterFormat::LongDouble => Value::LD(self.read_value(info.offset)),
            RegisterFormat::Vector if info.size == 8 => Value::B64(self.read_value(info.offset)),
            RegisterFormat::Vector => Value::B128(self.read_value(info.offset)),
        };
        Ok(v)
    }

    fn read_by_id(&self, register_id: RegisterId) -> Result<Value> {
        let reg_info = lookup_register_info_by_id(register_id)?;
        self.read(reg_info)
    }

    fn write(&self, register_info: &RegisterInfo, value: Value, process: &Process) -> Result<()> {
        let raw = unsafe {
            slice::from_raw_parts_mut(
                &self.data.unwrap() as *const user as *mut u8,
                size_of::<user>(),
            )
        };

        let value_bytes = value.to_bytes();
        let start = register_info.offset;
        let end = start + value_bytes.len();
        let raw = &mut raw[start..end];
        raw.copy_from_slice(value_bytes);

        let payload = u64::from_le_bytes(raw.try_into()?);
        process.write_user_area(register_info.offset, payload)?;
        Ok(())
    }

    fn write_by_id(&self, register_id: RegisterId, value: Value, process: &Process) -> Result<()> {
        let reg_info = lookup_register_info_by_id(register_id)?;
        self.write(reg_info, value, process)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_single_values() {
        {
            let source: u64 = 421;
            let v: u64 = read_value(to_slice(&source));
            assert_eq!(v, 421);
        }

        {
            let source: i32 = -291;
            let v: i32 = read_value(to_slice(&source));
            assert_eq!(v, -291);
        }
    }

    #[test]
    fn read_array_value() {
        let input = [0, 1, 2, 3, 4];
        let output: [i32; 5] = read_value(to_slice(&input));
        assert_eq!(input, output);
    }
}
