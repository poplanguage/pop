//! Rust implementation foundation for the public `Pop.Standard` Bubble.
//!
//! These APIs are intentionally small, typed, and function-first. They are
//! implementation adapters for the public Pop contracts, not a second source
//! language or a universal object layer.

mod baseline;
mod native_output;
mod rust_std;
pub mod text;

pub use baseline::{
    ApiBaselineError, ApiKind, ApiStatus, ApiTier, StandardApiBaseline, StandardApiEntry,
    parse_standard_api_baseline, standard_api_baseline,
};
pub use native_output::{
    NATIVE_EXPORTS, pop_std_print_int, pop_std_print_string, pop_std_terminal_flush,
    pop_std_terminal_write, pop_std_terminal_write_error, pop_std_terminal_write_line,
    print_string,
};
pub use rust_std::{
    RUST_STD_EXPORTS, pop_std_rust_available_parallelism, pop_std_rust_directory_access_close,
    pop_std_rust_directory_access_open, pop_std_rust_directory_exists,
    pop_std_rust_directory_exists_at, pop_std_rust_directory_is_directory_at,
    pop_std_rust_directory_list, pop_std_rust_directory_snapshot_close,
    pop_std_rust_directory_snapshot_count, pop_std_rust_directory_snapshot_name,
    pop_std_rust_environment_get, pop_std_rust_environment_has, pop_std_rust_file_access_close,
    pop_std_rust_file_access_open, pop_std_rust_file_close, pop_std_rust_file_create,
    pop_std_rust_file_exists, pop_std_rust_file_exists_at, pop_std_rust_file_handle_read,
    pop_std_rust_file_is_file, pop_std_rust_file_is_file_at, pop_std_rust_file_open,
    pop_std_rust_file_open_write, pop_std_rust_file_read_at, pop_std_rust_file_write,
    pop_std_rust_native_architecture, pop_std_rust_native_operating_system,
    pop_std_rust_net_ipv4_is_broadcast, pop_std_rust_net_ipv4_is_documentation,
    pop_std_rust_net_ipv4_is_link_local, pop_std_rust_net_ipv4_is_multicast,
    pop_std_rust_net_ipv6_is_documentation, pop_std_rust_net_ipv6_is_multicast,
    pop_std_rust_net_ipv6_is_unicast_link_local, pop_std_rust_net_ipv6_is_unique_local,
    pop_std_rust_process_executable, pop_std_rust_process_id, pop_std_rust_stderr_is_terminal,
    pop_std_rust_stdout_is_terminal,
};
