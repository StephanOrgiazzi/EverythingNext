use crate::{
    EngineError, EngineStatus, QueryRequest, SearchPage, SearchResult, SelectionRequest,
    SortColumn, SortDirection,
};
use libloading::Library;
use std::env;
use std::path::{Path, PathBuf};

const REQUEST_FILE_NAME: u32 = 0x0000_0001;
const REQUEST_PATH: u32 = 0x0000_0002;
const REQUEST_SIZE: u32 = 0x0000_0010;
const REQUEST_DATE_MODIFIED: u32 = 0x0000_0040;

const SORT_NAME_ASC: u32 = 1;
const SORT_NAME_DESC: u32 = 2;
const SORT_PATH_ASC: u32 = 3;
const SORT_PATH_DESC: u32 = 4;
const SORT_SIZE_ASC: u32 = 5;
const SORT_SIZE_DESC: u32 = 6;
const SORT_DATE_MODIFIED_ASC: u32 = 13;
const SORT_DATE_MODIFIED_DESC: u32 = 14;

const WINDOWS_TO_UNIX_SECONDS: u64 = 11_644_473_600;

type SetSearchW = unsafe extern "system" fn(*const u16);
type SetRequestFlags = unsafe extern "system" fn(u32);
type SetMax = unsafe extern "system" fn(u32);
type SetOffset = unsafe extern "system" fn(u32);
type SetSort = unsafe extern "system" fn(u32);
type QueryW = unsafe extern "system" fn(i32) -> i32;
type GetU32 = unsafe extern "system" fn() -> u32;
type GetResultStringW = unsafe extern "system" fn(u32) -> *const u16;
type GetResultU64 = unsafe extern "system" fn(u32, *mut u64) -> i32;
type IsFolderResult = unsafe extern "system" fn(u32) -> i32;

pub struct EverythingSdk {
    _library: Library,
    set_search_w: SetSearchW,
    set_request_flags: SetRequestFlags,
    set_max: SetMax,
    set_offset: SetOffset,
    set_sort: SetSort,
    query_w: QueryW,
    get_num_results: GetU32,
    get_tot_results: GetU32,
    get_last_error: GetU32,
    get_result_file_name_w: GetResultStringW,
    get_result_path_w: GetResultStringW,
    get_result_size: GetResultU64,
    get_result_date_modified: GetResultU64,
    is_folder_result: IsFolderResult,
    get_major_version: GetU32,
    get_minor_version: GetU32,
    get_revision: GetU32,
    get_build_number: GetU32,
    is_db_loaded: unsafe extern "system" fn() -> i32,
}

unsafe impl Send for EverythingSdk {}

impl EverythingSdk {
    pub fn load() -> Result<Self, EngineError> {
        let path = locate_sdk().ok_or(EngineError::SdkNotFound)?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self, EngineError> {
        if !path.is_file() {
            return Err(EngineError::SdkNotFound);
        }
        let library = unsafe { Library::new(path) }
            .map_err(|error| EngineError::SdkLoad(format!("{} ({})", error, path.display())))?;

        macro_rules! symbol {
            ($name:literal, $ty:ty) => {{
                let symbol = unsafe { library.get::<$ty>(concat!($name, "\0").as_bytes()) }
                    .map_err(|error| EngineError::SdkLoad(format!("{}: {}", $name, error)))?;
                *symbol
            }};
        }

        Ok(Self {
            set_search_w: symbol!("Everything_SetSearchW", SetSearchW),
            set_request_flags: symbol!("Everything_SetRequestFlags", SetRequestFlags),
            set_max: symbol!("Everything_SetMax", SetMax),
            set_offset: symbol!("Everything_SetOffset", SetOffset),
            set_sort: symbol!("Everything_SetSort", SetSort),
            query_w: symbol!("Everything_QueryW", QueryW),
            get_num_results: symbol!("Everything_GetNumResults", GetU32),
            get_tot_results: symbol!("Everything_GetTotResults", GetU32),
            get_last_error: symbol!("Everything_GetLastError", GetU32),
            get_result_file_name_w: symbol!("Everything_GetResultFileNameW", GetResultStringW),
            get_result_path_w: symbol!("Everything_GetResultPathW", GetResultStringW),
            get_result_size: symbol!("Everything_GetResultSize", GetResultU64),
            get_result_date_modified: symbol!("Everything_GetResultDateModified", GetResultU64),
            is_folder_result: symbol!("Everything_IsFolderResult", IsFolderResult),
            get_major_version: symbol!("Everything_GetMajorVersion", GetU32),
            get_minor_version: symbol!("Everything_GetMinorVersion", GetU32),
            get_revision: symbol!("Everything_GetRevision", GetU32),
            get_build_number: symbol!("Everything_GetBuildNumber", GetU32),
            is_db_loaded: symbol!("Everything_IsDBLoaded", unsafe extern "system" fn() -> i32),
            _library: library,
        })
    }

    pub fn status(&self) -> EngineStatus {
        let loaded = unsafe { (self.is_db_loaded)() != 0 };
        let version = unsafe {
            format!(
                "{}.{}.{}.{}",
                (self.get_major_version)(),
                (self.get_minor_version)(),
                (self.get_revision)(),
                (self.get_build_number)(),
            )
        };
        EngineStatus {
            available: loaded,
            message: if loaded {
                format!("Everything {version} connecté via le SDK IPC.")
            } else {
                "SDK chargé, mais la base Everything n’est pas disponible. Lancez Everything.".into()
            },
            version: Some(version),
        }
    }

    pub fn query(&mut self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        let search = to_wide(&request.query);
        let sort = map_sort(request.sort.column, request.sort.direction);
        unsafe {
            (self.set_search_w)(search.as_ptr());
            (self.set_request_flags)(REQUEST_FILE_NAME | REQUEST_PATH | REQUEST_SIZE | REQUEST_DATE_MODIFIED);
            (self.set_offset)(request.offset);
            (self.set_max)(request.limit.clamp(1, 4096));
            (self.set_sort)(sort);
            if (self.query_w)(1) == 0 {
                return Err(EngineError::QueryFailed((self.get_last_error)()));
            }
        }

        let count = unsafe { (self.get_num_results)() };
        let total = unsafe { (self.get_tot_results)() };
        let mut items = Vec::with_capacity(count as usize);

        for index in 0..count {
            let name = unsafe { wide_ptr_to_string((self.get_result_file_name_w)(index)) };
            let parent_path = unsafe { wide_ptr_to_string((self.get_result_path_w)(index)) };
            let full_path = if parent_path.ends_with('\\') || parent_path.ends_with('/') {
                format!("{parent_path}{name}")
            } else if parent_path.is_empty() {
                name.clone()
            } else {
                format!("{parent_path}\\{name}")
            };
            let is_dir = unsafe { (self.is_folder_result)(index) != 0 };
            let mut raw_size = 0_u64;
            let size = unsafe { ((self.get_result_size)(index, &mut raw_size) != 0).then_some(raw_size) };
            let mut raw_date = 0_u64;
            let modified_unix = unsafe {
                ((self.get_result_date_modified)(index, &mut raw_date) != 0)
                    .then(|| filetime_to_unix(raw_date))
                    .flatten()
            };

            items.push(SearchResult {
                id: stable_id(&full_path),
                name,
                parent_path,
                full_path,
                size: if is_dir { None } else { size },
                modified_unix,
                is_dir,
            });
        }

        Ok(SearchPage {
            request_id: request.request_id,
            offset: request.offset,
            total,
            items,
        })
    }

    pub fn resolve_selection_cancellable<F>(
        &mut self,
        request: SelectionRequest,
        max_items: usize,
        mut is_cancelled: F,
    ) -> Result<Vec<String>, EngineError>
    where
        F: FnMut() -> bool,
    {
        let ranges = normalize_selection_ranges(request.ranges);
        let requested = ranges
            .iter()
            .copied()
            .map(|range| range.len())
            .sum::<u64>();
        if requested > max_items as u64 {
            return Err(EngineError::InvalidSelection(format!(
                "Cette opération est limitée à {max_items} éléments à la fois"
            )));
        }
        let mut paths = Vec::with_capacity(requested as usize);

        for range in ranges {
            let mut offset = range.start;
            loop {
                if is_cancelled() {
                    return Err(EngineError::InvalidSelection(
                        "La recherche a changé pendant la préparation".into(),
                    ));
                }
                let remaining = u64::from(range.end) - u64::from(offset) + 1;
                let limit = remaining.min(1_024) as u32;
                let page = self.query(QueryRequest {
                    query: request.query.clone(),
                    offset,
                    limit,
                    sort: request.sort,
                    request_id: request.request_id,
                })?;
                let returned = page.items.len() as u32;
                if returned == 0 {
                    break;
                }
                paths.extend(page.items.into_iter().map(|item| item.full_path));
                if returned < limit || u64::from(returned) >= remaining {
                    break;
                }
                offset = offset.saturating_add(returned);
            }
        }

        Ok(paths)
    }
}

fn normalize_selection_ranges(
    mut ranges: Vec<crate::SelectionRange>,
) -> Vec<crate::SelectionRange> {
    for range in &mut ranges {
        *range = crate::SelectionRange::new(range.start, range.end);
    }
    ranges.sort_unstable_by_key(|range| range.start);

    let mut normalized: Vec<crate::SelectionRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = normalized.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                last.end = last.end.max(range.end);
                continue;
            }
        }
        normalized.push(range);
    }
    normalized
}

fn locate_sdk() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("EVERYTHING_SDK_DLL") {
        let path = PathBuf::from(explicit);
        if path.is_file() { return Some(path); }
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("Everything64.dll"));
            candidates.push(directory.join("Everything32.dll"));
        }
    }
    if let Ok(current) = env::current_dir() {
        candidates.push(current.join("Everything64.dll"));
        candidates.push(current.join("src-tauri").join("Everything64.dll"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn wide_ptr_to_string(pointer: *const u16) -> String {
    if pointer.is_null() { return String::new(); }
    let mut length = 0;
    while *pointer.add(length) != 0 { length += 1; }
    String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length))
}

fn map_sort(column: SortColumn, direction: SortDirection) -> u32 {
    match (column, direction) {
        (SortColumn::Name, SortDirection::Ascending) => SORT_NAME_ASC,
        (SortColumn::Name, SortDirection::Descending) => SORT_NAME_DESC,
        (SortColumn::Path, SortDirection::Ascending) => SORT_PATH_ASC,
        (SortColumn::Path, SortDirection::Descending) => SORT_PATH_DESC,
        (SortColumn::Size, SortDirection::Ascending) => SORT_SIZE_ASC,
        (SortColumn::Size, SortDirection::Descending) => SORT_SIZE_DESC,
        (SortColumn::Modified, SortDirection::Ascending) => SORT_DATE_MODIFIED_ASC,
        (SortColumn::Modified, SortDirection::Descending) => SORT_DATE_MODIFIED_DESC,
    }
}

fn filetime_to_unix(filetime: u64) -> Option<i64> {
    let seconds = filetime / 10_000_000;
    (seconds >= WINDOWS_TO_UNIX_SECONDS).then_some((seconds - WINDOWS_TO_UNIX_SECONDS) as i64)
}

fn stable_id(path: &str) -> String {
    // FNV-1a 64-bit: rapide, stable et suffisant pour une clé d’interface.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in path.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{
        filetime_to_unix, map_sort, normalize_selection_ranges, stable_id, SORT_NAME_ASC,
    };
    use crate::{SelectionRange, SortColumn, SortDirection};

    #[test]
    fn maps_default_sort() {
        assert_eq!(
            map_sort(SortColumn::Name, SortDirection::Ascending),
            SORT_NAME_ASC
        );
    }

    #[test]
    fn converts_windows_epoch() {
        assert_eq!(filetime_to_unix(116_444_736_000_000_000), Some(0));
    }

    #[test]
    fn stable_ids_are_deterministic() {
        assert_eq!(stable_id(r"C:\test.txt"), stable_id(r"C:\test.txt"));
        assert_ne!(stable_id(r"C:\a.txt"), stable_id(r"C:\b.txt"));
    }
    #[test]
    fn normalizes_overlapping_selection_ranges() {
        let ranges = normalize_selection_ranges(vec![
            SelectionRange::new(20, 25),
            SelectionRange::new(4, 8),
            SelectionRange::new(9, 12),
            SelectionRange::new(24, 30),
        ]);
        assert_eq!(ranges, vec![SelectionRange::new(4, 12), SelectionRange::new(20, 30)]);
    }

}
