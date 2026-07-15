pub mod builtins;

pub use builtins::get_builtin;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Profile {
    pub name: String,
    pub prefix: Option<String>,
    pub platforms: std::collections::HashMap<String, String>,
}
