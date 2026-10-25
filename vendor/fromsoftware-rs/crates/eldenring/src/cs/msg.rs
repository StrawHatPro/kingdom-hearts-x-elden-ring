use std::{
    marker::PhantomData,
    mem::{self, MaybeUninit},
    num::NonZero,
    ops::{Deref, DerefMut, Index, IndexMut},
    ptr::NonNull,
    slice,
};

use windows::core::PCWSTR;

use crate::dlkr::DLAllocator;

#[repr(C)]
#[shared::singleton("MsgRepository")]
pub struct MsgRepository {
    vftable: usize,
    files: NonNull<Option<NonNull<Option<NonNull<FmgFileHeader>>>>>,
    pub version_count: u32,
    pub file_capacity: u32,
    _unk18: u32,
    _unk20: usize,
    _unk28: usize,
    pub allocator: &'static DLAllocator,
    _unk38: u32,
    _unk3c: u32,
    _unk40: u32,
    _unk44: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct FmgFileHeader {
    _unk00: u8,
    pub endianness: u8,
    pub version: u16,
    pub file_size: u32,
    _unk08: u32,
    pub group_count: u32,
    pub msg_count: u32,
    pub max_group_size: u32,
    pub msg_offsets: *mut Option<NonZero<i64>>,
    _unk20: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MsgGroup {
    pub offset: u32,
    pub first_id: u32,
    pub last_id: u32,
    _unk0c: u32,
}

pub struct FmgFile<'a> {
    ptr: NonNull<[u8]>,
    _marker: PhantomData<&'a ()>,
}

impl MsgRepository {
    pub fn get_msg(&self, category: u32, id: u32) -> Option<&[u16]> {
        let file = self.get_file(category)?;
        file.get_msg(id)
    }

    pub fn get_msg_mut(&mut self, category: u32, id: u32) -> Option<&mut [u16]> {
        let mut file = self.get_file(category)?;
        file.get_msg_mut(id)
    }

    pub fn get_msg_disjoint_mut<const N: usize>(
        &mut self,
        mut query: [(u32, u32); N],
    ) -> Option<[&mut [u16]; N]> {
        query.sort();
        if query.windows(2).any(|w| w[0] == w[1]) {
            return None;
        }

        let mut result = [const { MaybeUninit::uninit() }; N];
        for (result, (category, id)) in result.iter_mut().zip(query) {
            let mut file = self.get_file(category)?;
            *result = MaybeUninit::new(file.get_msg_mut(id)?);
        }

        unsafe {
            Some(mem::transmute_copy::<
                [MaybeUninit<&mut [u16]>; N],
                [&mut [u16]; N],
            >(&result))
        }
    }

    pub fn get_file(&self, category: u32) -> Option<FmgFile<'_>> {
        let files = unsafe { self.files_by_version(0)?.as_ref() };
        let header = *files.get(category as usize)?;
        unsafe { Some(FmgFile::from_header(header?)) }
    }

    fn files_by_version(&self, version: u32) -> Option<NonNull<[Option<NonNull<FmgFileHeader>>]>> {
        let files = (version < self.version_count)
            .then(|| unsafe { self.files.add(version as usize).read() })??;

        Some(NonNull::slice_from_raw_parts(
            files,
            self.file_capacity as usize,
        ))
    }
}

impl<'a> FmgFile<'a> {
    pub fn header(&self) -> &FmgFileHeader {
        unsafe { self.header_ptr().as_ref() }
    }

    pub fn header_mut(&mut self) -> &mut FmgFileHeader {
        unsafe { self.header_ptr().as_mut() }
    }

    pub fn msg_groups(&self) -> &[MsgGroup] {
        let size = size_of::<MsgGroup>() * self.group_count as usize;
        unsafe { self.ptr.as_ref()[..size].align_to().1 }
    }

    pub fn msg_offsets(&self) -> &[Option<NonZero<i64>>] {
        if self.msg_offsets.is_aligned() && !self.msg_offsets.is_null() {
            unsafe { slice::from_raw_parts(self.msg_offsets, self.msg_count as usize) }
        } else {
            &[]
        }
    }

    pub fn get_msg(&self, id: u32) -> Option<&'a [u16]> {
        let index = self.msg_index_by_id(id)?;
        self.msg_by_index(index)
    }

    pub fn get_msg_mut(&mut self, id: u32) -> Option<&'a mut [u16]> {
        let index = self.msg_index_by_id(id)?;
        self.msg_by_index_mut(index)
    }

    pub fn msg_index_by_id(&self, id: u32) -> Option<u32> {
        let groups = self.msg_groups();

        let mut left = 0;
        let mut right = self.group_count.checked_sub(1)? as usize;

        if id < groups.get(left)?.first_id || id > groups.get(right)?.last_id {
            return None;
        }

        while left <= right {
            let mid = (left + right) / 2;
            let group = &groups[mid];

            if group.last_id < id {
                left = mid + 1;
            } else {
                if group.first_id <= id {
                    return Some(id - group.first_id + group.offset);
                }

                right = mid.checked_sub(1)?;
            }
        }

        None
    }

    pub fn msg_by_index(&self, index: u32) -> Option<&'a [u16]> {
        unsafe {
            let msg_ptr = self.msg_by_index_ptr(index)?.as_ptr();
            let len = PCWSTR::from_raw(msg_ptr).as_wide().len();
            Some(slice::from_raw_parts(msg_ptr, len))
        }
    }

    pub fn msg_by_index_mut(&mut self, index: u32) -> Option<&'a mut [u16]> {
        unsafe {
            let msg_ptr = self.msg_by_index_ptr(index)?.as_ptr();
            let len = PCWSTR::from_raw(msg_ptr).as_wide().len();
            Some(slice::from_raw_parts_mut(msg_ptr, len))
        }
    }

    unsafe fn from_header(header: NonNull<FmgFileHeader>) -> Self {
        assert!(header.is_aligned());

        let size = unsafe { header.as_ref().file_size as usize };
        let header_size = size_of::<Self>();
        let contents = unsafe { header.add(1).cast() };

        Self {
            ptr: NonNull::slice_from_raw_parts(contents, size.saturating_sub(header_size)),
            _marker: PhantomData,
        }
    }

    fn header_ptr(&self) -> NonNull<FmgFileHeader> {
        unsafe { self.ptr.cast().sub(1) }
    }

    fn msg_by_index_ptr(&self, index: u32) -> Option<NonNull<u16>> {
        let offsets = self.msg_offsets();
        let offset = (*offsets.get(index as usize)?)?.get() as isize;
        unsafe { Some(self.header_ptr().byte_offset(offset).cast::<u16>()) }
    }
}

impl Index<u32> for FmgFile<'_> {
    type Output = [u16];

    fn index(&self, id: u32) -> &Self::Output {
        self.get_msg(id).expect("index out of bounds")
    }
}

impl IndexMut<u32> for FmgFile<'_> {
    fn index_mut(&mut self, id: u32) -> &mut Self::Output {
        self.get_msg_mut(id).expect("index out of bounds")
    }
}

impl Deref for FmgFile<'_> {
    type Target = FmgFileHeader;

    fn deref(&self) -> &Self::Target {
        self.header()
    }
}

impl DerefMut for FmgFile<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.header_mut()
    }
}
