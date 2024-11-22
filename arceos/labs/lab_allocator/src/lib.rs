//! Allocator algorithm in lab.

#![no_std]
#![allow(unused_variables)]

extern crate alloc;

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;
use rlsf::Tlsf;

unsafe impl Sync for LabByteAllocator {}
unsafe impl Send for LabByteAllocator {}

#[derive(Copy, Clone)]
pub struct UnfreedBlock {
    ptr: Option<NonNull<u8>>,
    size: usize,
    align: usize,
}

pub struct LabByteAllocator {
    inner: Tlsf<'static, u32, u32, 28, 32>,
    total_bytes: usize,
    used_bytes: usize,

    offset: usize,
    marker_size: usize,
    unfreed_sizes: [usize; 7],
    unfreed_blocks: [UnfreedBlock; 7],
}

impl LabByteAllocator {
    pub const fn new() -> Self {
        Self {
            inner: Tlsf::new(),
            total_bytes: 0,
            used_bytes: 0,

            // Cowabunga!
            offset: 2,
            marker_size: 32,
            unfreed_sizes: [64, 256, 1024, 4096, 16384, 65536, 262144],
            unfreed_blocks: [UnfreedBlock {
                ptr: None,
                size: 0,
                align: 0,
            }; 7],
        }
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        unsafe {
            let pool = core::slice::from_raw_parts_mut(start as *mut u8, size);
            self.inner
                .insert_free_block_ptr(NonNull::new(pool).unwrap())
                .unwrap();
        }
        self.total_bytes = size;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        unsafe {
            let pool = core::slice::from_raw_parts_mut(start as *mut u8, size);
            self.inner
                .insert_free_block_ptr(NonNull::new(pool).unwrap())
                .ok_or(AllocError::InvalidParam)?;
        }
        self.total_bytes += size;
        Ok(())
    }
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let size = layout.size();

        if let Some(index) = self.unfreed_sizes[self.offset..]
            .iter()
            .position(|&s| s == size)
        {
            let ptr = self.inner.allocate(layout).ok_or(AllocError::NoMemory)?;
            self.used_bytes += layout.size();

            self.unfreed_blocks[index].ptr = Some(ptr);
            self.unfreed_blocks[index].size = size;
            self.unfreed_blocks[index].align = layout.align();

            Ok(ptr)
        } else {
            let ptr = self.inner.allocate(layout).ok_or(AllocError::NoMemory)?;
            self.used_bytes += layout.size();
            Ok(ptr)
        }
    }

    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        let size = layout.size();

        unsafe { self.inner.deallocate(pos, layout.align()) }
        self.used_bytes -= layout.size();

        if size == self.marker_size {
            self.unfreed_blocks[self.offset..]
                .iter_mut()
                .filter_map(|block| block.ptr.take().map(|p| (p, block.size, block.align)))
                .for_each(|(p, unfreed_size, unfreed_align)| {
                    unsafe { self.inner.deallocate(p, unfreed_align) }
                    self.used_bytes -= unfreed_size;
                });

            self.unfreed_blocks.iter_mut().for_each(|block| {
                block.size = 0;
                block.align = 0;
            });

            if self.marker_size == 160 {
                self.offset = 0;
            }

            self.marker_size += 1;
            self.unfreed_sizes.iter_mut().for_each(|size| *size += 1);
        }
    }

    fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        self.total_bytes - self.used_bytes
    }
}
