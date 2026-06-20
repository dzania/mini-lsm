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

use std::sync::Arc;

use bytes::Buf;

use crate::key::{KeySlice, KeyVec};

use super::Block;

/// Iterates on a block.
pub struct BlockIterator {
    /// The internal `Block`, wrapped by an `Arc`
    block: Arc<Block>,
    /// The current key, empty represents the iterator is invalid
    key: KeyVec,
    /// the current value range in the block.data, corresponds to the current key
    value_range: (usize, usize),
    /// Current index of the key-value pair, should be in range of [0, num_of_elements)
    idx: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl BlockIterator {
    fn new(block: Arc<Block>) -> Self {
        Self {
            first_key: block.get_first_key(),
            block,
            key: KeyVec::new(),
            value_range: (0, 0),
            idx: 0,
        }
    }

    /// Creates a block iterator and seek to the first entry.
    pub fn create_and_seek_to_first(block: Arc<Block>) -> Self {
        let mut iter = Self::new(block);
        iter.seek_to_first();
        iter
    }

    /// Creates a block iterator and seek to the first key that >= `key`.
    pub fn create_and_seek_to_key(block: Arc<Block>, key: KeySlice) -> Self {
        let mut iter = Self::new(block);
        iter.seek_to_key(key);
        iter
    }

    /// Returns the key of the current entry.
    pub fn key(&self) -> KeySlice<'_> {
        self.key.as_key_slice()
    }

    /// Returns the value of the current entry.
    pub fn value(&self) -> &[u8] {
        debug_assert!(!self.key.is_empty(), "invalid iterator");
        &self.block.data[self.value_range.0..self.value_range.1]
    }

    /// Returns true if the iterator is valid.
    pub fn is_valid(&self) -> bool {
        !self.key().is_empty()
    }

    /// Seeks to the first key in the block.
    pub fn seek_to_first(&mut self) {
        self.seek_to_offset(0);
    }

    pub fn seek_to_offset(&mut self, idx: usize) {
        self.idx = idx;
        let offset = self.block.offsets[idx] as usize;

        let mut entry = &self.block.data[offset..];

        let key_len = entry.get_u8() as usize;
        let key = KeyVec::from_vec(entry[..key_len].to_vec());
        self.key = key;
        entry.advance(key_len);
        let value_len = entry.get_u8() as usize;
        let value_start = offset + 1 + key_len + 1;
        self.value_range = (value_start, value_start + value_len);
        entry.advance(value_len);
    }

    /// Move to the next key in the block.
    pub fn next(&mut self) {
        let next_idx = self.idx + 1;
        if next_idx >= self.block.offsets.len() {
            self.key = KeyVec::default();
            self.value_range = (0, 0);
            return;
        }
        self.seek_to_offset(next_idx);
    }

    /// Seek to the first key that >= `key`.
    /// Note: You should assume the key-value pairs in the block are sorted when being added by
    /// callers.
    pub fn seek_to_key(&mut self, key: KeySlice) {
        let mut left = 0;
        let mut right = self.block.offsets.len();
        while left < right {
            let mid = (left + right) / 2;
            self.seek_to_offset(mid);
            debug_assert!(self.is_valid());
            match self.key.as_key_slice().cmp(&key) {
                std::cmp::Ordering::Greater => right = mid,
                std::cmp::Ordering::Less => left = mid + 1,
                std::cmp::Ordering::Equal => return,
            }
        }
        if left == self.block.offsets.len() {
            self.key = KeyVec::new();
            return;
        }
        self.seek_to_offset(left);
    }
}
