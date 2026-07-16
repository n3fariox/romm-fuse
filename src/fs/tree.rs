use std::collections::HashMap;

use crate::api::types::SimpleRom;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TreeNode {
    Dir {
        #[allow(dead_code)]
        name: String,
        children: Vec<String>,
    },
    File {
        #[allow(dead_code)]
        name: String,
        rom_id: u64,
        #[allow(dead_code)]
        file_id: Option<u64>,
        file_name: String,
        size: u64,
    },
}

pub struct FileTree {
    pub nodes: HashMap<u64, TreeNode>,
    pub names: HashMap<(u64, String), u64>,
    pub parent: HashMap<u64, u64>,
    next_ino: u64,
}

impl Default for FileTree {
    fn default() -> Self {
        Self::new()
    }
}

impl FileTree {
    pub fn new() -> Self {
        let mut tree = Self {
            nodes: HashMap::new(),
            names: HashMap::new(),
            parent: HashMap::new(),
            next_ino: 2,
        };
        tree.nodes.insert(
            1,
            TreeNode::Dir {
                name: String::new(),
                children: Vec::new(),
            },
        );
        tree
    }

    pub fn allocate_inode(&mut self) -> u64 {
        let ino = self.next_ino;
        self.next_ino += 1;
        ino
    }

    pub fn add_dir(&mut self, parent: u64, name: String, ino: u64) {
        self.nodes.insert(
            ino,
            TreeNode::Dir {
                name: name.clone(),
                children: Vec::new(),
            },
        );
        self.names.insert((parent, name.clone()), ino);
        self.parent.insert(ino, parent);

        if let Some(TreeNode::Dir { children, .. }) = self.nodes.get_mut(&parent) {
            children.push(name);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_file(
        &mut self,
        parent: u64,
        name: String,
        ino: u64,
        rom_id: u64,
        file_id: Option<u64>,
        file_name: String,
        size: u64,
    ) {
        self.nodes.insert(
            ino,
            TreeNode::File {
                name: name.clone(),
                rom_id,
                file_id,
                file_name,
                size,
            },
        );
        self.names.insert((parent, name.clone()), ino);
        self.parent.insert(ino, parent);

        if let Some(TreeNode::Dir { children, .. }) = self.nodes.get_mut(&parent) {
            children.push(name);
        }
    }

    pub fn lookup(&self, parent: u64, name: &str) -> Option<u64> {
        self.names.get(&(parent, name.to_string())).copied()
    }

    pub fn get(&self, ino: u64) -> Option<&TreeNode> {
        self.nodes.get(&ino)
    }

    pub fn children(&self, ino: u64) -> Option<Vec<(String, u64)>> {
        if let Some(TreeNode::Dir { children, .. }) = self.nodes.get(&ino) {
            let result: Vec<(String, u64)> = children
                .iter()
                .filter_map(|name| {
                    let child_ino = self.names.get(&(ino, name.clone()))?;
                    Some((name.clone(), *child_ino))
                })
                .collect();
            Some(result)
        } else {
            None
        }
    }

    pub fn build_from_roms(&mut self, platform_dirs: &HashMap<String, u64>, roms: &[SimpleRom]) {
        for rom in roms {
            let dir_ino = match platform_dirs.get(&rom.platform_slug) {
                Some(&ino) => ino,
                None => continue,
            };

            if rom.has_multiple_files && !rom.files.is_empty() {
                let folder_name = rom.fs_name_no_ext.clone();
                let folder_ino = self.allocate_inode();
                self.add_dir(dir_ino, folder_name, folder_ino);

                for file in &rom.files {
                    let file_ino = self.allocate_inode();
                    self.add_file(
                        folder_ino,
                        file.file_name.clone(),
                        file_ino,
                        rom.id,
                        Some(file.id),
                        file.file_name.clone(),
                        file.file_size_bytes,
                    );
                }
            } else {
                let file_ino = self.allocate_inode();
                let file_id = rom.files.first().map(|f| f.id);
                self.add_file(
                    dir_ino,
                    rom.fs_name.clone(),
                    file_ino,
                    rom.id,
                    file_id,
                    rom.fs_name.clone(),
                    rom.fs_size_bytes,
                );
            }
        }
    }
}
