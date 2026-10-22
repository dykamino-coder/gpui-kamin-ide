//! Shared byte and node allocation limits for one external-use session.
// KaminIDE patch: reservations cover source reads, XML, typed nodes and owned copies.
use super::*;
use super::super::super::{NODE_LIMIT, SOURCE_BYTE_LIMIT};

pub(super) struct Budget {
    pub(super) bytes_left: u64,
    pub(super) nodes_left: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            bytes_left: SOURCE_BYTE_LIMIT as u64,
            nodes_left: NODE_LIMIT,
        }
    }
}

impl Budget {
    pub(super) fn bytes(&mut self, count: u64) -> bool {
        if count > self.bytes_left {
            return false;
        }
        self.bytes_left -= count;
        true
    }

    pub(super) fn nodes(&mut self, count: usize) -> bool {
        if count > self.nodes_left {
            return false;
        }
        self.nodes_left -= count;
        true
    }

    pub(super) fn read_resource(&mut self, path: &std::path::Path) -> Option<Vec<u8>> {
        let file = std::fs::File::open(path).ok()?;
        let limit = RESOURCE_BYTE_LIMIT.min(self.bytes_left);
        if file.metadata().ok()?.len() > limit {
            return None;
        }
        // Reserve before reading. A failed/growing read consumes its allowance too.
        self.bytes_left -= limit;
        let bytes = read_bounded(file, limit)?;
        self.bytes_left += limit - bytes.len() as u64;
        Some(bytes)
    }

    pub(super) fn parse_xml<'a>(&mut self, text: &'a str) -> Option<roxmltree::Document<'a>> {
        let limit = std::mem::take(&mut self.nodes_left);
        let xml = roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                nodes_limit: limit.min(u32::MAX as usize) as u32,
                ..Default::default()
            },
        ).ok()?;
        self.nodes_left = limit.checked_sub(xml.descendants().count())?;
        Some(xml)
    }

    pub(super) fn parse_tree<'a>(&mut self, xml: &roxmltree::Document<'a>) -> Option<Document<'a>> {
        let limit = std::mem::take(&mut self.nodes_left);
        let doc = super::super::parse_with_limit(xml, None, true, limit).ok()?;
        self.nodes_left = limit.checked_sub(doc.nodes.len())?;
        Some(doc)
    }
}
