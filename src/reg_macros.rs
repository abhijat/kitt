#[macro_export]
macro_rules! gpr64 {
    ($register_id:ident, $register_field:expr, $dwarf_id:literal) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: $dwarf_id,
            size: 8,
            offset: offset_of!(user, regs) + offset_of!(user_regs_struct, $register_field),
            kind: RegisterKind::GeneralPurpose,
            format: RegisterFormat::Uint,
        }
    };
}

#[macro_export]
macro_rules! gpr32 {
    ($register_id:ident,$parent_field:expr) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: -1,
            size: 4,
            offset: offset_of!(user, regs) + offset_of!(user_regs_struct, $parent_field),
            kind: RegisterKind::SubGeneralPurpose,
            format: RegisterFormat::Uint,
        }
    };
}

#[macro_export]
macro_rules! gpr16 {
    ($register_id:ident,$parent_field:expr) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: -1,
            size: 2,
            offset: offset_of!(user, regs) + offset_of!(user_regs_struct, $parent_field),
            kind: RegisterKind::SubGeneralPurpose,
            format: RegisterFormat::Uint,
        }
    };
}

#[macro_export]
macro_rules! gpr8_hi {
    ($register_id:ident,$parent_field:expr) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: -1,
            size: 1,
            offset: offset_of!(user, regs) + offset_of!(user_regs_struct, $parent_field) + 1,
            kind: RegisterKind::SubGeneralPurpose,
            format: RegisterFormat::Uint,
        }
    };
}

#[macro_export]
macro_rules! gpr8_lo {
    ($register_id:ident,$parent_field:expr) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: -1,
            size: 1,
            offset: offset_of!(user, regs) + offset_of!(user_regs_struct, $parent_field),
            kind: RegisterKind::SubGeneralPurpose,
            format: RegisterFormat::Uint,
        }
    };
}

#[macro_export]
macro_rules! fpr {
    ($register_id:ident, $dwarf_id:literal, $user_name:expr, $size:literal) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: $dwarf_id,
            size: $size,
            offset: offset_of!(user, i387) + offset_of!(user_fpregs_struct, $user_name),
            kind: RegisterKind::FloatingPoint,
            format: RegisterFormat::Uint,
        }
    };
}

#[macro_export]
macro_rules! fp_st {
    ($register_id:ident, $seq:literal) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: 33 + $seq,
            size: 16,
            offset: offset_of!(user, i387) + offset_of!(user_fpregs_struct, st_space) + ($seq * 16),
            kind: RegisterKind::FloatingPoint,
            format: RegisterFormat::LongDouble,
        }
    };
}

#[macro_export]
macro_rules! fp_mm {
    ($register_id:ident, $seq:literal) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: 41 + $seq,
            size: 8,
            offset: offset_of!(user, i387) + offset_of!(user_fpregs_struct, st_space) + ($seq * 16),
            kind: RegisterKind::FloatingPoint,
            format: RegisterFormat::Vector,
        }
    };
}

#[macro_export]
macro_rules! xmm {
    ($register_id:ident, $seq:literal) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: 17 + $seq,
            size: 16,
            offset: offset_of!(user, i387)
                + offset_of!(user_fpregs_struct, xmm_space)
                + ($seq * 16),
            kind: RegisterKind::FloatingPoint,
            format: RegisterFormat::Vector,
        }
    };
}

#[macro_export]
macro_rules! debugreg {
    ($register_id:ident, $seq:literal) => {
        RegisterInfo {
            id: RegisterId::$register_id,
            name: stringify!($register_id).to_string(),
            dwarf_id: -1,
            size: 8,
            offset: offset_of!(user, u_debugreg) + ($seq * 8),
            kind: RegisterKind::Debug,
            format: RegisterFormat::Uint,
        }
    };
}
