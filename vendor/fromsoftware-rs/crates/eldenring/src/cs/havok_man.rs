use std::{ffi::c_void, ptr::NonNull};

use pelite::pe64::Pe;

use super::PlayerIns;
use crate::{
    cs::FieldInsBase,
    position::{HavokPosition, PositionDelta},
    rva,
};
use shared::{OwnedPtr, program::Program};

// Source of name: RTTI
#[shared::singleton("CSHavokMan")]
#[repr(C)]
pub struct CSHavokMan {
    vftable: usize,
    unk8: [u8; 0x90],
    pub phys_world: OwnedPtr<CSPhysWorld>,
    unka0: [u8; 0x20],
    pub collision_filter: *mut c_void,
}

// Source of name: RTTI
#[repr(C)]
pub struct CSPhysWorld {
    vftable: usize,
    pub hknp_world: OwnedPtr<hknpWorld>,
}

#[repr(C)]
pub struct CSPhysIns {
    vftable: usize,
    unk08: u16,
    pub owner: NonNull<FieldInsBase>,
}

// Source of name: RTTI
#[repr(C)]
pub struct hknpWorld {
    vftable: usize,
    unk08: [u8; 0x10],
    hknp_world: *mut hknpWorld,
    unk20: usize,
    pub hknp_body_array: *mut c_void,
    unk30: [u8; 0xad0],
    pub shape_tag_filter: *mut c_void,
}

type FnCastRay = extern "C" fn(
    *const CSPhysWorld,
    u32,
    *const HavokPosition,
    *const HavokPosition,
    *mut HavokPosition,
    *const PlayerIns,
) -> bool;

impl CSPhysWorld {
    /// Casts a ray inside of the physics world. Returns a None if the ray
    /// didn't hit anything.
    pub fn cast_ray(
        &self,
        filter: u32,
        origin: &HavokPosition,
        delta: PositionDelta,
        owner: &PlayerIns,
    ) -> Option<HavokPosition> {
        let target = unsafe {
            std::mem::transmute::<u64, FnCastRay>(
                Program::current()
                    .rva_to_va(rva::get().cs_phys_world_cast_ray)
                    .unwrap(),
            )
        };

        let mut result = HavokPosition(0.0, 0.0, 0.0, 0.0);
        let extent = HavokPosition(delta.0, delta.1, delta.2, 0.0);
        if target(self, filter, origin, &extent, &mut result, owner) {
            Some(result)
        } else {
            None
        }
    }
}
