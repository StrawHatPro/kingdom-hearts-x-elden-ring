use std::ptr::NonNull;

use shared::{F32ModelMatrix, F32Vector4};

use crate::cs::FieldInsHandle;

#[repr(C)]
#[shared::singleton("LockTgtMan")]
pub struct LockTgtMan {
    vftable: usize,
    unk08: [u8; 0x8],
    pub nodes: Option<NonNull<LockTgtNode>>,
    unk18: [u8; 0x2818],
    pub is_locked_on: bool,
    pub is_lock_on_requested: bool,
    unk2832: [u8; 0x15b],
    pub lock_camera: bool,
}

#[repr(C)]
pub struct LockTgtNode {
    unk00: F32Vector4,
    unk10: F32ModelMatrix,
    unk50: F32Vector4,
    unk60: f32,
    unk64: f32,
    unk68: f32,
    unk6c: f32,
    unk70: f32,
    unk74: f32,
    unk78: f32,
    pub next: Option<NonNull<LockTgtNode>>,
    pub value: NonNull<LockTgtNodeValue>,
    pub flags: u8,
}

#[repr(C)]
pub struct LockTgtNodeValue {
    unk00: F32Vector4,
    unk10: F32Vector4,
    unk20: F32Vector4,
    unk30: F32Vector4,
    unk40: F32Vector4,
    unk50: F32Vector4,
    unk60: F32Vector4,
    unk70: f32,
    unk74: u8,
    unk75: u8,
    unk76: u16,
    pub chr_handle: FieldInsHandle,
    unk80: *mut LockTgtNodeValue,
}
