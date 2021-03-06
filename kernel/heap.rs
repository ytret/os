// ytret's OS - hobby operating system
// Copyright (C) 2020, 2021  Yuri Tretyakov (ytretyakov18@gmail.com)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::kernel_static::Mutex;
use crate::memory_region::Region;
use crate::KERNEL_INFO;

use core::alloc::{GlobalAlloc, Layout};
use core::mem::{align_of, size_of};

struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // println!(
        //     "alloc: layout: size: {}, align: {}",
        //     layout.size(),
        //     layout.align(),
        // );

        let heap = match *KERNEL_HEAP.lock() {
            Some(kernel_heap) => kernel_heap,
            None => panic!("Kernel heap is not initiailized."),
        };

        // Find a suitable free chunk.
        let mut needed_size = 0;
        let mut chosen_tag: *mut Tag = core::ptr::null_mut();
        let mut chunk_start: *mut u8 = core::ptr::null_mut();
        for possible_tag in heap.iter_free_tags() {
            let chunk_size = possible_tag.chunk_size();
            chunk_start = (possible_tag as *mut Tag).offset(1) as *mut u8;
            needed_size = ((chunk_start as usize + layout.align() - 1)
                & !(layout.align() - 1))
                - chunk_start as usize
                + layout.size();
            if chunk_size >= needed_size {
                chosen_tag = possible_tag as *mut Tag;
                break;
            }
        }
        if chosen_tag.is_null() {
            panic!(
                "alloc: insufficient free heap: {} bytes, need: {} bytes",
                heap.total_free(),
                needed_size,
            );
            //return core::ptr::null_mut();
        }

        // Add +1 byte just in case an alignment for the tag is needed.
        if (*chosen_tag).chunk_size() - needed_size
            < size_of::<Tag>() + heap.min_chunk_size + 1
        {
            (*chosen_tag).set_used(true);
        } else {
            // Divide the chunk.
            let second_part = (((chosen_tag.add(1) as usize + needed_size) + 1)
                & !1) as *mut Tag;
            *second_part = Tag::new(false, 1, (*chosen_tag).next_tag());
            *chosen_tag = Tag::new(true, layout.align(), second_part);
        }

        let aligned = chunk_start.add(chunk_start.align_offset(layout.align()));
        assert_eq!(
            aligned as usize,
            (chunk_start as usize + layout.align() - 1) & !(layout.align() - 1),
        );

        // Place 0xFF's right before the aligned start so that it will be easy
        // to find the tag (Tag::align is never 0xFF).
        let n = aligned as usize - chunk_start as usize;
        (chunk_start as *mut u8).write_bytes(0xFF, n);

        assert_eq!(aligned.align_offset(layout.align()), 0);
        assert_ne!(aligned as usize, chosen_tag as usize);
        aligned
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // println!(
        //     "dealloc: ptr: 0x{:08X}, layout: size: {}, align: {}",
        //     ptr as u32,
        //     layout.size(),
        //     layout.align(),
        // );

        assert_eq!(
            ptr.align_offset(layout.align()),
            0,
            "dealloc: ptr is not properly aligned",
        );

        let heap = match *KERNEL_HEAP.lock() {
            Some(kernel_heap) => kernel_heap,
            None => panic!("dealloc on uninitialized kernel heap"),
        };

        let mut tag_ptr: *const u8 = ptr.sub(1);
        while *tag_ptr == 0xFF {
            tag_ptr = tag_ptr.sub(1);
        }

        let tag = (tag_ptr.add(1) as *mut Tag).sub(1);
        // println!(
        //     "- tag at 0x{:08X} -> 0x{:08X}, used: {}, align: {}, size: {}",
        //     tag as u32,
        //     (*tag).next_tag_addr(),
        //     (*tag).is_used() as usize,
        //     (*tag).align(),
        //     (*tag).chunk_size(),
        // );

        (*tag).set_used(false);
        (*tag).align = 1;

        heap.join_adjacent_free_chunks();
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator;

#[alloc_error_handler]
fn alloc_error_handler(_: Layout) -> ! {
    panic!("alloc_error_handler called");
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Tag {
    magic_1: u32,
    value: usize,
    align: usize,
    magic_2: u32,
}

impl Tag {
    fn new(used: bool, align: usize, next_tag: *const Tag) -> Self {
        let addr = next_tag as usize;
        assert_eq!(addr & 1, 0, "next_tag must be aligned at 2 bytes");
        assert_eq!(align.count_ones(), 1, "align must be a power of two");
        Tag {
            magic_1: 0xDEADBEEF,
            value: addr | used as usize,
            align,
            magic_2: 0xCAFEBABE,
        }
    }

    fn check_magic(&self) {
        assert_eq!(
            { self.magic_1 },
            0xDEADBEEF,
            "tag: 0x{:08X}",
            self as *const _ as usize,
        );
        assert_eq!(
            { self.magic_2 },
            0xCAFEBABE,
            "tag: 0x{:08X}",
            self as *const _ as usize,
        );
    }

    fn is_used(&self) -> bool {
        match self.value & 1 {
            1 => true,
            0 => false,
            _ => unreachable!(),
        }
    }

    fn is_end_tag(&self) -> bool {
        self.value == 0
    }

    fn next_tag_addr(&self) -> usize {
        self.value as usize & !1
    }

    fn next_tag(&self) -> *mut Tag {
        self.next_tag_addr() as *mut Tag
    }

    fn align(&self) -> usize {
        self.align
    }

    fn chunk_size(&self) -> usize {
        if self.is_end_tag() {
            0
        } else {
            let start = self as *const _ as usize + size_of::<Tag>();
            let end = self.next_tag_addr();
            assert!(
                end > start,
                "self: 0x{:08X}, start: 0x{:08X}, end: 0x{:08X}",
                self as *const _ as usize,
                start,
                end,
            );
            end - start
        }
    }

    fn set_used(&mut self, used: bool) {
        if used {
            self.value |= 1;
        } else {
            self.value &= !1;
        }
    }
}

#[derive(Clone, Copy)]
pub struct Heap {
    region: Region<usize>,
    min_chunk_size: usize,
}

impl Heap {
    fn first_tag(&self) -> *mut Tag {
        self.region.start as *mut Tag
    }

    fn total_free(&self) -> usize {
        let mut total_free: usize = 0;
        for tag in self.iter_free_tags() {
            if !tag.is_end_tag() {
                total_free += tag.chunk_size();
            }
        }
        total_free
    }

    pub fn join_adjacent_free_chunks(&self) {
        let mut from: *mut Tag = core::ptr::null_mut();
        let mut to: *const Tag = core::ptr::null();
        for tag in self.iter_tags() {
            if !tag.is_used() && !tag.is_end_tag() {
                if from.is_null() {
                    from = tag;
                } else {
                    to = tag;
                }
            } else if !to.is_null() {
                unsafe {
                    *from = Tag::new(false, 1, (*to).next_tag());
                }
                from = core::ptr::null_mut();
                to = core::ptr::null();
            } else {
                from = core::ptr::null_mut();
            }
        }
    }

    fn iter_tags(&self) -> HeapIter {
        HeapIter {
            heap: self,
            current_tag: core::ptr::null_mut(),
            only_free: false,
        }
    }

    fn iter_free_tags(&self) -> HeapIter {
        HeapIter {
            heap: self,
            current_tag: core::ptr::null_mut(),
            only_free: true,
        }
    }

    #[allow(dead_code)]
    pub fn print(&self) {
        for tag in self.iter_tags() {
            println!(
                "- tag at 0x{:08X} -> 0x{:08X}, used: {}, align: {}, \
                 chunk size: {}",
                tag as *const _ as usize,
                tag.next_tag_addr(),
                tag.is_used() as usize,
                tag.align(),
                tag.chunk_size(),
            );
        }
    }

    #[allow(dead_code)]
    pub fn stats(&self) {
        let mut used_sizes: [(usize, usize); 32] = [(0, 0); 32];
        let mut free_sizes: [(usize, usize); 32] = [(0, 0); 32];
        for tag in self.iter_tags() {
            let size = tag.chunk_size();
            let sizes = if tag.is_used() {
                &mut used_sizes
            } else {
                &mut free_sizes
            };
            if let Some(idx) = sizes.iter().position(|x| x.0 == size) {
                sizes[idx].1 += 1;
            } else if let Some(idx) = sizes.iter().position(|x| x.0 == 0) {
                sizes[idx] = (size, 1);
            } else {
                println!("[HEAP] Skipping size: {}.", size);
            }
        }
        used_sizes.sort_unstable_by_key(|&x| x.1);
        used_sizes.reverse();
        free_sizes.sort_unstable_by_key(|&x| x.1);
        free_sizes.reverse();
        println!("[HEAP] Used sizes: {:?}.", used_sizes);
        println!("[HEAP] Free sizes: {:?}.", free_sizes);
    }
}

struct HeapIter<'a> {
    heap: &'a Heap,
    current_tag: *mut Tag,
    only_free: bool,
}

impl<'a> Iterator for HeapIter<'a> {
    type Item = &'a mut Tag;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.current_tag.is_null() {
                self.current_tag = self.heap.first_tag() as *mut Tag;
                if !self.only_free || !(*self.current_tag).is_used() {
                    let tag = self.current_tag.as_mut().unwrap();
                    tag.check_magic();
                    return Some(tag);
                } else {
                    // self.only_free && (*self.current_tag).is_used()
                    // continue (see below)
                }
            }

            loop {
                self.current_tag = (*self.current_tag).next_tag();
                if self.current_tag.is_null() {
                    return None;
                } else if !self.only_free
                    || (self.only_free && !(*self.current_tag).is_used())
                {
                    let tag = self.current_tag.as_mut().unwrap();
                    tag.check_magic();
                    return Some(tag);
                }
            }
        }
    }
}

pub const KERNEL_HEAP_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

kernel_static! {
    pub static ref KERNEL_HEAP: Mutex<Option<Heap>> = Mutex::new(None);
}

pub fn init() {
    if KERNEL_HEAP.lock().is_some() {
        println!("[HEAP] Kernel heap has already been initialized.");
        return;
    }

    let heap_region = unsafe { KERNEL_INFO.arch.heap_region };
    assert!(
        heap_region.len() > 2 * size_of::<Tag>(),
        "heap must be big enough to accomodate at least two tags",
    );

    let heap_start_tag_ptr = heap_region.start as *mut Tag;
    let heap_end_tag_ptr = (heap_region.end - size_of::<Tag>()) as *mut Tag;
    assert_eq!(
        heap_start_tag_ptr.align_offset(align_of::<Tag>()),
        0,
        "heap start must be properly aligned",
    );
    assert_eq!(
        heap_end_tag_ptr.align_offset(align_of::<Tag>()),
        0,
        "heap end must be properly aligned",
    );

    let start_tag = Tag::new(false, 1, heap_end_tag_ptr);
    let end_tag = Tag::new(false, 1, core::ptr::null());

    unsafe {
        *heap_start_tag_ptr = start_tag;
        *heap_end_tag_ptr = end_tag;

        *KERNEL_HEAP.lock() = Some(Heap {
            region: heap_region,
            min_chunk_size: 1,
        });
    }

    println!(
        "Heap: start: 0x{:08X}, end: 0x{:08X}, total free: {} bytes",
        heap_region.start,
        heap_region.end,
        KERNEL_HEAP.lock().unwrap().total_free(),
    );
}
