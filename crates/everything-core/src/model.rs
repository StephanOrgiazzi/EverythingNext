use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortColumn {
    Name,
    Path,
    Extension,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortSpec {
    pub column: SortColumn,
    pub direction: SortDirection,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            column: SortColumn::Name,
            direction: SortDirection::Ascending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub query: String,
    pub offset: u32,
    pub limit: u32,
    pub sort: SortSpec,
    pub request_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionRange {
    pub start: u32,
    pub end: u32,
}

impl SelectionRange {
    pub fn new(start: u32, end: u32) -> Self {
        Self {
            start: start.min(end),
            end: start.max(end),
        }
    }

    #[allow(
        clippy::len_without_is_empty,
        reason = "inclusive selection ranges always contain at least one index"
    )]
    pub fn len(self) -> u64 {
        u64::from(self.end) - u64::from(self.start) + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionRequest {
    pub query: String,
    pub sort: SortSpec,
    pub request_id: u32,
    pub ranges: Vec<SelectionRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrashPreparation {
    pub snapshot_id: u64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrashOutcome {
    pub deleted: usize,
    pub deleted_paths: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexSelection {
    ranges: Vec<SelectionRange>,
}

impl IndexSelection {
    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn contains(&self, index: u32) -> bool {
        let position = self.ranges.partition_point(|range| range.end < index);
        self.ranges
            .get(position)
            .is_some_and(|range| range.start <= index)
    }

    pub fn count(&self) -> u64 {
        self.ranges.iter().copied().map(SelectionRange::len).sum()
    }

    #[must_use]
    pub fn first(&self) -> Option<u32> {
        self.ranges.first().map(|range| range.start)
    }

    pub fn ranges(&self) -> Vec<SelectionRange> {
        self.ranges.clone()
    }

    pub fn select_only(&mut self, index: u32) {
        self.ranges = vec![SelectionRange::new(index, index)];
    }

    pub fn select_range(&mut self, anchor: u32, target: u32) {
        self.ranges = vec![SelectionRange::new(anchor, target)];
    }

    pub fn select_all(&mut self, total: u32) {
        if total == 0 {
            self.clear();
        } else {
            self.ranges = vec![SelectionRange::new(0, total - 1)];
        }
    }

    pub fn add_range(&mut self, anchor: u32, target: u32) {
        let added = SelectionRange::new(anchor, target);
        let mut merged = Vec::with_capacity(self.ranges.len() + 1);
        let mut current = added;
        let mut inserted = false;

        for range in self.ranges.iter().copied() {
            if range.end.saturating_add(1) < current.start {
                merged.push(range);
            } else if current.end.saturating_add(1) < range.start {
                if !inserted {
                    merged.push(current);
                    inserted = true;
                }
                merged.push(range);
            } else {
                current.start = current.start.min(range.start);
                current.end = current.end.max(range.end);
            }
        }

        if !inserted {
            merged.push(current);
        }
        self.ranges = merged;
    }

    pub fn add_ranges(&mut self, ranges: impl IntoIterator<Item = SelectionRange>) {
        let mut ranges = ranges.into_iter();
        let Some(first) = ranges.next() else {
            return;
        };
        let mut combined = std::mem::take(&mut self.ranges);
        combined.push(SelectionRange::new(first.start, first.end));
        combined.extend(ranges.map(|range| SelectionRange::new(range.start, range.end)));
        combined.sort_unstable_by_key(|range| range.start);

        let mut merged: Vec<SelectionRange> = Vec::with_capacity(combined.len());
        for range in combined {
            if let Some(previous) = merged.last_mut() {
                if range.start <= previous.end.saturating_add(1) {
                    previous.end = previous.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        self.ranges = merged;
    }

    pub fn toggle(&mut self, index: u32) {
        if let Some(position) = self
            .ranges
            .iter()
            .position(|range| range.start <= index && index <= range.end)
        {
            let range = self.ranges.remove(position);
            if range.start < index {
                self.ranges
                    .insert(position, SelectionRange::new(range.start, index - 1));
            }
            if index < range.end {
                let insertion = if range.start < index {
                    position + 1
                } else {
                    position
                };
                self.ranges
                    .insert(insertion, SelectionRange::new(index + 1, range.end));
            }
        } else {
            self.add_range(index, index);
        }
    }
}

#[cfg(test)]
mod selection_tests {
    use super::IndexSelection;

    #[test]
    fn merges_adjacent_ranges() {
        let mut selection = IndexSelection::default();
        selection.add_range(4, 8);
        selection.add_range(9, 12);
        selection.add_range(1, 3);
        assert_eq!(selection.count(), 12);
        assert_eq!(selection.ranges().len(), 1);
    }

    #[test]
    fn toggling_splits_a_range() {
        let mut selection = IndexSelection::default();
        selection.select_range(2, 6);
        selection.toggle(4);
        assert!(selection.contains(2));
        assert!(!selection.contains(4));
        assert!(selection.contains(6));
        assert_eq!(selection.count(), 4);
        assert_eq!(selection.ranges().len(), 2);
    }

    #[test]
    fn mixed_range_operations_match_an_explicit_set() {
        use std::collections::BTreeSet;

        let mut selection = IndexSelection::default();
        let mut expected = BTreeSet::new();
        for (start, end) in [(20, 25), (2, 7), (8, 12), (40, 35)] {
            selection.add_range(start, end);
            for index in start.min(end)..=start.max(end) {
                expected.insert(index);
            }
        }
        for index in [2, 5, 11, 18, 37, 50] {
            selection.toggle(index);
            if !expected.remove(&index) {
                expected.insert(index);
            }
        }

        assert_eq!(
            selection.count(),
            u64::try_from(expected.len()).expect("test set length fits into u64")
        );
        for index in 0..=55 {
            assert_eq!(
                selection.contains(index),
                expected.contains(&index),
                "{index}"
            );
        }
    }

    #[test]
    fn select_all_is_constant_memory() {
        let mut selection = IndexSelection::default();
        selection.select_all(500_000);
        assert_eq!(selection.count(), 500_000);
        assert_eq!(selection.ranges().len(), 1);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub parent_path: String,
    pub full_path: String,
    pub size: Option<u64>,
    pub modified_unix: Option<i64>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub request_id: u32,
    pub offset: u32,
    pub total: u32,
    pub items: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub available: bool,
    pub indexing: bool,
    pub ready_volumes: u32,
    pub total_volumes: u32,
    pub message: String,
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{SelectionRange, SortDirection, SortSpec};

    #[test]
    fn sort_direction_toggles() {
        assert_eq!(SortDirection::Ascending.toggle(), SortDirection::Descending);
        assert_eq!(SortDirection::Descending.toggle(), SortDirection::Ascending);
    }

    #[test]
    fn default_sort_is_name_ascending() {
        let sort = SortSpec::default();
        assert_eq!(sort.direction, SortDirection::Ascending);
    }

    #[test]
    fn selection_ranges_are_normalized() {
        let range = SelectionRange::new(12, 4);
        assert_eq!(range.start, 4);
        assert_eq!(range.end, 12);
        assert_eq!(range.len(), 9);
    }
}
