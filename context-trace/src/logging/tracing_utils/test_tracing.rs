//! Main TestTracing API and initialization

use std::{
    env,
    fs,
    io::Write,
    path::{
        Path,
        PathBuf,
    },
    sync::Once,
};
use tracing::Dispatch;
use tracing_subscriber::{
    EnvFilter,
    Layer,
    layer::SubscriberExt,
};

use super::{
    config::TracingConfig,
    formatter::CompactFieldsFormatter,
    panic::install_panic_hook,
    timer::CompactTimer,
};

static GLOBAL_INIT: Once = Once::new();

/// A file wrapper that flushes after every write to ensure logs are visible on panic.
///
/// This is necessary because when a test panics, buffered data may not be flushed
/// to disk, resulting in truncated log files.
#[derive(Clone)]
struct FlushingWriter {
    file: std::sync::Arc<std::sync::Mutex<fs::File>>,
}

impl FlushingWriter {
    fn new(file: fs::File) -> Self {
        Self {
            file: std::sync::Arc::new(std::sync::Mutex::new(file)),
        }
    }
}

impl Write for FlushingWriter {
    fn write(
        &mut self,
        buf: &[u8],
    ) -> std::io::Result<usize> {
        // Use lock().ok() to handle poisoned mutex during panic gracefully
        // If we can't get the lock (e.g., during unwind), skip the write
        let Some(mut file) = self.file.lock().ok().or_else(|| {
            // Mutex is poisoned, try to recover it
            self.file.clear_poison();
            self.file.lock().ok()
        }) else {
            return Err(std::io::Error::other("Failed to acquire file lock"));
        };
        let result = file.write(buf)?;
        file.flush()?;
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(mut file) = self.file.lock().ok().or_else(|| {
            self.file.clear_poison();
            self.file.lock().ok()
        }) {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// A writer that pretty-prints JSON output with indentation
/// 
/// Wraps another writer and buffers JSON objects. When a complete JSON
/// object is detected, it's parsed and re-serialized with indentation.
#[derive(Clone)]
struct PrettyJsonWriter<W> {
    inner: W,
    buffer: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl<W: Clone> PrettyJsonWriter<W> {
    fn new(writer: W) -> Self {
        Self {
            inner: writer,
            buffer: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

/// Convert Windows paths to Unix paths in a string
fn to_unix_path(s: &str) -> String {
    // Replace backslashes with forward slashes
    let mut result = s.replace('\\', "/");
    // Collapse consecutive slashes (e.g., // -> /) but preserve :// for URLs
    while result.contains("//") {
        result = result.replace("//", "/");
    }
    result
}

/// Parse a Rust function signature into a JSON object
/// e.g., "fn foo(&mut self, x: Type) -> Result" -> {"name": "foo", "params": [...], "return_type": "Result"}
fn parse_fn_signature(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();
    if !s.starts_with("fn ") {
        return None;
    }
    
    // Extract function name
    let after_fn = &s[3..];
    let name_end = after_fn.find('(')?;
    let name = after_fn[..name_end].trim();
    
    // Find params between first ( and matching )
    let params_start = s.find('(')?;
    let mut depth = 0;
    let mut params_end = None;
    for (i, c) in s[params_start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    params_end = Some(params_start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let params_end = params_end?;
    let params_str = &s[params_start + 1..params_end];
    
    // Parse params - split by comma but respect nested brackets
    let params = parse_param_list(params_str);
    
    // Extract return type if present
    let after_params = &s[params_end + 1..];
    let return_type = if let Some(arrow_pos) = after_params.find("->") {
        Some(after_params[arrow_pos + 2..].trim().to_string())
    } else {
        None
    };
    
    let mut obj = serde_json::Map::new();
    obj.insert("name".to_string(), serde_json::Value::String(name.to_string()));
    obj.insert("params".to_string(), serde_json::Value::Array(params));
    if let Some(ret) = return_type {
        obj.insert("return_type".to_string(), serde_json::Value::String(ret));
    }
    
    Some(serde_json::Value::Object(obj))
}

/// Parse a comma-separated parameter list respecting nested brackets
fn parse_param_list(s: &str) -> Vec<serde_json::Value> {
    let mut params = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    
    for c in s.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let param = current.trim();
                if !param.is_empty() {
                    params.push(parse_param(param));
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    
    let param = current.trim();
    if !param.is_empty() {
        params.push(parse_param(param));
    }
    
    params
}

/// Parse a single parameter like "&mut self" or "x: Type" into JSON
fn parse_param(s: &str) -> serde_json::Value {
    let s = s.trim();
    
    // Handle self parameters
    if s == "self" || s == "&self" || s == "&mut self" {
        let mut obj = serde_json::Map::new();
        obj.insert("self".to_string(), serde_json::Value::String(s.to_string()));
        return serde_json::Value::Object(obj);
    }
    
    // Try to split name: type
    if let Some(colon_pos) = s.find(':') {
        let name = s[..colon_pos].trim();
        let ty = s[colon_pos + 1..].trim();
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), serde_json::Value::String(name.to_string()));
        obj.insert("type".to_string(), serde_json::Value::String(ty.to_string()));
        return serde_json::Value::Object(obj);
    }
    
    // Fallback: just return as string
    serde_json::Value::String(s.to_string())
}

/// Parse a token-like value: "text"(index) -> {"text": "text", "index": index}
fn parse_token_value(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();
    
    // Pattern: "text"(number) or 'text'(number)
    if !s.starts_with('"') && !s.starts_with('\'') {
        return None;
    }
    
    let quote_char = s.chars().next()?;
    let end_quote = s[1..].find(quote_char)?;
    let text = &s[1..end_quote + 1];
    
    let after_quote = &s[end_quote + 2..];
    if !after_quote.starts_with('(') || !after_quote.ends_with(')') {
        return None;
    }
    
    let index_str = &after_quote[1..after_quote.len() - 1];
    let index: i64 = index_str.parse().ok()?;
    
    let mut obj = serde_json::Map::new();
    obj.insert("text".to_string(), serde_json::Value::String(text.to_string()));
    obj.insert("index".to_string(), serde_json::Value::Number(index.into()));
    
    Some(serde_json::Value::Object(obj))
}

/// Map of known enum variants to their parent enum type
fn get_enum_parent(variant: &str) -> Option<&'static str> {
    match variant {
        "Some" | "None" => Some("Option"),
        "Ok" | "Err" => Some("Result"),
        _ => None,
    }
}

/// Try to parse a Rust Debug-formatted struct, e.g., "StructName { field: value }"
fn parse_rust_debug(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();
    
    // Don't try to parse arrays as structs
    if s.starts_with('[') {
        return None;
    }
    
    // Handle unit variants like "None" without braces
    if s == "None" {
        let mut obj = serde_json::Map::new();
        obj.insert("_type".to_string(), serde_json::Value::String("Option".to_string()));
        obj.insert("_variant".to_string(), serde_json::Value::String("None".to_string()));
        return Some(serde_json::Value::Object(obj));
    }
    
    // Check if it looks like "Name { ... }" or "Name(...)" 
    let brace_pos = s.find(|c| c == '{' || c == '(');
    let brace_pos = brace_pos?;
    
    let struct_name = s[..brace_pos].trim();
    
    // struct_name must be a valid identifier (alphanumeric, ::, _, no quotes/brackets)
    if struct_name.is_empty() 
        || struct_name.contains(' ') 
        || struct_name.contains('"')
        || struct_name.contains('[')
        || struct_name.contains(']')
    {
        return None;
    }
    
    let open_brace = s.chars().nth(brace_pos)?;
    let close_brace = if open_brace == '{' { '}' } else { ')' };
    
    // Find matching close brace
    let content_start = brace_pos + 1;
    let mut depth = 1;
    let mut content_end = None;
    for (i, c) in s[content_start..].char_indices() {
        if c == open_brace {
            depth += 1;
        } else if c == close_brace {
            depth -= 1;
            if depth == 0 {
                content_end = Some(content_start + i);
                break;
            }
        }
    }
    let content_end = content_end?;
    let content = &s[content_start..content_end];
    
    let mut obj = serde_json::Map::new();
    
    // Check if this is a known enum variant (e.g., Some, Ok, Err)
    // If so, use the parent enum as _type and add _variant
    if let Some(parent_enum) = get_enum_parent(struct_name) {
        obj.insert("_type".to_string(), serde_json::Value::String(parent_enum.to_string()));
        obj.insert("_variant".to_string(), serde_json::Value::String(struct_name.to_string()));
    } else {
        // Regular struct/enum - use the name directly as _type
        obj.insert("_type".to_string(), serde_json::Value::String(struct_name.to_string()));
    }
    
    // Parse fields
    if open_brace == '{' {
        // Named fields: "field: value, field2: value2"
        parse_struct_fields(content, &mut obj);
    } else {
        // Tuple fields: "value1, value2"
        let values = parse_tuple_fields(content);
        if !values.is_empty() {
            obj.insert("_values".to_string(), serde_json::Value::Array(values));
        }
    }
    
    Some(serde_json::Value::Object(obj))
}

/// Parse struct fields like "field: value, field2: value2"
fn parse_struct_fields(s: &str, obj: &mut serde_json::Map<String, serde_json::Value>) {
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_value = false;
    let mut depth = 0;
    
    for c in s.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
            ':' if depth == 0 && !in_value => {
                in_value = true;
            }
            ',' if depth == 0 => {
                let key = current_key.trim();
                let value = current_value.trim();
                if !key.is_empty() {
                    obj.insert(
                        key.to_string(),
                        try_parse_value(value),
                    );
                }
                current_key.clear();
                current_value.clear();
                in_value = false;
            }
            _ => {
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
        }
    }
    
    // Handle last field
    let key = current_key.trim();
    let value = current_value.trim();
    if !key.is_empty() {
        obj.insert(
            key.to_string(),
            try_parse_value(value),
        );
    }
}

/// Parse tuple fields
fn parse_tuple_fields(s: &str) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    
    for c in s.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let value = current.trim();
                if !value.is_empty() {
                    values.push(try_parse_value(value));
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    
    let value = current.trim();
    if !value.is_empty() {
        values.push(try_parse_value(value));
    }
    
    values
}

/// Try to parse a value into a more structured form
fn try_parse_value(s: &str) -> serde_json::Value {
    let s = s.trim();
    
    // Try parsing as a Rust array [...] first
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        let values = parse_tuple_fields(inner);
        return serde_json::Value::Array(values);
    }
    
    // Try parsing as a token like "a"(0) -> {"text": "a", "index": 0}
    if let Some(parsed) = parse_token_value(s) {
        return parsed;
    }
    
    // Try parsing as a nested struct
    if let Some(parsed) = parse_rust_debug(s) {
        return parsed;
    }
    
    // Handle PhantomData<...> by extracting the type parameter
    if s.starts_with("PhantomData<") && s.ends_with('>') {
        let inner = &s[12..s.len() - 1];
        let mut obj = serde_json::Map::new();
        obj.insert("_type".to_string(), serde_json::Value::String("PhantomData".to_string()));
        obj.insert("type_param".to_string(), serde_json::Value::String(inner.to_string()));
        return serde_json::Value::Object(obj);
    }
    
    // Handle quoted strings
    if s.starts_with('"') && s.ends_with('"') {
        return serde_json::Value::String(s[1..s.len() - 1].to_string());
    }
    
    // Try parsing as number
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return serde_json::Value::Number(num);
        }
    }
    
    // Boolean
    if s == "true" {
        return serde_json::Value::Bool(true);
    }
    if s == "false" {
        return serde_json::Value::Bool(false);
    }
    
    // Unit types like ()
    if s == "()" {
        return serde_json::Value::Null;
    }
    
    // Fallback: return as string
    serde_json::Value::String(s.to_string())
}

/// Check if a string looks like it might be a parseable Rust debug value
fn looks_like_rust_debug(s: &str) -> bool {
    let s = s.trim();
    // Struct/Enum patterns: Name { ... }, Name(...), Name
    // Array pattern: [...]
    // Token pattern: "text"(num)
    // Option/Result: Some(...), None, Ok(...), Err(...)
    // With path: path::Name { ... }
    s.starts_with('[')
        || s.starts_with('"') && s.contains('(')
        || s.starts_with("Some(")
        || s.starts_with("None")
        || s.starts_with("Ok(")
        || s.starts_with("Err(")
        || s.starts_with("PhantomData")
        || (s.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false)
            && (s.contains('{') || s.contains('(') || s.contains("::")))
}

/// Transform ALL string fields that look like Rust debug values into structured JSON objects
fn transform_structured_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(obj) => {
            // Check if this is the "fields" object - if so, parse all its string values
            let is_fields_obj = obj.contains_key("message") || obj.keys().any(|k| {
                obj.get(k).map(|v| matches!(v, serde_json::Value::String(s) if looks_like_rust_debug(s))).unwrap_or(false)
            });
            
            // Process each key-value pair
            let keys: Vec<String> = obj.keys().cloned().collect();
            for key in keys {
                if let Some(val) = obj.get_mut(&key) {
                    // First, recursively transform nested objects
                    transform_structured_fields(val);
                    
                    // Parse string values that look like Rust debug output
                    if let serde_json::Value::String(s) = val {
                        // Special handling for fn_sig
                        if key == "fn_sig" {
                            if let Some(parsed) = parse_fn_signature(s) {
                                *val = parsed;
                                continue;
                            }
                        }
                        
                        // For fields objects or known field keys, try to parse the value
                        if is_fields_obj || key == "fields" {
                            if looks_like_rust_debug(s) {
                                let parsed = try_parse_value(s);
                                if !matches!(parsed, serde_json::Value::String(_)) {
                                    *val = parsed;
                                }
                            }
                        }
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                transform_structured_fields(item);
            }
        }
        _ => {}
    }
}

/// Recursively convert all path-like strings in a JSON value to Unix format
fn convert_paths_to_unix(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            // Convert if it looks like a path (contains backslash)
            if s.contains('\\') {
                *s = to_unix_path(s);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                convert_paths_to_unix(item);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj {
                convert_paths_to_unix(v);
            }
        }
        _ => {}
    }
}

impl<W: Write + Clone> Write for PrettyJsonWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut buffer = self.buffer.lock().map_err(|_| {
            std::io::Error::other("Failed to acquire buffer lock")
        })?;
        
        // Add incoming data to buffer
        buffer.extend_from_slice(buf);
        
        // Check if we have a complete JSON object (ends with newline)
        if buffer.ends_with(b"\n") {
            // Try to parse and pretty-print the JSON
            if let Ok(json_str) = std::str::from_utf8(&buffer) {
                let trimmed = json_str.trim();
                if !trimmed.is_empty() {
                    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        // Convert Windows paths to Unix paths
                        convert_paths_to_unix(&mut value);
                        
                        // Transform structured fields (fn_sig, etc.) into JSON objects
                        transform_structured_fields(&mut value);
                        
                        // Write pretty-printed JSON
                        let pretty = serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| trimmed.to_string());
                        let mut inner = self.inner.clone();
                        inner.write_all(pretty.as_bytes())?;
                        inner.write_all(b"\n\n")?; // Double newline between entries
                        inner.flush()?;
                        buffer.clear();
                        return Ok(buf.len());
                    }
                }
            }
            
            // Fallback: write raw data if JSON parsing fails
            let mut inner = self.inner.clone();
            inner.write_all(&buffer)?;
            inner.flush()?;
            buffer.clear();
        }
        
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.clone().flush()
    }
}

/// Trait for types that can provide access to a Hypergraph for test graph registration
#[cfg(any(test, feature = "test-api"))]
pub trait AsGraphRef<G: crate::graph::kind::GraphKind> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display;
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G> for &crate::Hypergraph<G> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        crate::graph::test_graph::register_test_graph(self);
    }
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G>
    for &crate::HypergraphRef<G>
{
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        // Use the new register_test_graph_ref to avoid cloning
        crate::graph::test_graph::register_test_graph_ref(self);
    }
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G> for crate::Hypergraph<G> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        crate::graph::test_graph::register_test_graph(&self);
    }
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G>
    for crate::HypergraphRef<G>
{
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        // Use the new register_test_graph_ref to avoid cloning
        crate::graph::test_graph::register_test_graph_ref(&self);
    }
}

/// Guard that handles test logging lifecycle
///
/// Automatically cleans up log files when the test succeeds (guard is dropped without panic).
/// Also handles test graph registration and cleanup.
/// The guard holds a tracing dispatcher that's active for the lifetime of the test.
pub struct TestTracing {
    log_file_path: Option<PathBuf>,
    keep_success_logs: bool,
    clear_test_graph_on_drop: bool,
    _dispatcher: Dispatch,
    _guard: tracing::dispatcher::DefaultGuard,
}

impl TestTracing {
    /// Initialize tracing for a test
    ///
    /// # Example
    /// ```no_run
    /// use context_trace::logging::tracing_utils::TestTracing;
    ///
    /// #[test]
    /// fn my_test() {
    ///     let _tracing = TestTracing::init("my_test");
    ///     // Test code with automatic tracing
    ///     // Log file will be deleted if test passes
    /// }
    /// ```
    pub fn init(test_name: &str) -> Self {
        Self::init_with_config(test_name, TracingConfig::default())
    }

    /// Initialize tracing with custom configuration
    pub fn init_with_config(
        test_name: &str,
        config: TracingConfig,
    ) -> Self {
        Self::init_internal(test_name, config, false)
    }

    /// Initialize tracing and register a test graph
    ///
    /// # Example
    /// ```no_run
    /// use context_trace::{Hypergraph, logging::tracing_utils::TestTracing};
    ///
    /// #[test]
    /// fn my_test() {
    ///     let graph = Hypergraph::default();
    ///     let _tracing = TestTracing::init_with_graph("my_test", &graph);
    ///     // Test code - tokens will show string representations
    ///     // Graph and log file will be cleaned up if test passes
    /// }
    /// ```
    #[cfg(any(test, feature = "test-api"))]
    pub fn init_with_graph<G>(
        test_name: &str,
        graph: impl AsGraphRef<G>,
    ) -> Self
    where
        G: crate::graph::kind::GraphKind + Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        graph.register_test_graph();
        Self::init_internal(test_name, TracingConfig::default(), true)
    }

    /// Initialize tracing with custom configuration and register a test graph
    #[cfg(any(test, feature = "test-api"))]
    pub fn init_with_config_and_graph<G>(
        test_name: &str,
        config: TracingConfig,
        graph: impl AsGraphRef<G>,
    ) -> Self
    where
        G: crate::graph::kind::GraphKind + Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        graph.register_test_graph();
        Self::init_internal(test_name, config, true)
    }

    fn init_internal(
        test_name: &str,
        config: TracingConfig,
        clear_test_graph_on_drop: bool,
    ) -> Self {
        // Initialize global tracing only once
        GLOBAL_INIT.call_once(|| {
            // This is a placeholder - actual subscriber will be set per-test
        });

        // Install panic hook to log panics before spans close
        install_panic_hook(config.format.panic.clone());

        // Create log directory
        if config.log_to_file {
            fs::create_dir_all(&config.log_dir).ok();
        }

        let log_file_path = if config.log_to_file {
            Some(config.log_dir.join(format!("{}.log", test_name)))
        } else {
            None
        };

        // Build separate filters for stdout and file
        let stdout_filter = if let Some(directives) =
            &config.stdout_filter_directives
        {
            EnvFilter::try_new(directives).unwrap_or_else(|_| {
                EnvFilter::new(config.stdout_level.as_str())
            })
        } else {
            // Check for LOG_FILTER first (preferred), then RUST_LOG, otherwise use stdout level
            env::var("LOG_FILTER")
                .ok()
                .and_then(|filter| EnvFilter::try_new(&filter).ok())
                .or_else(|| EnvFilter::try_from_default_env().ok())
                .unwrap_or_else(|| EnvFilter::new(config.stdout_level.as_str()))
        };

        let file_filter = if let Some(directives) =
            &config.file_filter_directives
        {
            EnvFilter::try_new(directives)
                .unwrap_or_else(|_| EnvFilter::new(config.file_level.as_str()))
        } else {
            // For file output, also check LOG_FILTER
            env::var("LOG_FILTER")
                .ok()
                .and_then(|filter| EnvFilter::try_new(&filter).ok())
                .unwrap_or_else(|| EnvFilter::new(config.file_level.as_str()))
        };

        // Create the subscriber without a global filter
        let registry = tracing_subscriber::registry();

        // Extract config values to avoid partial move issues
        let span_events = config.span_events;
        let log_to_stdout = config.log_to_stdout;
        let format_config = config.format.clone();

        // Build layers based on configuration
        // Timestamp display is controlled by the formatter's show_timestamp config,
        // so we always use CompactTimer and let the formatter decide whether to call format_time.
        // For file output, we use JSON format for easy parsing by the log viewer
        // Create dispatcher based on configuration
        let dispatcher = match (log_to_stdout, log_file_path.as_ref()) {
            (true, Some(path)) => {
                // Both stdout and file
                let file =
                    fs::File::create(path).expect("Failed to create log file");
                let flushing_writer = FlushingWriter::new(file);
                let pretty_writer = PrettyJsonWriter::new(flushing_writer);

                let stdout_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_span_events(span_events.clone())
                    .with_target(false)
                    .with_file(false)
                    .with_line_number(false)
                    .with_level(false)
                    .with_ansi(format_config.enable_ansi)
                    .with_timer(CompactTimer::new())
                    .event_format(CompactFieldsFormatter::new(
                        format_config.clone(),
                    ))
                    .fmt_fields(super::SpanFieldFormatter)
                    .with_filter(stdout_filter);

                // File layer uses pretty-printed JSON format for human readability
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(move || pretty_writer.clone())
                    .with_span_events(span_events)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_ansi(false)
                    .json()
                    .with_filter(file_filter);

                Dispatch::new(registry.with(stdout_layer).with(file_layer))
            },
            (true, None) => {
                // Only stdout
                let stdout_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_span_events(span_events)
                    .with_target(false)
                    .with_file(false)
                    .with_line_number(false)
                    .with_level(false)
                    .with_ansi(format_config.enable_ansi)
                    .with_timer(CompactTimer::new())
                    .event_format(CompactFieldsFormatter::new(format_config))
                    .fmt_fields(super::SpanFieldFormatter)
                    .with_filter(stdout_filter);

                Dispatch::new(registry.with(stdout_layer))
            },
            (false, Some(path)) => {
                // Only file - use pretty-printed JSON format for human readability
                let file =
                    fs::File::create(path).expect("Failed to create log file");
                let flushing_writer = FlushingWriter::new(file);
                let pretty_writer = PrettyJsonWriter::new(flushing_writer);

                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(move || pretty_writer.clone())
                    .with_span_events(span_events)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_ansi(false)
                    .json()
                    .with_filter(file_filter);

                Dispatch::new(registry.with(file_layer))
            },
            (false, None) => {
                // No output - minimal subscriber
                Dispatch::new(registry)
            },
        };

        // Set as the default dispatcher for this test's scope
        let guard = tracing::dispatcher::set_default(&dispatcher);

        tracing::info!(
            test_name = %test_name,
            log_file = ?log_file_path,
            "Test tracing initialized"
        );

        Self {
            log_file_path,
            keep_success_logs: config.keep_success_logs,
            clear_test_graph_on_drop,
            _dispatcher: dispatcher,
            _guard: guard,
        }
    }

    /// Get the path to the log file for this test
    pub fn log_file(&self) -> Option<&Path> {
        self.log_file_path.as_deref()
    }

    /// Explicitly keep the log file (don't delete on drop)
    ///
    /// Useful if you want to preserve logs even for passing tests
    pub fn keep_log(mut self) -> Self {
        self.keep_success_logs = true;
        self
    }

    /// Explicitly clear the test graph on drop
    ///
    /// Useful if you manually registered a test graph but didn't use init_with_graph
    pub fn clear_test_graph(mut self) -> Self {
        self.clear_test_graph_on_drop = true;
        self
    }
}

impl Drop for TestTracing {
    fn drop(&mut self) {
        // Check if we're unwinding (test panicked/failed)
        let is_panicking = std::thread::panicking();

        if !is_panicking && !self.keep_success_logs {
            // Test passed and keep_success_logs disabled - clean up log file
            if let Some(ref path) = self.log_file_path {
                tracing::info!(
                    log_file = %path.display(),
                    "Test passed, removing log file"
                );
                fs::remove_file(path).ok();
            }
        } else {
            // Test failed or keep_success_logs enabled - keep log file
            if let Some(ref path) = self.log_file_path {
                if is_panicking {
                    eprintln!(
                        "\n❌ Test failed! Log file preserved at: {}",
                        path.display()
                    );
                } else if self.keep_success_logs {
                    eprintln!(
                        "\n📝 Test passed! Log file kept at: {}",
                        path.display()
                    );
                }
            }
        }

        // Clear test graph if requested
        #[cfg(any(test, feature = "test-api"))]
        if self.clear_test_graph_on_drop {
            crate::graph::test_graph::clear_test_graph();
        }
    }
}
