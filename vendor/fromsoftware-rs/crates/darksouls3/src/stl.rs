use std::ffi::c_void;

use crate::dlkr::DLAllocator;

impl fromsoftware_shared_stl::StlAllocator for &'static DLAllocator {
    unsafe fn allocate_raw(&self, size: usize, align: usize) -> *mut c_void {
        let allocation = (self.vftable.allocate_aligned)(self, size, align);
        if allocation.is_null() {
            panic!("DLAllocator returned null pointer")
        }
        allocation as _
    }

    unsafe fn deallocate_raw(&self, ptr: *mut c_void) {
        (self.vftable.deallocate)(self, ptr as _);
    }
}

/// An MSVC 2012-compatible vector that contains a custom DS3 allocator. This is
/// the type of vector generally used in DS3.
pub type DLVector<T> = fromsoftware_shared_stl::Vector<T, &'static DLAllocator>;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn proper_sizes() {
        assert_eq!(0x20, size_of::<DLVector<usize>>());
    }
}
