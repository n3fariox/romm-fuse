use std::collections::HashMap;
use std::ffi::OsStr;
use std::io;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use fuser::{
    AccessFlags, Config, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation,
    INodeNo, MountOption, OpenFlags, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyOpen, ReplyStatfs, Request, SessionACL,
};
use log::{debug, info, warn};

use crate::api::client::RommClient;
use crate::config::ProfileConfig;
use crate::fs::cache::Cache;
use crate::fs::tree::{FileTree, TreeNode};

pub struct RommFs {
    client: Arc<RommClient>,
    cache: Arc<Cache>,
    tree: Arc<RwLock<FileTree>>,
    #[allow(dead_code)]
    platform_dirs: HashMap<String, u64>,
    #[allow(dead_code)]
    ttl: Duration,
}

const ROOT_INO: u64 = 1;
const BLOCK_SIZE: u64 = 512;

impl RommFs {
    pub fn new(
        client: RommClient,
        cache: Cache,
        profile: &ProfileConfig,
        ttl: Duration,
    ) -> Result<Self> {
        let mut tree = FileTree::new();
        let mut platform_dirs = HashMap::new();

        info!("fetching platforms from RomM");
        let platforms = client.list_platforms()?;
        info!("found {} platforms", platforms.len());

        for platform in &platforms {
            if let Some(dir_name) = profile.platforms.get(&platform.slug) {
                let prefix = profile.profile.prefix.as_deref().unwrap_or("");

                let parent_ino = if prefix.is_empty() {
                    ROOT_INO
                } else {
                    // Ensure prefix directory exists (e.g. "games")
                    if let Some(existing) = tree.lookup(ROOT_INO, prefix) {
                        existing
                    } else {
                        let prefix_ino = tree.allocate_inode();
                        tree.add_dir(ROOT_INO, prefix.to_string(), prefix_ino);
                        info!("created prefix directory '{}'", prefix);
                        prefix_ino
                    }
                };

                let dir_ino = tree.allocate_inode();
                tree.add_dir(parent_ino, dir_name.clone(), dir_ino);
                platform_dirs.insert(platform.slug.clone(), dir_ino);

                info!(
                    "platform '{}' -> '{}' ({} ROMs)",
                    platform.slug, dir_name, platform.rom_count
                );

                if platform.rom_count > 0 {
                    info!("fetching ROMs for platform '{}'...", platform.slug);
                    let roms = client.list_all_roms(platform.id)?;
                    info!("found {} ROMs for '{}'", roms.len(), platform.slug);
                    tree.build_from_roms(&HashMap::from([(platform.slug.clone(), dir_ino)]), &roms);
                }
            }
        }

        info!("filesystem ready: {} nodes", tree.nodes.len());

        Ok(Self {
            client: Arc::new(client),
            cache: Arc::new(cache),
            tree: Arc::new(RwLock::new(tree)),
            platform_dirs,
            ttl,
        })
    }

    fn getattr_common(&self, ino: u64) -> Option<FileAttr> {
        let tree = self.tree.read().ok()?;
        let node = tree.get(ino)?;

        match node {
            TreeNode::Dir { .. } => Some(FileAttr {
                ino: INodeNo(ino),
                size: 0,
                blocks: 0,
                atime: SystemTime::now(),
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: FileType::Directory,
                perm: 0o555,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
                blksize: BLOCK_SIZE as u32,
            }),
            TreeNode::File { size, .. } => {
                let blocks = size.div_ceil(BLOCK_SIZE);
                Some(FileAttr {
                    ino: INodeNo(ino),
                    size: *size,
                    blocks,
                    atime: SystemTime::now(),
                    mtime: SystemTime::now(),
                    ctime: SystemTime::now(),
                    crtime: SystemTime::now(),
                    kind: FileType::RegularFile,
                    perm: 0o444,
                    nlink: 1,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    flags: 0,
                    blksize: BLOCK_SIZE as u32,
                })
            }
        }
    }
}

impl Filesystem for RommFs {
    fn init(&mut self, _req: &Request, _config: &mut fuser::KernelConfig) -> io::Result<()> {
        info!("FUSE filesystem initialized");
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(Errno::EIO);
                return;
            }
        };

        let tree = match self.tree.read() {
            Ok(t) => t,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        match tree.lookup(parent.0, name_str) {
            Some(ino) => {
                if let Some(attr) = self.getattr_common(ino) {
                    reply.entry(&Duration::from_secs(3600), &attr, Generation(0));
                } else {
                    reply.error(Errno::ENOENT);
                }
            }
            None => reply.error(Errno::ENOENT),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        if let Some(attr) = self.getattr_common(ino.0) {
            reply.attr(&Duration::from_secs(3600), &attr);
        } else {
            reply.error(Errno::ENOENT);
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let tree = match self.tree.read() {
            Ok(t) => t,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        let children = match tree.children(ino.0) {
            Some(c) => c,
            None => {
                reply.error(Errno::ENOTDIR);
                return;
            }
        };

        let mut entries: Vec<(u64, FileType, String)> = Vec::with_capacity(children.len() + 2);

        if ino.0 != ROOT_INO {
            entries.push((ino.0, FileType::Directory, ".".to_string()));
            entries.push((ROOT_INO, FileType::Directory, "..".to_string()));
        }

        for (name, child_ino) in &children {
            let ft = match tree.get(*child_ino) {
                Some(TreeNode::Dir { .. }) => FileType::Directory,
                _ => FileType::RegularFile,
            };
            entries.push((*child_ino, ft, name.clone()));
        }

        for (i, (child_ino, file_type, name)) in entries.iter().enumerate() {
            if (i as u64) < offset {
                continue;
            }
            if reply.add(
                INodeNo(*child_ino),
                (i as u64) + 1,
                *file_type,
                name.as_str(),
            ) {
                break;
            }
        }
        reply.ok();
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        let tree = match self.tree.read() {
            Ok(t) => t,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        match tree.get(ino.0) {
            Some(TreeNode::Dir { .. }) => {
                reply.opened(FileHandle(ino.0), FopenFlags::empty());
            }
            _ => {
                reply.error(Errno::ENOTDIR);
            }
        }
    }

    fn access(&self, _req: &Request, ino: INodeNo, _mask: AccessFlags, reply: ReplyEmpty) {
        let tree = match self.tree.read() {
            Ok(t) => t,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        match tree.get(ino.0) {
            Some(_) => reply.ok(),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        let tree = match self.tree.read() {
            Ok(t) => t,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        match tree.get(ino.0) {
            Some(TreeNode::File { .. }) => {
                debug!("opened rom inode {ino}");
                reply.opened(FileHandle(ino.0), FopenFlags::empty());
            }
            Some(TreeNode::Dir { .. }) => {
                reply.error(Errno::EISDIR);
            }
            None => {
                reply.error(Errno::ENOENT);
            }
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let tree = match self.tree.read() {
            Ok(t) => t,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        let (rom_id, file_name, file_size) = match tree.get(ino.0) {
            Some(TreeNode::File {
                rom_id,
                file_name,
                size,
                ..
            }) => (*rom_id, file_name.clone(), *size),
            _ => {
                reply.error(Errno::EISDIR);
                return;
            }
        };
        drop(tree);

        let data =
            match self
                .cache
                .read_range(&self.client, rom_id, &file_name, file_size, offset, size)
            {
                Ok(d) => d,
                Err(e) => {
                    warn!("failed to read rom {rom_id} file {file_name}: {e}");
                    reply.error(Errno::EIO);
                    return;
                }
            };

        reply.data(&data);
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let cache_dir = self.cache.dir_for_statfs();
        let stat = unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            let path = std::ffi::CString::new(cache_dir.to_str().unwrap_or("/tmp")).unwrap();
            if libc::statvfs(path.as_ptr(), &mut stat) == 0 {
                Some(stat)
            } else {
                None
            }
        };

        let (blocks, bfree, bavail, files, ffree, fsize) = match stat {
            Some(s) => (
                s.f_blocks,
                s.f_bfree,
                s.f_bavail,
                s.f_files,
                s.f_ffree,
                s.f_frsize as u32,
            ),
            None => (0, 0, 0, 0, 0, BLOCK_SIZE as u32),
        };
        reply.statfs(blocks, bfree, bavail, files, ffree, fsize, 255, fsize);
    }

    fn forget(&self, _req: &Request, _ino: INodeNo, _nlookup: u64) {}
}

pub fn mount(args: crate::config::ResolvedConfig) -> Result<()> {
    let profile = ProfileConfig::load(&args)?;

    let client = RommClient::new(&args.romm_url, &args.token)?;
    let cache = Cache::new(
        &args.cache_dir,
        Duration::from_secs(args.ttl),
        args.chunk_size,
    )?;
    let ttl = Duration::from_secs(args.ttl);

    let fs = RommFs::new(client, cache, &profile, ttl)?;

    let options = vec![
        MountOption::RO,
        MountOption::FSName("romm-fuse".to_string()),
    ];

    let mut config = Config::default();
    config.mount_options = options;

    if args.allow_other {
        config.acl = SessionACL::All;
    }

    let mountpoint = args
        .mountpoint
        .clone()
        .ok_or_else(|| anyhow::anyhow!("mountpoint is required"))?;

    info!("mounting at {}", mountpoint.display());
    fuser::mount2(fs, &mountpoint, &config)?;

    Ok(())
}
