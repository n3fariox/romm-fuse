#![allow(dead_code)]

use std::collections::HashMap;

pub struct InodeTable {
    pub ino_to_path: HashMap<u64, String>,
    pub path_to_ino: HashMap<String, u64>,
    next: u64,
}

impl InodeTable {
    pub fn new() -> Self {
        let mut table = Self {
            ino_to_path: HashMap::new(),
            path_to_ino: HashMap::new(),
            next: 2,
        };
        table.ino_to_path.insert(1, String::new());
        table.path_to_ino.insert(String::new(), 1);
        table
    }

    pub fn alloc(&mut self, path: &str) -> u64 {
        let ino = self.next;
        self.next += 1;
        self.ino_to_path.insert(ino, path.to_string());
        self.path_to_ino.insert(path.to_string(), ino);
        ino
    }

    pub fn get_path(&self, ino: u64) -> Option<&str> {
        self.ino_to_path.get(&ino).map(|s| s.as_str())
    }

    pub fn get_ino(&self, path: &str) -> Option<u64> {
        self.path_to_ino.get(path).copied()
    }
}
