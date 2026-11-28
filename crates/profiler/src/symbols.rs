//! Symbolization outside allocator hooks. A caller supplies the exact loaded
//! images and bases; search never downloads symbols or guesses another build.

use kamin_heap::live::Stack;
use std::{collections::BTreeMap, path::PathBuf};

#[cfg(windows)]
mod windows;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Module {
    pub image: PathBuf,
    pub base: u64,
}

pub struct Symbols {
    names: BTreeMap<usize, Vec<String>>,
    #[cfg(windows)]
    platform: windows::Symbols,
}

impl Symbols {
    pub fn open(modules: &[Module]) -> Result<Self, String> {
        #[cfg(not(windows))]
        let _ = modules;
        Ok(Self {
            names: BTreeMap::new(),
            #[cfg(windows)]
            platform: windows::Symbols::open(modules)?,
        })
    }

    pub fn stack(&mut self, stack: &Stack) -> Vec<String> {
        let mut names = Vec::new();
        for &address in stack
            .addresses
            .iter()
            .take(usize::from(stack.depth).min(stack.addresses.len()))
        {
            if self.names.len() >= 16_384 {
                self.names.clear();
            }
            let frames = self.names.entry(address).or_insert_with(|| {
                #[cfg(windows)]
                {
                    self.platform.frames(address)
                }
                #[cfg(not(windows))]
                {
                    vec![format!("? 0x{address:x}")]
                }
            });
            names.extend(frames.iter().cloned());
        }
        names
    }
}

fn strip_hash(name: String) -> String {
    if let Some((prefix, hash)) = name.rsplit_once("::h") {
        if hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return prefix.to_owned();
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_rust_symbol_hash_suffix_is_removed() {
        assert_eq!(strip_hash("a::b::h0123456789abcdef".into()), "a::b");
        assert_eq!(strip_hash("a::h123<Thing>".into()), "a::h123<Thing>");
    }
}
