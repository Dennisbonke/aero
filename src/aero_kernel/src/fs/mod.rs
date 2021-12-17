/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

use core::mem;

use aero_syscall::AeroSyscallError;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;

use crate::userland::scheduler;
use crate::utils::sync::Mutex;
use spin::Once;

use crate::fs::inode::FileType;

use self::cache::Cacheable;
use self::{cache::DirCacheItem, ramfs::RamFs};

pub mod block;
pub mod cache;
pub mod devfs;
pub mod file_table;
pub mod inode;
pub mod ramfs;

static ROOT_FS: Once<Arc<RamFs>> = Once::new();
static ROOT_DIR: Once<DirCacheItem> = Once::new();

lazy_static::lazy_static! {
    pub static ref MOUNT_MANAGER: MountManager = MountManager::new();
}

pub type Result<T> = core::result::Result<T, FileSystemError>;
type MountKey = (usize, String);

#[derive(Clone)]
struct MountPoint {
    filesystem: Arc<dyn FileSystem>,

    root_entry: DirCacheItem,
    origin_entry: DirCacheItem,
}

#[repr(transparent)]
pub struct MountManager(Mutex<BTreeMap<MountKey, MountPoint>>);

impl MountManager {
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    fn mount(&self, directory: DirCacheItem, filesystem: Arc<dyn FileSystem>) -> Result<()> {
        let mut this = self.0.lock();
        let mount_key = directory.cache_key();

        if this.contains_key(&mount_key) {
            return Err(FileSystemError::EntryExists);
        }

        let root_dir = filesystem.root_dir();

        let current_data = directory.data.lock();
        let mut root_data = root_dir.data.lock();

        root_data.name = current_data.name.clone();
        root_data.parent = current_data.parent.clone();

        mem::drop(root_data);
        mem::drop(current_data);

        this.insert(
            mount_key,
            MountPoint {
                filesystem,
                root_entry: root_dir,
                origin_entry: directory,
            },
        );

        Ok(())
    }

    fn find_mount(&self, directory: DirCacheItem) -> Result<MountPoint> {
        let this = self.0.lock();
        let cache_key = directory.cache_key();

        if let Some(mount_point) = this.get(&cache_key) {
            Ok(mount_point.clone())
        } else {
            Err(FileSystemError::EntryNotFound)
        }
    }
}

pub trait FileSystem: Send + Sync {
    fn root_dir(&self) -> DirCacheItem {
        todo!()
    }
}

#[derive(Debug, PartialEq)]
pub enum FileSystemError {
    NotSupported,
    EntryExists,
    EntryNotFound,
    Busy,
    NotDirectory,
}

impl From<FileSystemError> for AeroSyscallError {
    fn from(error: FileSystemError) -> Self {
        match error {
            FileSystemError::NotSupported => AeroSyscallError::EACCES,
            FileSystemError::EntryExists => AeroSyscallError::EEXIST,
            FileSystemError::EntryNotFound => AeroSyscallError::ENOENT,
            FileSystemError::Busy => AeroSyscallError::EBUSY,
            FileSystemError::NotDirectory => AeroSyscallError::ENOTDIR,
        }
    }
}

/// A slice of a path (akin to [str]).
#[derive(Debug)]
pub struct Path(str);

impl Path {
    #[inline]
    pub fn new(path: &str) -> &Self {
        unsafe { &*(path as *const str as *const Path) }
    }

    /// Returns [`true`] if the path is absolute.
    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    /// Returns an iterator over the components of the path.
    #[inline]
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split("/").filter(|e| *e != "" && *e != ".")
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Helper function that returns the parent path and the base name
    /// of the path.
    pub fn parent_and_basename(&self) -> (&Self, &str) {
        if let Some(slash_index) = self.0.rfind('/') {
            let parent_dir = if slash_index == 0 {
                Path::new("/")
            } else {
                Path::new(&self.0[..slash_index])
            };

            let basename = &self.0[(slash_index + 1)..];
            (parent_dir, basename)
        } else {
            // A relative path without any slashes.
            (Path::new(""), &self.0)
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum LookupMode {
    None,
    /// Creates the file if it does not exist.
    Create,
}

pub fn lookup_path_with(
    mut cwd: DirCacheItem,
    path: &Path,
    mode: LookupMode,
) -> Result<DirCacheItem> {
    // Iterate and resolve each component. For example `a`, `b`, and `c` in `a/b/c`.
    for (i, component) in path.components().enumerate() {
        match component {
            // Handle some special cases that might occur in a relative path.
            "." => continue,
            ".." => {
                let current = cwd.data.lock();

                if let Some(parent) = current.parent.clone() {
                    core::mem::drop(current); // drop the data lock.
                    cwd = parent;
                }

                // Else the entry does not have a parent, ie. the current entry is the root aand
                // we can't go any further :^)
            }

            _ => {
                // After we have resolved all of the special cases that might occur in a path, now
                // we have to resolve the directory entry itself. For example `a` in `./a/`.
                let cache_entry = inode::fetch_dir_entry(cwd.clone(), String::from(component));

                if let Some(entry) = cache_entry {
                    cwd = entry;
                } else {
                    match cwd.inode().lookup(cwd.clone(), component) {
                        Ok(entry) => cwd = entry,

                        Err(err)
                            if err == FileSystemError::EntryNotFound
                                && i == path.components().count() - 1
                                && mode == LookupMode::Create =>
                        {
                            cwd.inode().touch(cwd.clone(), component)?;
                        }

                        Err(err) => return Err(err),
                    }
                }

                if cwd.inode().metadata()?.file_type == FileType::Directory {
                    if let Ok(mount_point) = MOUNT_MANAGER.find_mount(cwd.clone()) {
                        cwd = mount_point.root_entry;
                    }
                }
            }
        }
    }

    Ok(cwd)
}

pub fn lookup_path(path: &Path) -> Result<DirCacheItem> {
    let cwd = if !path.is_absolute() {
        scheduler::get_scheduler().current_task().get_cwd_dirent()
    } else {
        root_dir().clone()
    };

    lookup_path_with(cwd, path, LookupMode::None)
}

pub fn root_dir() -> &'static DirCacheItem {
    ROOT_DIR.get().expect("How's this possible?")
}

pub fn init() -> Result<()> {
    cache::init();

    let filesystem = RamFs::new();

    ROOT_FS.call_once(|| filesystem.clone());
    ROOT_DIR.call_once(|| filesystem.root_dir().clone());

    root_dir().inode().mkdir("dev")?;
    root_dir().inode().mkdir("home")?;
    root_dir().inode().mkdir("temp")?;

    devfs::init()?;
    log::info!("Installed devfs");

    Ok(())
}

pub fn launch() -> Result<()> {
    // This is temporary and will be removed when we have support for
    // ext2 filesystem and fat32 filesystem to make it functional for both
    // UEFI and BIOS.

    #[cfg(not(doc))]
    {
        #[cfg(debug_assertions)]
        static SHELL: &[u8] =
            include_bytes!("../../../../userland/target/x86_64-unknown-none/debug/aero_shell");

        #[cfg(not(debug_assertions))]
        static SHELL: &[u8] =
            include_bytes!("../../../../userland/target/x86_64-unknown-none/release/aero_shell");

        root_dir().inode().mkdir("bin")?;
        root_dir().inode().mkdir("lib")?;

        let bin = lookup_path(Path::new("/bin"))?;
        let shell = bin.inode().touch(bin.clone(), "sh")?;

        shell.inode().write_at(0x00, SHELL)?;
    }

    // Add some more files if the sysroot feature is enabled.
    #[cfg(feature = "sysroot")]
    {
        macro_rules! include_bin {
            ($name:expr => $path:expr) => {{
                let file = bin.inode().touch(bin.clone(), $name)?;

                static BYTES: &[u8] = include_bytes!($path);
                file.inode().write_at(0x00, BYTES)?;
            }};
        }

        let lib = lookup_path(Path::new("/lib"))?;

        include_bin!("ld" => "../../../../sysroot/system-root/usr/bin/ld");

        static LD_SO: &[u8] = include_bytes!("../../../../sysroot/system-root/usr/lib/ld.so");

        let ldso = lib.inode().touch(lib.clone(), "ld.so")?;
        ldso.inode().write_at(0x00, LD_SO)?;
    }

    Ok(())
}
