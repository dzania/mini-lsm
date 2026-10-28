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

use anyhow::Result;

use super::StorageIterator;

#[derive(PartialEq)]
enum CurrentIterator {
    A,
    B,
    Exhausted, // Both iterators exhausted
}

/// Merges two iterators of different types into one. If the two iterators have the same key, only
/// produce the key once and prefer the entry from A.
pub struct TwoMergeIterator<A: StorageIterator, B: StorageIterator> {
    a: A,
    b: B,
    current: CurrentIterator,
}

impl<
    A: 'static + StorageIterator,
    B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
> TwoMergeIterator<A, B>
{
    pub fn create(a: A, b: B) -> Result<Self> {
        let mut iter = Self {
            a,
            b,
            current: CurrentIterator::A,
        };
        iter.set_current();
        Ok(iter)
    }

    fn set_current(&mut self) {
        match (self.a.is_valid(), self.b.is_valid()) {
            (false, false) => self.current = CurrentIterator::Exhausted,
            (true, false) => self.current = CurrentIterator::A,
            (false, true) => self.current = CurrentIterator::B,
            (true, true) => {
                if self.a.key() <= self.b.key() {
                    self.current = CurrentIterator::A
                } else {
                    self.current = CurrentIterator::B
                }
            }
        }
    }
}

impl<
    A: 'static + StorageIterator,
    B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
> StorageIterator for TwoMergeIterator<A, B>
{
    type KeyType<'a> = A::KeyType<'a>;

    fn key(&self) -> Self::KeyType<'_> {
        match self.current {
            CurrentIterator::A => self.a.key(),
            CurrentIterator::B => self.b.key(),
            CurrentIterator::Exhausted => panic!("key() called on exhausted iterator"),
        }
    }

    fn value(&self) -> &[u8] {
        match self.current {
            CurrentIterator::A => self.a.value(),
            CurrentIterator::B => self.b.value(),
            CurrentIterator::Exhausted => panic!("value() called on exhausted iterator"),
        }
    }

    fn is_valid(&self) -> bool {
        self.current != CurrentIterator::Exhausted
    }

    fn next(&mut self) -> Result<()> {
        if self.current == CurrentIterator::Exhausted {
            return Ok(());
        }

        if self.a.is_valid() && self.b.is_valid() && self.a.key() == self.b.key() {
            self.b.next()?;
        };

        match self.current {
            CurrentIterator::A => self.a.next()?,
            CurrentIterator::B => self.b.next()?,
            CurrentIterator::Exhausted => (),
        }
        self.set_current();
        Ok(())
    }
}
