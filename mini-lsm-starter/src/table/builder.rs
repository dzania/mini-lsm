// Copyright (c) 2022-2025 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bytes::BufMut;

use super::{BlockMeta, SsTable};
use crate::{
    block::BlockBuilder,
    key::{KeyBytes, KeySlice},
    lsm_storage::BlockCache,
    table::FileObject,
};

/// Builds an SSTable from key-value pairs.
pub struct SsTableBuilder {
    builder: BlockBuilder,
    first_key: Vec<u8>,
    last_key: Vec<u8>,
    data: Vec<u8>,
    pub(crate) meta: Vec<BlockMeta>,
    block_size: usize,
}

impl SsTableBuilder {
    /// Create a builder based on target block size.
    pub fn new(block_size: usize) -> Self {
        Self {
            builder: BlockBuilder::new(block_size),
            first_key: Vec::new(),
            last_key: Vec::new(),
            data: Vec::new(),
            meta: Vec::new(),
            block_size,
        }
    }

    /// Finalizes current block and creates and cleans first/last keys
    fn finalize_current_block(&mut self) {
        let block = std::mem::replace(&mut self.builder, BlockBuilder::new(self.block_size));
        let block = block.build();
        let offset = self.estimated_size();
        self.data.extend_from_slice(&block.encode());
        let meta = BlockMeta {
            offset,
            first_key: KeyBytes::from_bytes(std::mem::take(&mut self.first_key).into()),
            last_key: KeyBytes::from_bytes(std::mem::take(&mut self.last_key).into()),
        };
        self.meta.push(meta);
    }

    /// Adds a key-value pair to SSTable.
    pub fn add(&mut self, key: KeySlice, value: &[u8]) {
        // Builder returns false when block is full
        if !self.builder.add(key, value) {
            self.finalize_current_block();
            // We just created new block the key will fit new
            let _ = self.builder.add(key, value);
        }
        if self.first_key.is_empty() {
            self.first_key = key.into_inner().to_vec();
        }
        self.last_key = key.into_inner().to_vec();
    }

    /// Get the estimated size of the SSTable.
    ///
    /// Since the data blocks contain much more data than meta blocks, just return the size of data
    /// blocks here.
    pub fn estimated_size(&self) -> usize {
        self.data.len()
    }

    /// Builds the SSTable and writes it to the given path. Use the `FileObject` structure to manipulate the disk objects.
    pub fn build(
        mut self,
        id: usize,
        block_cache: Option<Arc<BlockCache>>,
        path: impl AsRef<Path>,
    ) -> Result<SsTable> {
        self.finalize_current_block();
        let block_meta_offset = self.estimated_size();
        BlockMeta::encode_block_meta(&self.meta, &mut self.data);
        self.data.put_u32(block_meta_offset as u32);
        let file = FileObject::create(path.as_ref(), self.data)?;
        let first_key = self
            .meta
            .first()
            .map(|block| block.first_key.clone())
            .ok_or_else(|| anyhow::anyhow!("missing first key"))?;
        let last_key = self
            .meta
            .last()
            .map(|block| block.last_key.clone())
            .ok_or_else(|| anyhow::anyhow!("missing last key"))?;

        Ok(SsTable {
            id,
            file,
            block_meta_offset,
            block_meta: self.meta,
            first_key: first_key,
            last_key: last_key,
            block_cache,
            bloom: None,
            max_ts: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn build_for_test(self, path: impl AsRef<Path>) -> Result<SsTable> {
        self.build(0, None, path)
    }
}
