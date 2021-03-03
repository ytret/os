// ytret's OS - hobby operating system
// Copyright (C) 2020  Yuri Tretyakov (ytretyakov18@gmail.com)
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

use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{DirEntryContent, Directory, FileSystem};
use crate::disk::Disk;

pub struct Node {
    _type: NodeType,
    name: String,
    children: Vec<Rc<RefCell<Node>>>,
}

impl Node {
    fn new_root(disk: Rc<Disk>) -> Self {
        let mut root_node = Node {
            _type: NodeType::MountPoint(Rc::clone(&disk)),
            name: String::from("/"),
            children: Vec::new(),
        };
        let fs = disk.file_system.as_ref().unwrap();
        root_node.fill_children(fs.root_dir().unwrap(), &fs);
        root_node
    }

    fn fill_children(&mut self, dir: Directory, fs: &Box<dyn FileSystem>) {
        assert!(self.children.is_empty());
        for entry in dir.entries {
            match entry.content {
                DirEntryContent::Unknown => {
                    println!("[VFS] fill_children: skipping an unknown entry");
                }
                DirEntryContent::RegularFile => {
                    println!(
                        "RegularFile id {}, name {}",
                        entry.id, entry.name,
                    );
                    self.children.push(Rc::new(RefCell::new(Node {
                        _type: NodeType::RegularFile,
                        name: entry.name,
                        children: Vec::with_capacity(0),
                    })));
                }
                DirEntryContent::Directory => {
                    if entry.name == "." {
                        println!("Skipping directory .");
                        continue;
                    } else if entry.name == ".." {
                        println!("Skipping directory ..");
                        continue;
                    }
                    println!("Directory id {}, name {}", entry.id, entry.name);
                    let node = Rc::new(RefCell::new(Node {
                        _type: NodeType::Dir,
                        name: entry.name,
                        children: Vec::new(),
                    }));
                    let dir = fs.read_dir(entry.id).unwrap();
                    node.borrow_mut().fill_children(dir, fs);
                    self.children.push(node);
                }
            }
        }
    }
}

enum NodeType {
    MountPoint(Rc<Disk>),
    RegularFile,
    Dir,
}

pub static mut VFS: Option<Node> = None;

pub fn init(root_disk: Rc<Disk>) {
    let fs = root_disk.file_system.as_ref().unwrap();

    println!("[VFS] Initializing VFS with root on disk {}.", root_disk.id);
    let root_node = Node::new_root(Rc::clone(&root_disk));

    unsafe {
        VFS = Some(root_node);
    }
    println!("[VFS] Init was finished.");
}
