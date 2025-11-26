//! WASM runtime engine for live cells
//!
//! This module provides the wasmtime-based execution engine for monowiki live cells.
//! It handles WASM module compilation, instantiation, and execution with proper
//! host function bindings.

use crate::abi::{Severity, Span};
use crate::host::RuntimeHost;
use anyhow::Result;
use wasmtime::*;

/// WASM runtime engine for live cells
///
/// This engine manages wasmtime configuration, host function linking,
/// and provides facilities for compiling and running live cell WASM modules.
pub struct LiveCellEngine {
    engine: Engine,
    linker: Linker<RuntimeHost>,
}

impl LiveCellEngine {
    /// Create a new live cell engine with all host functions registered
    pub fn new() -> Result<Self> {
        // Configure the wasmtime engine with reasonable defaults
        let mut config = Config::new();
        config.wasm_multi_memory(false);
        config.wasm_bulk_memory(true);

        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);

        // Register all host function interfaces
        Self::register_signal_imports(&mut linker)?;
        Self::register_ui_imports(&mut linker)?;
        Self::register_diagnostics_imports(&mut linker)?;
        Self::register_fetch_imports(&mut linker)?;
        Self::register_dataspace_imports(&mut linker)?;

        Ok(Self { engine, linker })
    }

    /// Register signal-related host functions
    fn register_signal_imports(linker: &mut Linker<RuntimeHost>) -> Result<()> {
        // signals::signal-create(initial: list<u8>) -> u64
        linker.func_wrap(
            "monowiki:runtime/signals",
            "signal-create",
            |mut caller: Caller<'_, RuntimeHost>, ptr: i32, len: i32| -> i64 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let data = mem.data(&caller);
                let initial = data[ptr as usize..(ptr + len) as usize].to_vec();

                match caller.data_mut().signal_create(&initial) {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )?;

        // signals::signal-get(id: u64) -> list<u8>
        // Returns length and writes to a pre-allocated buffer
        linker.func_wrap(
            "monowiki:runtime/signals",
            "signal-get",
            |mut caller: Caller<'_, RuntimeHost>, id: i64, out_ptr: i32| -> i32 {
                match caller.data().signal_get(id as u64) {
                    Ok(value) => {
                        let mem = match caller.get_export("memory") {
                            Some(Extern::Memory(m)) => m,
                            _ => return -1,
                        };

                        if mem.write(&mut caller, out_ptr as usize, &value).is_ok() {
                            value.len() as i32
                        } else {
                            -1
                        }
                    }
                    Err(_) => -1,
                }
            },
        )?;

        // signals::signal-set(id: u64, value: list<u8>)
        linker.func_wrap(
            "monowiki:runtime/signals",
            "signal-set",
            |mut caller: Caller<'_, RuntimeHost>, id: i64, ptr: i32, len: i32| {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return,
                };

                let data = mem.data(&caller);
                let value = data[ptr as usize..(ptr + len) as usize].to_vec();
                let _ = caller.data_mut().signal_set(id as u64, &value);
            },
        )?;

        // signals::signal-subscribe(id: u64, callback-id: u64)
        linker.func_wrap(
            "monowiki:runtime/signals",
            "signal-subscribe",
            |mut caller: Caller<'_, RuntimeHost>, id: i64, callback_id: i64| {
                let _ = caller.data_mut().signal_subscribe(id as u64, callback_id as u64);
            },
        )?;

        Ok(())
    }

    /// Register UI widget host functions
    fn register_ui_imports(linker: &mut Linker<RuntimeHost>) -> Result<()> {
        // ui::slider(min: f64, max: f64, initial: f64) -> u64
        linker.func_wrap(
            "monowiki:runtime/ui",
            "slider",
            |mut caller: Caller<'_, RuntimeHost>, min: f64, max: f64, initial: f64| -> i64 {
                match caller.data_mut().ui_slider(min, max, initial) {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )?;

        // ui::text-input(placeholder: string, initial: string) -> u64
        linker.func_wrap(
            "monowiki:runtime/ui",
            "text-input",
            |mut caller: Caller<'_, RuntimeHost>,
             placeholder_ptr: i32,
             placeholder_len: i32,
             initial_ptr: i32,
             initial_len: i32| -> i64 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let data = mem.data(&caller);
                let placeholder =
                    data[placeholder_ptr as usize..(placeholder_ptr + placeholder_len) as usize].to_vec();
                let initial = data[initial_ptr as usize..(initial_ptr + initial_len) as usize].to_vec();

                let placeholder_str = match std::str::from_utf8(&placeholder) {
                    Ok(s) => s.to_string(),
                    Err(_) => return -1,
                };
                let initial_str = match std::str::from_utf8(&initial) {
                    Ok(s) => s.to_string(),
                    Err(_) => return -1,
                };

                match caller.data_mut().ui_text_input(&placeholder_str, &initial_str) {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )?;

        // ui::button(label: string) -> u64
        linker.func_wrap(
            "monowiki:runtime/ui",
            "button",
            |mut caller: Caller<'_, RuntimeHost>, ptr: i32, len: i32| -> i64 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let data = mem.data(&caller);
                let label = data[ptr as usize..(ptr + len) as usize].to_vec();

                let label_str = match std::str::from_utf8(&label) {
                    Ok(s) => s.to_string(),
                    Err(_) => return -1,
                };

                match caller.data_mut().ui_button(&label_str) {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )?;

        // ui::show(value: list<u8>)
        linker.func_wrap(
            "monowiki:runtime/ui",
            "show",
            |mut caller: Caller<'_, RuntimeHost>, ptr: i32, len: i32| {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return,
                };

                let data = mem.data(&caller);
                let value = data[ptr as usize..(ptr + len) as usize].to_vec();
                let _ = caller.data_mut().ui_show(&value);
            },
        )?;

        Ok(())
    }

    /// Register diagnostics host functions
    fn register_diagnostics_imports(linker: &mut Linker<RuntimeHost>) -> Result<()> {
        // diagnostics::emit-diagnostic(severity: severity, span: span, message: string)
        linker.func_wrap(
            "monowiki:runtime/diagnostics",
            "emit-diagnostic",
            |mut caller: Caller<'_, RuntimeHost>,
             severity: i32,
             start_line: u32,
             start_col: u32,
             end_line: u32,
             end_col: u32,
             msg_ptr: i32,
             msg_len: i32| {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return,
                };

                let data = mem.data(&caller);
                let msg_bytes = data[msg_ptr as usize..(msg_ptr + msg_len) as usize].to_vec();

                let msg_str = match std::str::from_utf8(&msg_bytes) {
                    Ok(s) => s.to_string(),
                    Err(_) => return,
                };

                let sev = match severity {
                    0 => Severity::Error,
                    1 => Severity::Warning,
                    2 => Severity::Info,
                    3 => Severity::Hint,
                    _ => return,
                };

                let span = Span::new(start_line, start_col, end_line, end_col);
                let _ = caller.data_mut().emit_diagnostic(sev, span, &msg_str);
            },
        )?;

        // diagnostics::add-decoration(span: span, class: string)
        linker.func_wrap(
            "monowiki:runtime/diagnostics",
            "add-decoration",
            |mut caller: Caller<'_, RuntimeHost>,
             start_line: u32,
             start_col: u32,
             end_line: u32,
             end_col: u32,
             class_ptr: i32,
             class_len: i32| {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return,
                };

                let data = mem.data(&caller);
                let class_bytes = data[class_ptr as usize..(class_ptr + class_len) as usize].to_vec();

                let class_str = match std::str::from_utf8(&class_bytes) {
                    Ok(s) => s.to_string(),
                    Err(_) => return,
                };

                let span = Span::new(start_line, start_col, end_line, end_col);
                let _ = caller.data_mut().add_decoration(span, &class_str);
            },
        )?;

        Ok(())
    }

    /// Register fetch host functions (stub for now)
    fn register_fetch_imports(linker: &mut Linker<RuntimeHost>) -> Result<()> {
        // fetch::fetch(request: http-request) -> result<http-response, string>
        // This is complex due to async nature - leaving as stub for now
        // Will implement in a future iteration with async wasmtime support
        linker.func_wrap(
            "monowiki:runtime/fetch",
            "fetch",
            |_caller: Caller<'_, RuntimeHost>| -> i32 {
                // Return error code for now
                -1
            },
        )?;

        Ok(())
    }

    /// Register dataspace host functions
    fn register_dataspace_imports(linker: &mut Linker<RuntimeHost>) -> Result<()> {
        // dataspace::publish(pattern: string, value: list<u8>) -> u64
        linker.func_wrap(
            "monowiki:runtime/dataspace",
            "publish",
            |mut caller: Caller<'_, RuntimeHost>,
             pattern_ptr: i32,
             pattern_len: i32,
             value_ptr: i32,
             value_len: i32| -> i64 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let data = mem.data(&caller);
                let pattern_bytes =
                    &data[pattern_ptr as usize..(pattern_ptr + pattern_len) as usize];
                let value = &data[value_ptr as usize..(value_ptr + value_len) as usize];

                let pattern_str = match std::str::from_utf8(pattern_bytes) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                match caller.data_mut().dataspace_publish(pattern_str, value) {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )?;

        // dataspace::retract(assertion-id: u64)
        linker.func_wrap(
            "monowiki:runtime/dataspace",
            "retract",
            |mut caller: Caller<'_, RuntimeHost>, assertion_id: i64| {
                let _ = caller.data_mut().dataspace_retract(assertion_id as u64);
            },
        )?;

        // dataspace::subscribe(pattern: string, callback-id: u64) -> u64
        linker.func_wrap(
            "monowiki:runtime/dataspace",
            "subscribe",
            |mut caller: Caller<'_, RuntimeHost>,
             pattern_ptr: i32,
             pattern_len: i32,
             callback_id: i64| -> i64 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let data = mem.data(&caller);
                let pattern_bytes =
                    &data[pattern_ptr as usize..(pattern_ptr + pattern_len) as usize];

                let pattern_str = match std::str::from_utf8(pattern_bytes) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                match caller
                    .data_mut()
                    .dataspace_subscribe(pattern_str, callback_id as u64)
                {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )?;

        // dataspace::unsubscribe(subscription-id: u64)
        linker.func_wrap(
            "monowiki:runtime/dataspace",
            "unsubscribe",
            |mut caller: Caller<'_, RuntimeHost>, subscription_id: i64| {
                let _ = caller.data_mut().dataspace_unsubscribe(subscription_id as u64);
            },
        )?;

        Ok(())
    }

    /// Compile and instantiate a WASM module for a live cell
    ///
    /// # Arguments
    /// * `wasm_bytes` - The compiled WASM bytecode
    /// * `host` - The runtime host with capabilities and state
    ///
    /// # Returns
    /// A `LiveCellInstance` ready for execution
    pub fn instantiate(&self, wasm_bytes: &[u8], host: RuntimeHost) -> Result<LiveCellInstance> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, host);

        // Set resource limits
        store.limiter(|state| state as &mut dyn ResourceLimiter);

        let instance = self.linker.instantiate(&mut store, &module)?;

        Ok(LiveCellInstance { store, instance })
    }
}

impl Default for LiveCellEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default LiveCellEngine")
    }
}

/// An instantiated live cell WASM module
///
/// This represents a running live cell with its own memory and execution state.
pub struct LiveCellInstance {
    store: Store<RuntimeHost>,
    instance: Instance,
}

impl LiveCellInstance {
    /// Run the live cell's main function
    ///
    /// This executes the exported `run()` function from the WASM module.
    pub fn run(&mut self) -> Result<()> {
        let run = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "run")?;
        run.call(&mut self.store, ())?;
        Ok(())
    }

    /// Get immutable access to the host state
    ///
    /// This allows inspecting signals, widgets, diagnostics, etc. after execution.
    pub fn host(&self) -> &RuntimeHost {
        self.store.data()
    }

    /// Get mutable access to the host state
    ///
    /// This allows modifying host state or processing pending updates.
    pub fn host_mut(&mut self) -> &mut RuntimeHost {
        self.store.data_mut()
    }
}

/// Implement ResourceLimiter for RuntimeHost to provide memory limits
impl ResourceLimiter for RuntimeHost {
    fn memory_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> anyhow::Result<bool> {
        // Limit memory to 16MB
        const MAX_MEMORY: usize = 16 * 1024 * 1024;
        Ok(desired <= MAX_MEMORY)
    }

    fn table_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> anyhow::Result<bool> {
        // Limit table size to 10000 elements
        const MAX_TABLE_ELEMENTS: usize = 10000;
        Ok(desired <= MAX_TABLE_ELEMENTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = LiveCellEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_simple_wasm_module() {
        let engine = LiveCellEngine::new().unwrap();

        // A minimal WASM module that just exports a run function
        let wat = r#"
            (module
              (func (export "run"))
            )
        "#;
        let wasm = wat::parse_str(wat).unwrap();

        let host = RuntimeHost::with_default_capabilities();
        let mut instance = engine.instantiate(&wasm, host).unwrap();

        assert!(instance.run().is_ok());
    }

    #[test]
    fn test_memory_access() {
        let engine = LiveCellEngine::new().unwrap();

        // Module with memory
        let wat = r#"
            (module
              (memory (export "memory") 1)
              (func (export "run"))
            )
        "#;
        let wasm = wat::parse_str(wat).unwrap();

        let host = RuntimeHost::with_default_capabilities();
        let mut instance = engine.instantiate(&wasm, host).unwrap();

        assert!(instance.run().is_ok());
    }
}
