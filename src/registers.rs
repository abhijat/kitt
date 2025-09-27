use crate::process::Process;
use crate::reginfo::{
    lookup_register_info_by_id, RegisterFormat, RegisterId, RegisterInfo, RegisterKind,
};
use crate::registers::values::Value;
use anyhow::{bail, Result};
use bytemuck::{
    bytes_of, bytes_of_mut, from_bytes, AnyBitPattern, Pod, TransparentWrapper, Zeroable,
};
use nix::libc::user;
use std::mem;

mod values;

#[derive(Copy, Clone)]
#[repr(transparent)]
struct User(pub user);

unsafe impl TransparentWrapper<user> for User {}
unsafe impl Zeroable for User {}
unsafe impl Pod for User {}

pub(crate) struct Registers {
    data: User,
}

impl Default for Registers {
    fn default() -> Self {
        unsafe {
            Self {
                data: User::wrap(mem::zeroed()),
            }
        }
    }
}

impl Registers {
    pub fn user_data(&self) -> user {
        self.data.0
    }

    pub fn set_user_data(&mut self, u: user) {
        self.data = User::wrap(u);
    }

    fn read_value<T>(&self, offset: usize) -> T
    where
        T: AnyBitPattern,
    {
        let slice = bytes_of(&self.data);
        *from_bytes(&slice[offset..])
    }

    fn read(&self, info: &RegisterInfo) -> Result<Value> {
        use Value::*;
        let v = match info.format {
            RegisterFormat::Uint => match info.size {
                1 => U8(self.read_value(info.offset)),
                2 => U16(self.read_value(info.offset)),
                4 => U32(self.read_value(info.offset)),
                8 => U64(self.read_value(info.offset)),
                size => bail!("unexpected size of register: {size}"),
            },
            RegisterFormat::DoubleFloat => F(self.read_value(info.offset)),
            RegisterFormat::LongDouble => LD(self.read_value(info.offset)),
            RegisterFormat::Vector if info.size == 8 => B64(self.read_value(info.offset)),
            RegisterFormat::Vector => B128(self.read_value(info.offset)),
        };
        Ok(v)
    }

    fn read_by_id(&self, register_id: RegisterId) -> Result<Value> {
        self.read(lookup_register_info_by_id(register_id)?)
    }

    fn write(
        &mut self,
        register_info: &RegisterInfo,
        value: Value,
        process: &Process,
    ) -> Result<()> {
        let user_bytes = bytes_of_mut(&mut self.data);
        let widened = value.widen();
        let value_bytes = bytes_of(&widened);
        let start = register_info.offset;
        let end = start + value_bytes.len();
        let user_bytes_section = &mut user_bytes[start..end];
        user_bytes_section.copy_from_slice(value_bytes);

        if register_info.kind == RegisterKind::FloatingPoint {
            process.write_fprs(self.user_data().i387)?;
            Ok(())
        } else {
            // make sure the address is aligned to 8 bytes
            let aligned_address = register_info.offset & !0b111;

            // read 8 bytes starting from aligned address into word
            let word = *from_bytes(&user_bytes[aligned_address..]);

            // write into process user data. the assumption is that the value size is <= 8 bytes
            process.write_user_area(aligned_address, word)?;
            Ok(())
        }
    }

    fn write_by_id(
        &mut self,
        register_id: RegisterId,
        value: Value,
        process: &Process,
    ) -> Result<()> {
        let reg_info = lookup_register_info_by_id(register_id)?;
        self.write(reg_info, value, process)
    }
}
