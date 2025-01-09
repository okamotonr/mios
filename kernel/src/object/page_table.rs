use crate::{
    common::{Err, KernelResult}, memory::{PhysAddr, VirtAddr, PAGE_SIZE}, println, vm::{KernelVAddress, KERNEL_VM_ROOT}
};

use core::arch::asm;
use core::ptr;

pub const SATP_SV48: usize = 9 << 60;
pub const PAGE_V: usize = 1 << 0;
pub const PAGE_R: usize = 1 << 1;
pub const PAGE_W: usize = 1 << 2;
pub const PAGE_X: usize = 1 << 3;
pub const PAGE_U: usize = 1 << 4;

// page table lv1(bottom) has 512 * 4kb page = 2048kb
// page table lv2(middle) has 512 * lv1 table = 512 * 2048kb
// ...

// TODO: use array [PTE; 512]
// TODO: root page table and other tables should be different type?
pub struct PageTable;

impl PageTable {
    pub fn new() -> Self {
        Self
    }
    pub fn map(&self, parent: &mut Self, vaddr: VirtAddr) -> KernelResult<usize> {
        let (level, entry) = parent.walk(vaddr);
        if level == 0 {
            Err(Err::VaddressAlreadyMapped)
        } else {
            entry.write(self as *const _ as usize, PAGE_V);
            Ok(level - 1)
        }
    }

    fn get_pte(&mut self, vpn: usize) -> &mut PTE {
        unsafe { (self as *mut Self).cast::<PTE>().add(vpn).as_mut().unwrap() }
    }

    pub fn walk(&mut self, vaddr: VirtAddr) -> (usize, &mut PTE) {
        let mut page_table = self;
        // walk page table
        for level in (1..=3).rev() {
            let vpn = vaddr.get_vpn(level);
            let pte = page_table.get_pte(vpn);
            if !pte.is_valid() {
                return (level, pte);
            }
            page_table = pte.as_page_table();
        }

        let pte = page_table.get_pte(vaddr.get_vpn(0));
        (0, pte)
    }

    pub unsafe fn activate(&self) {
        let addr: PhysAddr = KernelVAddress::from(self as *const Self).into();
        println!("{:?}", addr);
        asm!(
            "sfence.vma x0, x0",
            "csrw satp, {satp}",
            "sfence.vma x0, x0",
            satp = in(reg) (SATP_SV48 | (addr.addr >> 12))
        )
    }

    pub fn copy_global_mapping(&mut self) {
        let self_addr = self as *mut PageTable as *mut u8;
        unsafe {
            let k_root = &raw const KERNEL_VM_ROOT as *const u8;
            ptr::copy::<u8>(k_root, self_addr, PAGE_SIZE);
        };
    }
}

// 4kb page
pub struct Page;

impl Page {
    pub fn new() -> Self {
        Self
    }
    pub fn map(&self, parent: &mut PageTable, vaddr: VirtAddr, flags: usize) -> KernelResult<()> {
        let (level, entry) = parent.walk(vaddr);
        if level != 0 {
            Err(Err::PageTableNotMappedYet)
        } else {
            if entry.is_valid() {
                Err(Err::VaddressAlreadyMapped)
            } else {
                entry.write(self as *const _ as usize, flags | PAGE_V);
                Ok(())
            }
        }
    }
}

pub struct PTE(usize);

impl PTE {
    pub fn is_valid(&self) -> bool {
        self.0 & PAGE_V != 0
    }

    pub fn write(&mut self, addr: usize, flags: usize) {
        self.0 = ((addr >> 12) << 10) | flags
    }

    pub fn as_page_table(&mut self) -> &mut PageTable {
        let raw = (self.0 << 2) & !0xfff;
        unsafe { (raw as *mut PageTable).as_mut().unwrap() }
    }
}
