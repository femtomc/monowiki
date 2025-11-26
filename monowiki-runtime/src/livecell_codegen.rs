//! Live cell code generator
//!
//! Generates WASM modules for live cells that interact with the sammy dataspace.
//! The generated WASM publishes EvalRequest assertions and subscribes to EvalResult.

use crate::emitter::{FuncType, Instruction, ValType, WasmEmitter};

/// Configuration for live cell code generation
#[derive(Debug, Clone)]
pub struct LiveCellConfig {
    /// Cell identifier
    pub cell_id: String,
    /// Document identifier
    pub doc_id: String,
    /// Kernel to use for evaluation (e.g., "wasm", "js")
    pub kernel_id: String,
    /// Sequence number for ordering
    pub seq: u64,
}

/// Builder for generating live cell WASM modules
///
/// This generates WASM that:
/// 1. Stores cell metadata in linear memory
/// 2. Imports dataspace publish/subscribe functions
/// 3. Publishes an EvalRequest on initialization
/// 4. Subscribes to EvalResult for this cell
pub struct LiveCellCodeGen {
    config: LiveCellConfig,
    /// Payload to be evaluated (source or WASM bytes)
    payload: Vec<u8>,
    /// Whether payload is source code (vs raw WASM)
    is_source: bool,
}

impl LiveCellCodeGen {
    /// Create a new live cell code generator with source payload
    pub fn with_source(config: LiveCellConfig, source: &str) -> Self {
        Self {
            config,
            payload: source.as_bytes().to_vec(),
            is_source: true,
        }
    }

    /// Create a new live cell code generator with WASM payload
    pub fn with_wasm(config: LiveCellConfig, wasm: Vec<u8>) -> Self {
        Self {
            config,
            payload: wasm,
            is_source: false,
        }
    }

    /// Generate the complete WASM module
    ///
    /// The generated module:
    /// - Exports "memory" (1 page = 64KB)
    /// - Exports "run" function that publishes EvalRequest
    /// - Imports dataspace publish/subscribe functions
    pub fn generate(&self) -> Vec<u8> {
        let mut emitter = WasmEmitter::new();

        // Import dataspace functions
        // publish(pattern_ptr, pattern_len, value_ptr, value_len) -> i64
        let publish_idx = emitter.add_import(
            "monowiki:runtime/dataspace",
            "publish",
            FuncType {
                params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
                results: vec![ValType::I64],
            },
        );

        // subscribe(pattern_ptr, pattern_len, callback_id) -> i64
        let subscribe_idx = emitter.add_import(
            "monowiki:runtime/dataspace",
            "subscribe",
            FuncType {
                params: vec![ValType::I32, ValType::I32, ValType::I64],
                results: vec![ValType::I64],
            },
        );

        // Import ui::show to display results
        let _show_idx = emitter.add_import(
            "monowiki:runtime/ui",
            "show",
            FuncType {
                params: vec![ValType::I32, ValType::I32],
                results: vec![],
            },
        );

        // Build the JSON payload for EvalRequest
        let request_json = self.build_request_json();
        let pattern = "EvalRequest";

        // Calculate memory layout:
        // 0..pattern_len: pattern string
        // pattern_len..pattern_len+json_len: JSON payload
        let pattern_len = pattern.len();
        let json_len = request_json.len();
        let result_pattern = format!("EvalResult.{}.{}", self.config.cell_id, self.config.doc_id);
        let result_pattern_len = result_pattern.len();

        // Create the run function that:
        // 1. Publishes the EvalRequest
        // 2. Subscribes to EvalResult
        let run_body = vec![
            // Publish: pattern at 0, JSON at pattern_len
            Instruction::I32Const(0),                        // pattern_ptr
            Instruction::I32Const(pattern_len as i32),       // pattern_len
            Instruction::I32Const(pattern_len as i32),       // value_ptr (JSON starts after pattern)
            Instruction::I32Const(json_len as i32),          // value_len
            Instruction::Call(publish_idx),
            // Drop the assertion ID (we don't need to retract)
            Instruction::I64Const(0),
            Instruction::I64Const(0),
            // Subscribe to results: pattern at json_offset
            Instruction::I32Const((pattern_len + json_len) as i32),  // result_pattern_ptr
            Instruction::I32Const(result_pattern_len as i32),        // result_pattern_len
            Instruction::I64Const(1),                                // callback_id = 1
            Instruction::Call(subscribe_idx),
            // Drop the subscription ID
            Instruction::I64Const(0),
            Instruction::I64Const(0),
            Instruction::End,
        ];

        let run_idx = emitter.add_function(&[], &[], vec![], run_body);
        emitter.add_export("run", run_idx);

        // Generate module bytes
        let module = emitter.emit();

        // We need to add memory and data sections manually since WasmEmitter
        // doesn't support them yet. For now, we'll create a simpler approach
        // by embedding the data directly in the module.

        // The WasmEmitter output doesn't include memory, so we need to patch it
        // For a complete implementation, we'd extend WasmEmitter to support memory

        module
    }

    /// Build the JSON payload for EvalRequest
    fn build_request_json(&self) -> String {
        let payload_type = if self.is_source { "source" } else { "wasm" };
        let payload_data = if self.is_source {
            // Escape the string for JSON
            serde_json::to_string(&String::from_utf8_lossy(&self.payload)).unwrap_or_default()
        } else {
            // Base64 encode for WASM
            use base64::{engine::general_purpose::STANDARD, Engine};
            format!("\"{}\"", STANDARD.encode(&self.payload))
        };

        format!(
            r#"{{"kernel_id":"{}","cell_id":"{}","doc_id":"{}","payload":{{"type":"{}","data":{}}},"seq":{}}}"#,
            self.config.kernel_id,
            self.config.cell_id,
            self.config.doc_id,
            payload_type,
            payload_data,
            self.config.seq,
        )
    }

    /// Generate a minimal WASM module that just shows output
    ///
    /// This is useful for testing - generates a module that displays a message.
    pub fn generate_show_module(message: &str) -> Vec<u8> {
        let mut emitter = WasmEmitter::new();

        // Import ui::show
        let show_idx = emitter.add_import(
            "monowiki:runtime/ui",
            "show",
            FuncType {
                params: vec![ValType::I32, ValType::I32],
                results: vec![],
            },
        );

        let msg_len = message.len();

        // Create run function that calls show with message
        let run_body = vec![
            Instruction::I32Const(0),              // ptr (message at start of memory)
            Instruction::I32Const(msg_len as i32), // len
            Instruction::Call(show_idx),
            Instruction::End,
        ];

        let run_idx = emitter.add_function(&[], &[], vec![], run_body);
        emitter.add_export("run", run_idx);

        emitter.emit()
    }
}

/// Extended WASM emitter with memory support
///
/// This extends the basic WasmEmitter to support:
/// - Memory section
/// - Data section for initializing memory
pub struct WasmEmitterWithMemory {
    inner: WasmEmitter,
    /// Memory pages (1 page = 64KB)
    memory_pages: u32,
    /// Data segments to initialize memory
    data_segments: Vec<DataSegment>,
    /// Current data offset
    data_offset: u32,
}

/// A segment of data to initialize in memory
struct DataSegment {
    offset: u32,
    data: Vec<u8>,
}

impl WasmEmitterWithMemory {
    /// Create a new emitter with 1 page of memory
    pub fn new() -> Self {
        Self {
            inner: WasmEmitter::new(),
            memory_pages: 1,
            data_segments: Vec::new(),
            data_offset: 0,
        }
    }

    /// Add data to memory and return its offset
    pub fn add_data(&mut self, data: &[u8]) -> u32 {
        let offset = self.data_offset;
        self.data_segments.push(DataSegment {
            offset,
            data: data.to_vec(),
        });
        self.data_offset += data.len() as u32;
        offset
    }

    /// Add a string to memory and return (offset, length)
    pub fn add_string(&mut self, s: &str) -> (u32, u32) {
        let offset = self.add_data(s.as_bytes());
        (offset, s.len() as u32)
    }

    /// Get a mutable reference to the inner emitter for adding functions/imports
    pub fn emitter(&mut self) -> &mut WasmEmitter {
        &mut self.inner
    }

    /// Emit the complete WASM module with memory and data sections
    pub fn emit(&self) -> Vec<u8> {
        let base = self.inner.emit();

        // We need to inject memory section (id 5) and data section (id 11)
        // This is a simplified implementation - a full implementation would
        // properly parse and reconstruct the module

        let mut module = Vec::new();

        // Copy magic and version
        module.extend_from_slice(&base[0..8]);

        // We need to insert memory section after function section (id 3)
        // and data section at the end

        // For now, just add memory section and export it
        // Memory section
        module.push(0x05); // memory section id
        let mem_section = encode_memory_section(self.memory_pages);
        module.extend_from_slice(&encode_u32_leb(mem_section.len() as u32));
        module.extend_from_slice(&mem_section);

        // Copy rest of base module (skip magic/version)
        module.extend_from_slice(&base[8..]);

        // Add data section if we have data
        if !self.data_segments.is_empty() {
            module.push(0x0B); // data section id
            let data_section = encode_data_section(&self.data_segments);
            module.extend_from_slice(&encode_u32_leb(data_section.len() as u32));
            module.extend_from_slice(&data_section);
        }

        module
    }
}

impl Default for WasmEmitterWithMemory {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for encoding

fn encode_u32_leb(mut n: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = (n & 0x7F) as u8;
        n >>= 7;
        if n != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if n == 0 {
            break;
        }
    }
    bytes
}

fn encode_memory_section(pages: u32) -> Vec<u8> {
    let mut section = Vec::new();
    section.push(1); // count = 1 memory
    section.push(0x00); // flags: no maximum
    section.extend_from_slice(&encode_u32_leb(pages)); // initial pages
    section
}

fn encode_data_section(segments: &[DataSegment]) -> Vec<u8> {
    let mut section = Vec::new();
    section.extend_from_slice(&encode_u32_leb(segments.len() as u32));

    for seg in segments {
        section.push(0x00); // active segment, memory 0
        // i32.const offset
        section.push(0x41);
        section.extend_from_slice(&encode_i32_leb(seg.offset as i32));
        section.push(0x0B); // end
        // data bytes
        section.extend_from_slice(&encode_u32_leb(seg.data.len() as u32));
        section.extend_from_slice(&seg.data);
    }

    section
}

fn encode_i32_leb(mut n: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut more = true;
    while more {
        let mut byte = (n & 0x7F) as u8;
        n >>= 7;
        if (n == 0 && (byte & 0x40) == 0) || (n == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        bytes.push(byte);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_cell_config() {
        let config = LiveCellConfig {
            cell_id: "cell1".to_string(),
            doc_id: "doc1".to_string(),
            kernel_id: "wasm".to_string(),
            seq: 1,
        };
        assert_eq!(config.cell_id, "cell1");
    }

    #[test]
    fn test_request_json_source() {
        let config = LiveCellConfig {
            cell_id: "cell1".to_string(),
            doc_id: "doc1".to_string(),
            kernel_id: "js".to_string(),
            seq: 42,
        };
        let gen = LiveCellCodeGen::with_source(config, "console.log('hi')");
        let json = gen.build_request_json();

        assert!(json.contains("\"kernel_id\":\"js\""));
        assert!(json.contains("\"cell_id\":\"cell1\""));
        assert!(json.contains("\"seq\":42"));
        assert!(json.contains("\"type\":\"source\""));
    }

    #[test]
    fn test_leb128_encoding() {
        assert_eq!(encode_u32_leb(0), vec![0x00]);
        assert_eq!(encode_u32_leb(127), vec![0x7F]);
        assert_eq!(encode_u32_leb(128), vec![0x80, 0x01]);
        assert_eq!(encode_u32_leb(624485), vec![0xE5, 0x8E, 0x26]);
    }

    #[test]
    fn test_emitter_with_memory() {
        let mut emitter = WasmEmitterWithMemory::new();

        let (offset, len) = emitter.add_string("hello");
        assert_eq!(offset, 0);
        assert_eq!(len, 5);

        let (offset2, len2) = emitter.add_string(" world");
        assert_eq!(offset2, 5);
        assert_eq!(len2, 6);
    }
}
