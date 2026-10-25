use std::ptr::NonNull;

use shared::Subclass;

use crate::fd4::{FD4FileCap, FD4ResCap};

#[repr(C)]
#[shared::singleton("TpfRepository")]
pub struct TpfRepository {}

#[repr(C)]
#[derive(Subclass)]
pub struct TpfFileCap {
    pub file_cap: FD4FileCap,
    pub tex_res_cap: Option<NonNull<TpfResCap>>,
}

#[repr(C)]
#[derive(Subclass)]
pub struct TpfResCap {
    pub res_cap: FD4ResCap,
    unk78: [u8; 0x40],
}

#[repr(C)]
#[shared::singleton("TexRepository")]
pub struct TexRepository {}

#[repr(C)]
#[shared::singleton("ScaleformTexRepository")]
pub struct ScaleformTexRepository {}
