use crate::{
    EngineError, EngineStatus, QueryRequest, SearchPage, SearchResult, SelectionRequest,
    SortColumn, SortDirection,
};
use libloading::Library;
use std::env;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;

const PROPERTY_ID_NAME: u32 = 0;
const PROPERTY_ID_PATH: u32 = 1;
const PROPERTY_ID_SIZE: u32 = 2;
const PROPERTY_ID_EXTENSION: u32 = 3;
const PROPERTY_ID_DATE_MODIFIED: u32 = 5;
const DEFAULT_INSTANCE_NAME: &str = "";
const UNKNOWN_UINT64: u64 = u64::MAX;
const WINDOWS_TO_UNIX_SECONDS: u64 = 11_644_473_600;

type ConnectW = unsafe extern "system" fn(*const u16) -> *mut c_void;
type DestroyClient = unsafe extern "system" fn(*mut c_void) -> i32;
type GetLastError = unsafe extern "system" fn() -> u32;
type GetClientU32 = unsafe extern "system" fn(*mut c_void) -> u32;
type IsDbLoaded = unsafe extern "system" fn(*mut c_void) -> i32;
type CreateSearchState = unsafe extern "system" fn() -> *mut c_void;
type DestroySearchState = unsafe extern "system" fn(*mut c_void) -> i32;
type SetSearchTextW = unsafe extern "system" fn(*mut c_void, *const u16) -> i32;
type AddSearchSort = unsafe extern "system" fn(*mut c_void, u32, i32) -> i32;
type AddSearchPropertyRequest = unsafe extern "system" fn(*mut c_void, u32) -> i32;
type SetSearchViewport = unsafe extern "system" fn(*mut c_void, usize) -> i32;
type GetSearchViewport = unsafe extern "system" fn(*mut c_void) -> usize;
type Search = unsafe extern "system" fn(*mut c_void, *mut c_void) -> *mut c_void;
type DestroyResultList = unsafe extern "system" fn(*mut c_void) -> i32;
type GetResultListCount = unsafe extern "system" fn(*const c_void) -> usize;
type IsFolderResult = unsafe extern "system" fn(*const c_void, usize) -> i32;
type GetResultStringW = unsafe extern "system" fn(*const c_void, usize, *mut u16, usize) -> usize;
type GetResultU64 = unsafe extern "system" fn(*const c_void, usize) -> u64;

pub struct EverythingSdk {
    _library: Library,
    client: *mut c_void,
    instance_name: String,
    destroy_client: DestroyClient,
    get_last_error: GetLastError,
    get_major_version: GetClientU32,
    get_minor_version: GetClientU32,
    get_revision: GetClientU32,
    get_build_number: GetClientU32,
    is_db_loaded: IsDbLoaded,
    create_search_state: CreateSearchState,
    destroy_search_state: DestroySearchState,
    set_search_text_w: SetSearchTextW,
    add_search_sort: AddSearchSort,
    add_search_property_request: AddSearchPropertyRequest,
    set_search_viewport_offset: SetSearchViewport,
    set_search_viewport_count: SetSearchViewport,
    get_search_viewport_offset: GetSearchViewport,
    get_search_viewport_count: GetSearchViewport,
    search: Search,
    destroy_result_list: DestroyResultList,
    get_result_list_count: GetResultListCount,
    get_result_list_viewport_count: GetResultListCount,
    is_folder_result: IsFolderResult,
    get_result_name_w: GetResultStringW,
    get_result_path_w: GetResultStringW,
    get_result_size: GetResultU64,
    get_result_date_modified: GetResultU64,
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

        let connect_w = symbol!("Everything3_ConnectW", ConnectW);
        let destroy_client = symbol!("Everything3_DestroyClient", DestroyClient);
        let get_last_error = symbol!("Everything3_GetLastError", GetLastError);
        let get_major_version = symbol!("Everything3_GetMajorVersion", GetClientU32);
        let get_minor_version = symbol!("Everything3_GetMinorVersion", GetClientU32);
        let get_revision = symbol!("Everything3_GetRevision", GetClientU32);
        let get_build_number = symbol!("Everything3_GetBuildNumber", GetClientU32);
        let is_db_loaded = symbol!("Everything3_IsDBLoaded", IsDbLoaded);
        let create_search_state = symbol!("Everything3_CreateSearchState", CreateSearchState);
        let destroy_search_state = symbol!("Everything3_DestroySearchState", DestroySearchState);
        let set_search_text_w = symbol!("Everything3_SetSearchTextW", SetSearchTextW);
        let add_search_sort = symbol!("Everything3_AddSearchSort", AddSearchSort);
        let add_search_property_request = symbol!(
            "Everything3_AddSearchPropertyRequest",
            AddSearchPropertyRequest
        );
        let set_search_viewport_offset =
            symbol!("Everything3_SetSearchViewportOffset", SetSearchViewport);
        let set_search_viewport_count =
            symbol!("Everything3_SetSearchViewportCount", SetSearchViewport);
        let get_search_viewport_offset =
            symbol!("Everything3_GetSearchViewportOffset", GetSearchViewport);
        let get_search_viewport_count =
            symbol!("Everything3_GetSearchViewportCount", GetSearchViewport);
        let search = symbol!("Everything3_Search", Search);
        let destroy_result_list = symbol!("Everything3_DestroyResultList", DestroyResultList);
        let get_result_list_count = symbol!("Everything3_GetResultListCount", GetResultListCount);
        let get_result_list_viewport_count =
            symbol!("Everything3_GetResultListViewportCount", GetResultListCount);
        let is_folder_result = symbol!("Everything3_IsFolderResult", IsFolderResult);
        let get_result_name_w = symbol!("Everything3_GetResultNameW", GetResultStringW);
        let get_result_path_w = symbol!("Everything3_GetResultPathW", GetResultStringW);
        let get_result_size = symbol!("Everything3_GetResultSize", GetResultU64);
        let get_result_date_modified = symbol!("Everything3_GetResultDateModified", GetResultU64);

        let instance_name = configured_instance_name();
        let instance_wide = to_wide(&instance_name);
        let instance_ptr = if instance_name.is_empty() {
            ptr::null()
        } else {
            instance_wide.as_ptr()
        };
        let client = unsafe { connect_w(instance_ptr) };
        if client.is_null() {
            return Err(EngineError::ConnectionFailed {
                instance: display_instance_name(&instance_name),
                code: unsafe { get_last_error() },
            });
        }

        let version = unsafe {
            format!(
                "{}.{}.{}.{}",
                get_major_version(client),
                get_minor_version(client),
                get_revision(client),
                get_build_number(client),
            )
        };
        let supported = unsafe { get_major_version(client) == 1 && get_minor_version(client) >= 5 };
        if !supported {
            unsafe {
                destroy_client(client);
            }
            return Err(EngineError::UnsupportedEverythingVersion(version));
        }

        Ok(Self {
            _library: library,
            client,
            instance_name,
            destroy_client,
            get_last_error,
            get_major_version,
            get_minor_version,
            get_revision,
            get_build_number,
            is_db_loaded,
            create_search_state,
            destroy_search_state,
            set_search_text_w,
            add_search_sort,
            add_search_property_request,
            set_search_viewport_offset,
            set_search_viewport_count,
            get_search_viewport_offset,
            get_search_viewport_count,
            search,
            destroy_result_list,
            get_result_list_count,
            get_result_list_viewport_count,
            is_folder_result,
            get_result_name_w,
            get_result_path_w,
            get_result_size,
            get_result_date_modified,
        })
    }

    pub fn status(&self) -> EngineStatus {
        let loaded = unsafe { (self.is_db_loaded)(self.client) != 0 };
        let version = self.version();
        let instance = display_instance_name(&self.instance_name);
        EngineStatus {
            available: loaded,
            message: if loaded {
                format!("Everything {version} is connected to {instance} via SDK3.")
            } else {
                format!(
                    "SDK3 is connected to {instance}, but the Everything 1.5 database is unavailable."
                )
            },
            version: Some(version),
        }
    }

    pub fn query(&mut self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        let search_state_ptr = unsafe { (self.create_search_state)() };
        if search_state_ptr.is_null() {
            return Err(self.call_error("Everything3_CreateSearchState"));
        }
        let search_state = SearchStateGuard {
            pointer: search_state_ptr,
            destroy: self.destroy_search_state,
        };

        let search = to_wide(&request.query);
        self.check_call("Everything3_SetSearchTextW", unsafe {
            (self.set_search_text_w)(search_state.pointer, search.as_ptr())
        })?;

        let (sort_property, ascending) = map_sort(request.sort.column, request.sort.direction);
        self.check_call("Everything3_AddSearchSort", unsafe {
            (self.add_search_sort)(search_state.pointer, sort_property, ascending)
        })?;
        for property_id in [
            PROPERTY_ID_NAME,
            PROPERTY_ID_PATH,
            PROPERTY_ID_SIZE,
            PROPERTY_ID_DATE_MODIFIED,
        ] {
            self.check_call("Everything3_AddSearchPropertyRequest", unsafe {
                (self.add_search_property_request)(search_state.pointer, property_id)
            })?;
        }
        self.set_search_viewport(
            "Everything3_SetSearchViewportOffset",
            self.set_search_viewport_offset,
            self.get_search_viewport_offset,
            search_state.pointer,
            usize::try_from(request.offset)
                .expect("u32 offsets fit into usize on supported Windows targets"),
        )?;
        self.set_search_viewport(
            "Everything3_SetSearchViewportCount",
            self.set_search_viewport_count,
            self.get_search_viewport_count,
            search_state.pointer,
            usize::try_from(request.limit.clamp(1, 4096))
                .expect("viewport limits fit into usize on supported Windows targets"),
        )?;

        let result_list_ptr = unsafe { (self.search)(self.client, search_state.pointer) };
        if result_list_ptr.is_null() {
            return Err(self.call_error("Everything3_Search"));
        }
        let result_list = ResultListGuard {
            pointer: result_list_ptr,
            destroy: self.destroy_result_list,
        };

        let count = unsafe { (self.get_result_list_viewport_count)(result_list.pointer) };
        let total = u32::try_from(unsafe { (self.get_result_list_count)(result_list.pointer) })
            .unwrap_or(u32::MAX);
        let mut items = Vec::with_capacity(count);

        for index in 0..count {
            let name = self.read_result_string(
                self.get_result_name_w,
                result_list.pointer,
                index,
                "Everything3_GetResultNameW",
            )?;
            let parent_path = self.read_result_string(
                self.get_result_path_w,
                result_list.pointer,
                index,
                "Everything3_GetResultPathW",
            )?;
            let full_path = if parent_path.ends_with('\\') || parent_path.ends_with('/') {
                format!("{parent_path}{name}")
            } else if parent_path.is_empty() {
                name.clone()
            } else {
                format!("{parent_path}\\{name}")
            };
            let is_dir = unsafe { (self.is_folder_result)(result_list.pointer, index) != 0 };
            let raw_size = unsafe { (self.get_result_size)(result_list.pointer, index) };
            let raw_date = unsafe { (self.get_result_date_modified)(result_list.pointer, index) };

            items.push(SearchResult {
                id: stable_id(&full_path),
                name,
                parent_path,
                full_path,
                size: (!is_dir && raw_size != UNKNOWN_UINT64).then_some(raw_size),
                modified_unix: (raw_date != UNKNOWN_UINT64)
                    .then(|| filetime_to_unix(raw_date))
                    .flatten(),
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
        let requested = ranges.iter().copied().map(|range| range.len()).sum::<u64>();
        let max_items_as_u64 = u64::try_from(max_items).unwrap_or(u64::MAX);
        if requested > max_items_as_u64 {
            return Err(EngineError::InvalidSelection(format!(
                "This operation is limited to {max_items} items at a time"
            )));
        }
        let capacity =
            usize::try_from(requested).expect("validated selection count fits into usize");
        let mut paths = Vec::with_capacity(capacity);

        for range in ranges {
            let mut offset = range.start;
            loop {
                if is_cancelled() {
                    return Err(EngineError::InvalidSelection(
                        "The search changed while preparing the operation".into(),
                    ));
                }
                let remaining = u64::from(range.end) - u64::from(offset) + 1;
                let limit =
                    u32::try_from(remaining.min(1_024)).expect("page size is bounded to 1024");
                let page = self.query(QueryRequest {
                    query: request.query.clone(),
                    offset,
                    limit,
                    sort: request.sort,
                    request_id: request.request_id,
                })?;
                let returned =
                    u32::try_from(page.items.len()).expect("page contains at most 1024 items");
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

    fn version(&self) -> String {
        unsafe {
            format!(
                "{}.{}.{}.{}",
                (self.get_major_version)(self.client),
                (self.get_minor_version)(self.client),
                (self.get_revision)(self.client),
                (self.get_build_number)(self.client),
            )
        }
    }

    fn read_result_string(
        &self,
        getter: GetResultStringW,
        result_list: *const c_void,
        index: usize,
        operation: &'static str,
    ) -> Result<String, EngineError> {
        let required = unsafe { getter(result_list, index, ptr::null_mut(), 0) };
        let mut buffer = vec![0_u16; required.saturating_add(1).max(1)];
        let written = unsafe { getter(result_list, index, buffer.as_mut_ptr(), buffer.len()) };
        let error_code = unsafe { (self.get_last_error)() };
        if error_code != 0 || written >= buffer.len() {
            return Err(EngineError::SdkCall {
                operation,
                code: error_code,
            });
        }
        buffer.truncate(written);
        Ok(String::from_utf16_lossy(&buffer))
    }

    fn set_search_viewport(
        &self,
        operation: &'static str,
        setter: SetSearchViewport,
        getter: GetSearchViewport,
        search_state: *mut c_void,
        value: usize,
    ) -> Result<(), EngineError> {
        unsafe { setter(search_state, value) };
        let value_applied_by_sdk = unsafe { getter(search_state) };
        if value_applied_by_sdk == value {
            Ok(())
        } else {
            Err(self.call_error(operation))
        }
    }

    fn check_call(&self, operation: &'static str, succeeded: i32) -> Result<(), EngineError> {
        if succeeded != 0 {
            Ok(())
        } else {
            Err(self.call_error(operation))
        }
    }

    fn call_error(&self, operation: &'static str) -> EngineError {
        EngineError::SdkCall {
            operation,
            code: unsafe { (self.get_last_error)() },
        }
    }
}

impl Drop for EverythingSdk {
    fn drop(&mut self) {
        if !self.client.is_null() {
            unsafe {
                (self.destroy_client)(self.client);
            }
            self.client = ptr::null_mut();
        }
    }
}

struct SearchStateGuard {
    pointer: *mut c_void,
    destroy: DestroySearchState,
}

impl Drop for SearchStateGuard {
    fn drop(&mut self) {
        unsafe {
            (self.destroy)(self.pointer);
        }
    }
}

struct ResultListGuard {
    pointer: *mut c_void,
    destroy: DestroyResultList,
}

impl Drop for ResultListGuard {
    fn drop(&mut self) {
        unsafe {
            (self.destroy)(self.pointer);
        }
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

fn configured_instance_name() -> String {
    env::var("EVERYTHING_INSTANCE").unwrap_or_else(|_| DEFAULT_INSTANCE_NAME.into())
}

fn display_instance_name(instance_name: &str) -> String {
    if instance_name.is_empty() {
        "l’instance principale".into()
    } else {
        format!("l’instance « {instance_name} »")
    }
}

fn locate_sdk() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("EVERYTHING_SDK3_DLL") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("Everything3_x64.dll"));
        }
    }
    if let Ok(current) = env::current_dir() {
        candidates.push(current.join("Everything3_x64.dll"));
        candidates.push(current.join("src-tauri").join("Everything3_x64.dll"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn map_sort(column: SortColumn, direction: SortDirection) -> (u32, i32) {
    let property_id = match column {
        SortColumn::Name => PROPERTY_ID_NAME,
        SortColumn::Path => PROPERTY_ID_PATH,
        SortColumn::Extension => PROPERTY_ID_EXTENSION,
        SortColumn::Size => PROPERTY_ID_SIZE,
        SortColumn::Modified => PROPERTY_ID_DATE_MODIFIED,
    };
    let ascending = i32::from(direction == SortDirection::Ascending);
    (property_id, ascending)
}

fn filetime_to_unix(filetime: u64) -> Option<i64> {
    let seconds = filetime / 10_000_000;
    if seconds < WINDOWS_TO_UNIX_SECONDS {
        return None;
    }
    i64::try_from(seconds - WINDOWS_TO_UNIX_SECONDS).ok()
}

fn stable_id(path: &str) -> String {
    let mut fnv1a_64_hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in path.as_bytes() {
        fnv1a_64_hash ^= u64::from(*byte);
        fnv1a_64_hash = fnv1a_64_hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{fnv1a_64_hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{
        filetime_to_unix, map_sort, normalize_selection_ranges, stable_id, PROPERTY_ID_NAME,
    };
    use crate::{SelectionRange, SortColumn, SortDirection};

    #[test]
    fn maps_default_sort() {
        assert_eq!(
            map_sort(SortColumn::Name, SortDirection::Ascending),
            (PROPERTY_ID_NAME, 1)
        );
    }

    #[test]
    fn maps_extension_sort() {
        assert_eq!(
            map_sort(SortColumn::Extension, SortDirection::Ascending),
            (super::PROPERTY_ID_EXTENSION, 1)
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
        assert_eq!(
            ranges,
            vec![SelectionRange::new(4, 12), SelectionRange::new(20, 30)]
        );
    }
}
