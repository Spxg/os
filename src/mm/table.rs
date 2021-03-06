#![allow(unused)]

use core::cmp::min;

use alloc::string::String;
use crate::config::*;
use crate::sbi::console_getchar;
use super::{
    FrameTracker, 
    PTEFlags, 
    PageTableEntry, 
    PhysAddr, 
    PhysPageNum, 
    VirtAddr, 
    VirtPageNum, 
    frame_alloc
};
use alloc::{
    vec,
    vec::Vec
};

// define mode, Sv32 not impl, just look...
pub enum Mode {
    #[allow(unused)]
    Bare = 0,
    #[allow(unused)]
    Sv32 = 1,
    Sv39 = 8,
    #[allow(unused)]
    Sv48 = 9
}

pub struct PageTable {
    root: PhysPageNum,
    frames: Vec<FrameTracker>
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc();
        Self {
            root: frame.ppn(),
            frames: vec![frame]
        }
    }

    pub fn from_satp(satp: usize) -> Self {
        Self {
            root: PhysPageNum::from(satp & ((1 << 44) - 1)),
            frames: Vec::new(),
        }
    }

    pub fn satp_bits(&self, mode: Mode) -> usize {
        (mode as usize) << 60 | usize::from(self.root)
    }

    fn find_pte(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let indexes = vpn.indexes();
        let mut ppn = self.root;
        for &i in &indexes[0..2] {
            let pte = &mut ppn.get_ptes()[i];
            if !pte.is_valid() { return None; }
            ppn = pte.ppn();
        }
        Some(&ppn.get_ptes()[indexes[2]])
    }

    fn find_pte_by_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let indexes = vpn.indexes();
        let mut ppn = self.root;
        for &i in &indexes[0..2] {
            let pte = &mut ppn.get_ptes()[i];
            if !pte.is_valid() {
                let f = frame_alloc();
                *pte = PageTableEntry::new(f.ppn(), PTEFlags::V);
                self.frames.push(f);
            }
            ppn = pte.ppn();
        }
        Some(&mut ppn.get_ptes()[indexes[2]])
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| pte.clone())
    }

    // need to align
    // example(Identical map):
    // if va is 0x80120066, pa is 0x80120000 + 0x66
    pub fn translate_va_to_pa(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.into()).map(|pte| {
            let aligned_pa: PhysAddr = pte.ppn().into();
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into()
        })
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = match self.find_pte_by_create(vpn) {
            Some(pte) => pte,
            None => unreachable!()
        };

        assert!(!pte.is_valid(), "{:?} is valid before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = match self.find_pte_by_create(vpn) {
            Some(pte) => pte,
            None => unreachable!()
        };
        
        assert!(pte.is_valid(), "{:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
}

pub fn translated_byte_buffer(
    satp: usize, 
    ptr: *const u8, 
    len: usize
) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_satp(satp);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();

    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table
            .translate(vpn)
            .unwrap()
            .ppn();
        vpn += 1;
        let mut end_va: VirtAddr = vpn.into();
        end_va = min(end_va.value(), end).into();

        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_page_bytes()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_page_bytes()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

pub fn translated_str(satp: usize, ptr: *const u8, len: usize) -> String {
    let page_table = PageTable::from_satp(satp);
    let mut vec_str = Vec::new();
    let mut va = ptr as usize;
    
    for _ in 0..len {
        let pa = page_table.translate_va_to_pa(
            VirtAddr::from(va)
        ).unwrap();
        let ch = *pa.get_mut::<u8>();
        vec_str.push(ch);
        va += 1;
    }
    
    String::from_utf8(vec_str).unwrap()
}

pub fn translated_ref<T>(satp: usize, ptr: *const T) -> &'static T {
    let page_table = PageTable::from_satp(satp);
    let mut va = (ptr as usize).into();
    page_table.translate_va_to_pa(va).unwrap().get_ref()
}

pub fn translated_refmut<T>(satp: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_satp(satp);
    let mut va = (ptr as usize).into();
    page_table.translate_va_to_pa(va).unwrap().get_mut()
}
