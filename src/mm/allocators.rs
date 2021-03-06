use buddy_system_allocator::LockedHeap;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::config::*;

use super::{
    PhysPageNum,
    PhysAddr
};

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

pub fn init_heap() {
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

lazy_static! {
    static ref FRAME_ALL0CATOR: Mutex<FrameAllocator> = {
        extern "C" {
            fn ekernel();
        }
        let start = PhysAddr::from(ekernel as usize).floor();
        let end = PhysAddr::from(MEMORY_END).ceil();
        Mutex::new(FrameAllocator::new(start, end))
    };
}

pub struct FrameTracker(PhysPageNum);

impl FrameTracker {
    pub fn ppn(&self) -> PhysPageNum {
        self.0.clone()
    }
}

struct FrameAllocator {
    current: PhysPageNum,
    end: PhysPageNum,
    recycled: Vec<PhysPageNum>
}

impl FrameAllocator {
    pub fn new(start: PhysPageNum, end: PhysPageNum) -> Self {
        (start.value()..end.value()).into_iter().for_each(|v| {
            let ppn: PhysPageNum = v.into();
            ppn.clean();
        });

        Self {
            current: start,
            end,
            recycled: Vec::new()
        }
    }

    pub fn alloc(&mut self) -> Option<FrameTracker> {
        match self.recycled.pop() {
            Some(p) => Some(p.into()),
            None if self.current < self.end => {
                self.current += 1;
                Some((self.current - 1).into())
            },
            None => None
        }
    }

    pub fn dealloc(&mut self, ppn: PhysPageNum) {
        if ppn >= self.current || self.recycled.contains(&ppn) {
            panic!("Frame {:?} has not been allocated!", ppn);
        }
        self.recycled.push(ppn);
    }
}

impl From<PhysPageNum> for FrameTracker {
    fn from(p: PhysPageNum) -> Self {
        Self(p)
    }
}

pub fn frame_alloc() -> FrameTracker {
    match FRAME_ALL0CATOR.lock().alloc() {
        Some(f) => f,
        None => panic!("No frame can be allocated")
    }
}

pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALL0CATOR.lock().dealloc(ppn)
}

// RAII
impl Drop for FrameTracker {
    fn drop(&mut self) {
        // clean page table entrys
        self.ppn().clean();
        frame_dealloc(self.ppn())
    }
}
