use std::{mem::MaybeUninit, ptr::NonNull, slice};

use bitfield::bitfield;

use shared::*;

#[repr(C)]
#[derive(Superclass)]
#[superclass(children(WorldRes))]
/// Source of name: RTTI
pub struct WorldInfo {
    _vftable: usize,

    /// The number of defined entries in
    /// [world_area_info](Self::world_area_info).
    ///
    /// Use [Self::area_info] to access this safely.
    pub world_area_info_len: u32,

    /// A pointer to the beginning of [world_area_info](Self::world_area_info).
    ///
    /// Use [Self::area_info] to access this safely.
    pub world_area_info_list_ptr: NonNull<WorldAreaInfo>,

    /// The number of defined entries in
    /// [world_block_info](Self::world_block_info).
    ///
    /// Use [Self::block_info] to access this safely.
    pub world_block_info_len: u32,

    /// A pointer to the beginning of
    /// [world_block_info](Self::world_block_info).
    ///
    /// These are always an initial sublist of
    /// [world_block_info](Self::world_block_info).
    ///
    /// Use [Self::block_info] to access this safely.
    pub world_block_info_list_ptr: NonNull<WorldBlockInfo>,

    /// This name comes from debug data, but the behavior isn't yet well-understood.
    pub is_lock: bool,

    /// The pool of [WorldAreaInfo]s. Only the first
    /// [world_area_info_len](Self::world_area_info_len) are initialized.
    ///
    /// Use [Self::area_info] to access this safely.
    pub world_area_info: [MaybeUninit<WorldAreaInfo>; 0x14],

    /// The pool of [WorldBlockInfo]s. Only the first
    /// [world_block_info_len](Self::world_block_info_len) are initialized.
    ///
    /// Use [Self::block_info] to access this safely.
    pub world_block_info: [MaybeUninit<WorldBlockInfo>; 0x20],

    _unk1290: u64,
}

impl WorldInfo {
    /// The currently initialized area infos.
    pub fn area_info(&self) -> &[WorldAreaInfo] {
        unsafe {
            slice::from_raw_parts(
                self.world_area_info_list_ptr.as_ptr(),
                self.world_area_info_len as usize,
            )
        }
    }

    /// The mutable currently initialized area infos.
    pub fn area_info_mut(&mut self) -> &mut [WorldAreaInfo] {
        unsafe {
            slice::from_raw_parts_mut(
                self.world_area_info_list_ptr.as_mut(),
                self.world_area_info_len as usize,
            )
        }
    }

    /// The currently initialized block infos.
    pub fn block_info(&self) -> &[WorldBlockInfo] {
        unsafe {
            slice::from_raw_parts(
                self.world_block_info_list_ptr.as_ptr(),
                self.world_block_info_len as usize,
            )
        }
    }

    /// The mutable currently initialized block infos.
    pub fn block_info_mut(&mut self) -> &mut [WorldBlockInfo] {
        unsafe {
            slice::from_raw_parts_mut(
                self.world_block_info_list_ptr.as_mut(),
                self.world_block_info_len as usize,
            )
        }
    }

    /// The currently initialized area infos and their corresponding block
    /// infos.
    pub fn area_and_block_info(&self) -> impl Iterator<Item = (&WorldAreaInfo, &[WorldBlockInfo])> {
        self.area_info().iter().map(|area| {
            // Safety: We know there isn't a mutable reference to the block
            // info because it's owned by this WorldInfo to which we have an
            // immutable reference.
            (area, unsafe {
                slice::from_raw_parts(area.block_info.as_ptr(), area.block_info_length as usize)
            })
        })
    }

    /// The mutable currently initialized area infos and their corresponding
    /// block infos.
    pub fn area_and_block_info_mut(
        &mut self,
    ) -> impl Iterator<Item = (&mut WorldAreaInfo, &mut [WorldBlockInfo])> {
        self.area_info_mut().iter_mut().map(|area| {
            // Safety: We know there aren't any other references to the block
            // info because it's owned by this WorldInfo to which we have a
            // mutable reference.
            let blocks = unsafe {
                slice::from_raw_parts_mut(area.block_info.as_mut(), area.block_info_length as usize)
            };
            (area, blocks)
        })
    }
}

#[repr(C)]
/// Source of name: RTTI
pub struct WorldAreaInfo {
    _vftable: usize,
    _unk08: [u8; 3],

    /// The area's numeric identifier.
    ///
    /// This is corresponds to the `XX00000` digits in an event flag.
    pub area_number: u8,

    /// The [WorldInfo] instance that owns this area.
    pub owner: NonNull<WorldInfo>,

    /// The index of this area in [WorldInfo::world_area_info].
    pub world_area_index: u32,

    /// The index of this area's first block in [WorldInfo::world_block_info].
    pub world_block_index: u32,

    /// The length of the [block_info](Self::block_info) array.
    pub block_info_length: u32,

    /// The block infos for this [WorldAreaInfo].
    pub block_info: NonNull<WorldBlockInfo>,

    /// This name comes from debug data, but the behavior isn't yet well-understood.
    pub is_lock: bool,
}

bitfield! {
    /// An ID that contains information about the block's event locations.
    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    pub struct BlockId(u32);
    impl Debug;

    /// The event group that this block belongs to.
    pub u8, group, _: 23, 16;

    /// The area number that this block belongs to.
    pub u8, area, _: 31, 24;
}

#[repr(C)]
/// Source of name: RTTI
pub struct WorldBlockInfo {
    _vftable: usize,

    /// The block ID that indicates which event flags this block refers to.
    pub block_id: BlockId,

    /// The [WorldInfo] instance that owns this area.
    pub owner: NonNull<WorldInfo>,

    /// The [WorldAreaInfo] that contains this block.
    pub world_area_info: NonNull<WorldAreaInfo>,

    /// The index of this in [WorldInfo.world_block_info].
    ///
    /// This is also used as the index of this block's events in
    /// [EventWorld.blocks].
    pub world_block_index: u32,

    _unk24: u32,
    _msb_res_cap: usize,
    _btab_file_cap: usize,
    _btl_file_cap: usize,
    _btpb_file_cap: usize,
    _breakobj_file_cap: usize,
    _pre_map_decal_file_cap: usize,
    _unk58: usize,
    _unk60: u8,
    _pad61: [u8; 3],
    _unk64: u8,
    _unk68: u32,
}

#[repr(C)]
#[derive(Subclass)]
/// Source of name: RTTI
pub struct WorldRes {
    pub world_info: WorldInfo,
    _unk8: u64,

    /// The number of defined entries in [world_area_res](Self::world_area_res).
    ///
    /// Use [Self::area_res] to access this safely.
    pub world_area_res_len: u32,

    /// A pointer to the beginning of [world_area_res](Self::world_area_res).
    ///
    /// Use [Self::area_res] to access this safely.
    pub world_area_res_list_ptr: NonNull<WorldAreaRes>,

    _unk12b0: u32,
    _unk12b4: u32,

    /// The number of defined entries in
    /// [world_block_res](Self::world_block_res).
    ///
    /// Use [Self::block_res] to access this safely.
    pub world_block_res_len: u32,

    /// A pointer to the beginning of [world_block_res](Self::world_block_res).
    ///
    /// Use [Self::block_res] to access this safely.
    pub world_block_res_list_ptr: NonNull<WorldBlockRes>,

    _unk12c8: u64,
    _unk12d0: u64,
    _unk12d8: u64,

    /// The pool of [WorldAreaRes]es. Only the first
    /// [world_area_res_len](Self::world_area_res_len) are initialized.
    ///
    /// Use [Self::area_res] to access this safely.
    pub world_area_res: [MaybeUninit<WorldAreaRes>; 0x14],

    /// The pool of [WorldBlockRes]es. Only the first
    /// [world_block_res_len](Self::world_block_res_len) are initialized.
    ///
    /// Use [Self::block_res] to access this safely.
    pub world_block_res: [MaybeUninit<WorldBlockRes>; 0x20],

    _unkae80: u64,
    _unkae88: u64,
}

impl WorldRes {
    pub fn area_res(&self) -> &[WorldAreaRes] {
        unsafe {
            slice::from_raw_parts(
                self.world_area_res_list_ptr.as_ptr(),
                self.world_area_res_len as usize,
            )
        }
    }

    pub fn area_res_mut(&mut self) -> &mut [WorldAreaRes] {
        unsafe {
            slice::from_raw_parts_mut(
                self.world_area_res_list_ptr.as_mut(),
                self.world_area_res_len as usize,
            )
        }
    }

    pub fn block_res(&self) -> &[WorldBlockRes] {
        unsafe {
            slice::from_raw_parts(
                self.world_block_res_list_ptr.as_ptr(),
                self.world_block_res_len as usize,
            )
        }
    }

    pub fn block_res_mut(&mut self) -> &mut [WorldBlockRes] {
        unsafe {
            slice::from_raw_parts_mut(
                self.world_block_res_list_ptr.as_mut(),
                self.world_block_res_len as usize,
            )
        }
    }
}

// WorldInfoOwner doesn't add any additional fields.
pub type WorldInfoOwner = WorldRes;

// Source of name: RTTI
pub type WorldAreaRes = UnknownStruct<0x108>;

// Source of name: RTTI
pub type WorldBlockRes = UnknownStruct<0x438>;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn proper_sizes() {
        assert_eq!(0x38, size_of::<WorldAreaInfo>());
        assert_eq!(0x70, size_of::<WorldBlockInfo>());
        assert_eq!(0x1298, size_of::<WorldInfo>());
        assert_eq!(0xae90, size_of::<WorldRes>());
    }
}
