use shared::OwnedPtr;

#[repr(C)]
#[shared::singleton("CSRemo")]
pub struct CSRemo {
    vtable: usize,
    pub remo_man: Option<OwnedPtr<CSRemoMan>>,
}

#[repr(C)]
pub struct CSRemoMan {
    unk00: [u8; 0x40],
    pub state: i32,
}
