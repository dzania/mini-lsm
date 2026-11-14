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

use serde::{Deserialize, Serialize};

use crate::lsm_storage::LsmStorageState;

#[derive(Debug, Clone)]
pub struct SimpleLeveledCompactionOptions {
    pub size_ratio_percent: usize,
    pub level0_file_num_compaction_trigger: usize,
    pub max_levels: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleLeveledCompactionTask {
    // if upper_level is `None`, then it is L0 compaction
    pub upper_level: Option<usize>,
    pub upper_level_sst_ids: Vec<usize>,
    pub lower_level: usize,
    pub lower_level_sst_ids: Vec<usize>,
    pub is_lower_level_bottom_level: bool,
}

pub struct SimpleLeveledCompactionController {
    options: SimpleLeveledCompactionOptions,
}

impl SimpleLeveledCompactionController {
    pub fn new(options: SimpleLeveledCompactionOptions) -> Self {
        Self { options }
    }

    /// Generates a compaction task.
    ///
    /// Returns `None` if no compaction needs to be scheduled. The order of SSTs in the compaction task id vector matters.
    pub fn generate_compaction_task(
        &self,
        snapshot: &LsmStorageState,
    ) -> Option<SimpleLeveledCompactionTask> {
        if snapshot.l0_sstables.len() >= self.options.level0_file_num_compaction_trigger {
            let l1_sstables = snapshot
                .levels
                .first()
                .expect("L1 SSTs not found for compaction")
                .1
                .clone();
            return Some(SimpleLeveledCompactionTask {
                upper_level: None,
                upper_level_sst_ids: snapshot.l0_sstables.clone(),
                lower_level: 1,
                lower_level_sst_ids: l1_sstables,
                is_lower_level_bottom_level: snapshot.levels.len() == 1,
            });
        }

        for pair in snapshot.levels.windows(2) {
            let (upper_level, upper_ids) = &pair[0];
            let (lower_level, lower_ids) = &pair[1];
            // Division by 0
            if upper_ids.is_empty() {
                continue;
            };
            let ratio = lower_ids.len() as f64 / upper_ids.len() as f64 * 100.0;
            if ratio < self.options.size_ratio_percent as f64 {
                return Some(SimpleLeveledCompactionTask {
                    upper_level: Some(*upper_level),
                    upper_level_sst_ids: upper_ids.to_vec(),
                    lower_level: *lower_level,
                    lower_level_sst_ids: lower_ids.to_vec(),
                    is_lower_level_bottom_level: *lower_level == self.options.max_levels,
                });
            }
        }

        None
    }

    /// Apply the compaction result.
    ///
    /// The compactor will call this function with the compaction task and the list of SST ids generated. This function applies the
    /// result and generates a new LSM state. The functions should only change `l0_sstables` and `levels` without changing memtables
    /// and `sstables` hash map. Though there should only be one thread running compaction jobs, you should think about the case
    /// where an L0 SST gets flushed while the compactor generates new SSTs, and with that in mind, you should do some sanity checks
    /// in your implementation.
    pub fn apply_compaction_result(
        &self,
        snapshot: &LsmStorageState,
        task: &SimpleLeveledCompactionTask,
        output: &[usize],
    ) -> (LsmStorageState, Vec<usize>) {
        let mut new_state = snapshot.clone();
        match task.upper_level {
            None => {
                assert!(
                    task.upper_level_sst_ids
                        .iter()
                        .all(|id| snapshot.l0_sstables.contains(id)),
                    "compaction inputs must still exist in L0"
                );
                new_state
                    .l0_sstables
                    .retain(|id| !task.upper_level_sst_ids.contains(id));
            }
            Some(level) => {
                assert_eq!(
                    snapshot.levels[level - 1].1,
                    task.upper_level_sst_ids,
                    "upper-level files must match compaction inputs"
                );
                new_state.levels[level - 1].1.clear();
            }
        }

        assert_eq!(
            snapshot.levels[task.lower_level - 1].1,
            task.lower_level_sst_ids,
            "lower-level files must match compaction inputs"
        );
        new_state.levels[task.lower_level - 1].1 = output.to_vec();

        let obsolete_ids = task
            .upper_level_sst_ids
            .iter()
            .chain(&task.lower_level_sst_ids)
            .copied()
            .collect();
        // L0
        (new_state, obsolete_ids)
    }
}
