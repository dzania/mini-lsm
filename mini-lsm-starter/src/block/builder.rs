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

use bytes::BufMut;

use crate::key::{KeySlice, KeyVec};

use super::Block;

const SIZEOF_U16: usize = std::mem::size_of::<u16>();

/// Builds a block.
pub struct BlockBuilder {
    /// Offsets of each key-value entries.
    offsets: Vec<u16>,
    /// All serialized key-value pairs in the block.
    data: Vec<u8>,
    /// The expected block size.
    block_size: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl BlockBuilder {
    /// Creates a new block builder.
    pub fn new(block_size: usize) -> Self {
        Self {
            offsets: Vec::new(),
            data: Vec::new(),
            block_size,
            first_key: KeyVec::new(),
        }
    }

    /// Adds a key-value pair to the block. Returns false when the block is full.
    /// You may find the `bytes::BufMut` trait useful for manipulating binary data.
    #[must_use]
    pub fn add(&mut self, key: KeySlice, value: &[u8]) -> bool {
        let entry_len = SIZEOF_U16 + key.len() + SIZEOF_U16 + value.len();
        let estimated_size =
            entry_len + self.data.len() + self.offsets.len() * SIZEOF_U16 + SIZEOF_U16;

        if !self.is_empty() && estimated_size > self.block_size {
            return false;
        };
        self.offsets.push(self.data.len() as u16);

        self.data.put_u8(key.len() as u8);
        self.data.extend_from_slice(key.into_inner());

        self.data.put_u8(value.len() as u8);
        self.data.extend_from_slice(value);
        true
    }

    /// Check if there is no key-value pair in the block.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Finalize the block.
    pub fn build(self) -> Block {
        if self.is_empty() {
            panic!("Trying to finalize empty block.")
        };
        Block {
            data: self.data,
            offsets: self.offsets,
        }
    }
}
