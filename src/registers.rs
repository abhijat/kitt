use crate::process::Process;
use crate::reginfo::{lookup_register_info_by_id, RegisterFormat, RegisterId, RegisterInfo};
use crate::types::{Byte128, Byte64};
use anyhow::{bail, Result};
use nix::libc::user;
use std::slice;

struct Register<'a> {
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
    LD(f64),
    B64(Byte64),
    B128(Byte128),
}

fn value_from<T>(slice: &[u8]) -> T
where
    T: Copy,
{
    let size = size_of::<T>();
    let slice = &slice[..size];
    let p = slice.as_ptr();
    let p = p as *const T;
    unsafe { *p }
}

impl<'a> Register<'a> {
    fn user_as_slice(&self, offset: usize) -> &[u8] {
        let p = &self.data as *const user;
        let p = p as *const u8;
        unsafe {
            let p = p.add(offset);
            slice::from_raw_parts(p, size_of::<user>())
        }
    }

    fn read(&self, info: &RegisterInfo) -> Result<Value> {
        let slice = self.user_as_slice(info.offset);
        let v = match info.format {
            RegisterFormat::Uint => match info.size {
                1 => Value::U8(value_from(slice)),
                2 => Value::U16(value_from(slice)),
                4 => Value::U32(value_from(slice)),
                8 => Value::U64(value_from(slice)),
                size => bail!("unexpected size of register: {size}"),
            },
            RegisterFormat::DoubleFloat => Value::F(value_from(slice)),
            RegisterFormat::LongDouble => Value::LD(value_from(slice)),
            RegisterFormat::Vector if info.size == 8 => {
                let v = (&slice[..8]).try_into()?;
                Value::B64(v)
            }
            RegisterFormat::Vector => {
                let v = (&slice[..16]).try_into()?;
                Value::B128(v)
            }
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
