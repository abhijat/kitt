use crate::process::Process;
use crate::reginfo::{lookup_register_info_by_id, RegisterFormat, RegisterId, RegisterInfo};
use crate::types::{Byte128, Byte64};
use anyhow::{bail, Result};
use nix::libc::user;
use std::slice;

struct Registers<'a> {
    data: user,
    process: &'a Process,
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

fn read_value<T>(data: &[u8]) -> T
where
    T: Copy,
{
    unsafe { *(data.as_ptr() as *const T) }
}

impl<'a> Registers<'a> {
    fn read_value<T>(&self, offset: usize) -> T
    where
        T: Copy,
    {
        let p = &self.data as *const user;
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

    fn write(&self, register_info: &RegisterInfo, value: Value) -> Result<()> {
        unimplemented!()
    }

    fn write_by_id(&self, register_id: RegisterId, value: Value) -> Result<()> {
        let reg_info = lookup_register_info_by_id(register_id)?;
        self.write(reg_info, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_slice<T>(t: &T) -> &[u8] {
        unsafe { slice::from_raw_parts(t as *const T as *const u8, size_of::<T>()) }
    }

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
