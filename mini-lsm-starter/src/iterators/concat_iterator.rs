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

use anyhow::Result;

use super::StorageIterator;
use crate::{
    key::KeySlice,
    table::{SsTable, SsTableIterator},
};

/// Concat multiple iterators ordered in key order and their key ranges do not overlap. We do not want to create the
/// iterators when initializing this iterator to reduce the overhead of seeking.
pub struct SstConcatIterator {
    current: Option<SsTableIterator>,
    next_sst_idx: usize,
    sstables: Vec<Arc<SsTable>>,
}

impl SstConcatIterator {
    pub fn create_and_seek_to_first(sstables: Vec<Arc<SsTable>>) -> Result<Self> {
        let table = sstables.first().map(Arc::clone);
        let current = if let Some(table) = table {
            Some(SsTableIterator::create_and_seek_to_first(table)?)
        } else {
            None
        };

        Ok(Self {
            current,
            next_sst_idx: 1,
            sstables,
        })
    }

    pub fn create_and_seek_to_key(sstables: Vec<Arc<SsTable>>, key: KeySlice) -> Result<Self> {
        let candidate = sstables.partition_point(|sst| sst.last_key().as_key_slice() < key);

        let (current, next_sst_idx) = match sstables.get(candidate) {
            Some(table) => (
                Some(SsTableIterator::create_and_seek_to_key(
                    Arc::clone(table),
                    key,
                )?),
                candidate + 1,
            ),
            None => (None, sstables.len()),
        };

        Ok(Self {
            current,
            next_sst_idx,
            sstables,
        })
    }
}

impl StorageIterator for SstConcatIterator {
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice<'_> {
        self.current
            .as_ref()
            .expect("key() requires an active SST iterator")
            .key()
    }

    fn value(&self) -> &[u8] {
        self.current
            .as_ref()
            .expect("value() requires an active SST iterator")
            .value()
    }

    fn is_valid(&self) -> bool {
        match &self.current {
            Some(iter) => iter.is_valid(),
            None => false,
        }
    }

    fn next(&mut self) -> Result<()> {
        let Some(current) = self.current.as_mut() else {
            return Ok(());
        };

        current.next()?;

        if !current.is_valid() {
            self.current = match self.sstables.get(self.next_sst_idx) {
                Some(table) => {
                    let iter = SsTableIterator::create_and_seek_to_first(Arc::clone(table))?;
                    self.next_sst_idx += 1;
                    Some(iter)
                }
                None => None,
            };
        }

        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        1
    }
}
