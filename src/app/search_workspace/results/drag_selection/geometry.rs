use super::super::numeric::floor_to_u32;
use super::super::view_modes::{ViewMode, GRID_GAP, GRID_PADDING};
use everything_core::{IndexSelection, SelectionRange};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app::search_workspace::results) struct DragSelectionRect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl DragSelectionRect {
    pub fn between(first: (f64, f64), second: (f64, f64)) -> Self {
        Self {
            left: first.0.min(second.0),
            top: first.1.min(second.1),
            right: first.0.max(second.0),
            bottom: first.1.max(second.1),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }

    pub fn style(self) -> String {
        format!(
            "left: {}px; top: {}px; width: {}px; height: {}px",
            self.left,
            self.top,
            self.right - self.left,
            self.bottom - self.top,
        )
    }
}

#[derive(Clone, Copy)]
pub(super) struct SelectionLayout {
    pub mode: ViewMode,
    pub columns: u32,
    pub width: f64,
}

pub(super) fn selection_in_rectangle(
    rectangle: DragSelectionRect,
    total: u32,
    layout: SelectionLayout,
    baseline: &IndexSelection,
    additive: bool,
) -> IndexSelection {
    let mut selection = if additive {
        baseline.clone()
    } else {
        IndexSelection::default()
    };
    if total == 0 {
        return selection;
    }

    let ranges = if layout.mode == ViewMode::Details {
        detail_ranges(rectangle, total, layout.width, layout.mode.item_height())
    } else {
        grid_ranges(
            rectangle,
            total,
            layout.mode,
            layout.columns.max(1),
            layout.width,
        )
    };
    selection.add_ranges(ranges);
    selection
}

fn detail_ranges(
    rectangle: DragSelectionRect,
    total: u32,
    width: f64,
    row_height: f64,
) -> Vec<SelectionRange> {
    if rectangle.right <= 0.0 || rectangle.left >= width {
        return Vec::new();
    }

    let first = nonnegative_index(rectangle.top / row_height);
    if first >= total {
        return Vec::new();
    }
    let mut last = nonnegative_index(rectangle.bottom / row_height).min(total - 1);
    let last_row = item_rectangle(last, 0.0, width, row_height);
    if !rectangle.intersects(last_row) {
        if last == 0 {
            return Vec::new();
        }
        last -= 1;
    }
    (first <= last)
        .then(|| SelectionRange::new(first, last))
        .into_iter()
        .collect()
}

fn grid_ranges(
    rectangle: DragSelectionRect,
    total: u32,
    mode: ViewMode,
    columns: u32,
    width: f64,
) -> Vec<SelectionRange> {
    let item_height = mode.item_height();
    let cell_width = ((width - GRID_PADDING * 2.0 - GRID_GAP * f64::from(columns - 1))
        / f64::from(columns))
    .max(mode.min_width());
    let selected_columns = intersected_columns(rectangle, columns, cell_width);
    let Some((first_column, last_column)) = selected_columns else {
        return Vec::new();
    };

    let total_rows = total.div_ceil(columns);
    let first_row = nonnegative_index((rectangle.top - GRID_PADDING) / item_height);
    if first_row >= total_rows {
        return Vec::new();
    }
    let last_row =
        nonnegative_index((rectangle.bottom - GRID_PADDING) / item_height).min(total_rows - 1);
    let mut ranges = Vec::new();
    for row in first_row..=last_row {
        let top = GRID_PADDING + f64::from(row) * item_height;
        if rectangle.top >= top + item_height - GRID_GAP || rectangle.bottom <= top {
            continue;
        }

        let start = row * columns + first_column;
        let end = (row * columns + last_column).min(total - 1);
        if start <= end {
            ranges.push(SelectionRange::new(start, end));
        }
    }
    ranges
}

fn intersected_columns(
    rectangle: DragSelectionRect,
    columns: u32,
    cell_width: f64,
) -> Option<(u32, u32)> {
    let mut first = None;
    let mut last = None;
    for column in 0..columns {
        let left = GRID_PADDING + f64::from(column) * (cell_width + GRID_GAP);
        if rectangle.left < left + cell_width && rectangle.right > left {
            first.get_or_insert(column);
            last = Some(column);
        }
    }
    first.zip(last)
}

fn item_rectangle(index: u32, left: f64, right: f64, height: f64) -> DragSelectionRect {
    DragSelectionRect {
        left,
        top: f64::from(index) * height,
        right,
        bottom: f64::from(index + 1) * height,
    }
}

fn nonnegative_index(value: f64) -> u32 {
    floor_to_u32(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> DragSelectionRect {
        DragSelectionRect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn rectangle_is_normalized_in_every_drag_direction() {
        assert_eq!(
            DragSelectionRect::between((80.0, 90.0), (20.0, 30.0)),
            rectangle(20.0, 30.0, 80.0, 90.0),
        );
    }

    #[test]
    fn details_select_every_intersected_row() {
        let selection = selection_in_rectangle(
            rectangle(20.0, 30.0, 100.0, 70.0),
            10,
            SelectionLayout {
                mode: ViewMode::Details,
                columns: 1,
                width: 600.0,
            },
            &IndexSelection::default(),
            false,
        );

        assert_eq!(selection.count(), 3);
        assert!(selection.contains(0));
        assert!(selection.contains(1));
        assert!(selection.contains(2));
    }

    #[test]
    fn grid_selection_respects_tile_gaps() {
        let selection = selection_in_rectangle(
            rectangle(138.0, 20.0, 250.0, 130.0),
            12,
            SelectionLayout {
                mode: ViewMode::Medium,
                columns: 4,
                width: 520.0,
            },
            &IndexSelection::default(),
            false,
        );

        assert_eq!(selection.count(), 1);
        assert!(selection.contains(1));
    }

    #[test]
    fn control_drag_adds_to_the_existing_selection() {
        let mut baseline = IndexSelection::default();
        baseline.select_only(8);
        let selection = selection_in_rectangle(
            rectangle(20.0, 0.0, 100.0, 34.0),
            10,
            SelectionLayout {
                mode: ViewMode::Details,
                columns: 1,
                width: 600.0,
            },
            &baseline,
            true,
        );

        assert_eq!(selection.count(), 2);
        assert!(selection.contains(0));
        assert!(selection.contains(8));
    }
}
