use std::collections::HashMap;

use crate::shell::{HeaderFormat, ShellType};

#[derive(Clone)]
pub struct AppState {
    /// Key is "METHOD /path", value is command
    pub commands: HashMap<String, String>,
    pub shell: ShellType,
    pub header_format: HeaderFormat,
    pub query_format: HeaderFormat,
}
