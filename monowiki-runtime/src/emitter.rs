//! Minimal WASM bytecode emitter for live cells
//!
//! This module provides a simple WASM bytecode emitter for compiling
//! trivial MRL expressions to WASM. For complex expressions, we fall back
//! to an interpreter.

use std::collections::HashMap;

/// WASM value types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
}

impl ValType {
    pub fn to_byte(&self) -> u8 {
        match self {
            ValType::I32 => 0x7F,
            ValType::I64 => 0x7E,
            ValType::F32 => 0x7D,
            ValType::F64 => 0x7C,
        }
    }
}

/// Function type signature
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

/// WASM instruction
#[derive(Debug, Clone)]
pub enum Instruction {
    // Constants
    I32Const(i32),
    I64Const(i64),
    F32Const(f32),
    F64Const(f64),

    // Arithmetic
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32RemS,

    F64Add,
    F64Sub,
    F64Mul,
    F64Div,

    // Local variables
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),

    // Control flow
    Call(u32),
    Return,

    // Blocks
    Block(ValType),
    Loop(ValType),
    If(ValType),
    Else,
    End,
}

impl Instruction {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Instruction::I32Const(n) => {
                let mut bytes = vec![0x41]; // i32.const opcode
                bytes.extend_from_slice(&encode_i32(*n));
                bytes
            }
            Instruction::I64Const(n) => {
                let mut bytes = vec![0x42]; // i64.const opcode
                bytes.extend_from_slice(&encode_i64(*n));
                bytes
            }
            Instruction::F32Const(f) => {
                let mut bytes = vec![0x43]; // f32.const opcode
                bytes.extend_from_slice(&f.to_le_bytes());
                bytes
            }
            Instruction::F64Const(f) => {
                let mut bytes = vec![0x44]; // f64.const opcode
                bytes.extend_from_slice(&f.to_le_bytes());
                bytes
            }
            Instruction::I32Add => vec![0x6A],
            Instruction::I32Sub => vec![0x6B],
            Instruction::I32Mul => vec![0x6C],
            Instruction::I32DivS => vec![0x6D],
            Instruction::I32RemS => vec![0x6F],
            Instruction::F64Add => vec![0xA0],
            Instruction::F64Sub => vec![0xA1],
            Instruction::F64Mul => vec![0xA2],
            Instruction::F64Div => vec![0xA3],
            Instruction::LocalGet(idx) => {
                let mut bytes = vec![0x20]; // local.get opcode
                bytes.extend_from_slice(&encode_u32(*idx));
                bytes
            }
            Instruction::LocalSet(idx) => {
                let mut bytes = vec![0x21]; // local.set opcode
                bytes.extend_from_slice(&encode_u32(*idx));
                bytes
            }
            Instruction::LocalTee(idx) => {
                let mut bytes = vec![0x22]; // local.tee opcode
                bytes.extend_from_slice(&encode_u32(*idx));
                bytes
            }
            Instruction::Call(idx) => {
                let mut bytes = vec![0x10]; // call opcode
                bytes.extend_from_slice(&encode_u32(*idx));
                bytes
            }
            Instruction::Return => vec![0x0F],
            Instruction::Block(vt) => vec![0x02, vt.to_byte()],
            Instruction::Loop(vt) => vec![0x03, vt.to_byte()],
            Instruction::If(vt) => vec![0x04, vt.to_byte()],
            Instruction::Else => vec![0x05],
            Instruction::End => vec![0x0B],
        }
    }
}

/// Import description
#[derive(Debug, Clone)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub type_idx: u32,
}

/// Export description
#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum ExportKind {
    Func = 0,
}

/// Function body
#[derive(Debug, Clone)]
pub struct FuncBody {
    pub locals: Vec<ValType>,
    pub instructions: Vec<Instruction>,
}

/// Simple WASM module emitter
///
/// This emitter can generate minimal WASM modules for simple live cell expressions.
/// For complex expressions with control flow or multiple statements, we fall back
/// to the interpreter.
#[derive(Debug)]
pub struct WasmEmitter {
    types: Vec<FuncType>,
    imports: Vec<Import>,
    functions: Vec<u32>, // type indices
    exports: Vec<Export>,
    bodies: Vec<FuncBody>,
    type_map: HashMap<FuncType, u32>,
}

impl WasmEmitter {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            exports: Vec::new(),
            bodies: Vec::new(),
            type_map: HashMap::new(),
        }
    }

    /// Add or get a function type
    pub fn add_type(&mut self, func_type: FuncType) -> u32 {
        if let Some(&idx) = self.type_map.get(&func_type) {
            return idx;
        }

        let idx = self.types.len() as u32;
        self.types.push(func_type.clone());
        self.type_map.insert(func_type, idx);
        idx
    }

    /// Add an import
    pub fn add_import(&mut self, module: &str, name: &str, ty: FuncType) -> u32 {
        let type_idx = self.add_type(ty);
        let import_idx = self.imports.len() as u32;

        self.imports.push(Import {
            module: module.to_string(),
            name: name.to_string(),
            type_idx,
        });

        import_idx
    }

    /// Add a function
    pub fn add_function(
        &mut self,
        params: &[ValType],
        results: &[ValType],
        locals: Vec<ValType>,
        body: Vec<Instruction>,
    ) -> u32 {
        let func_type = FuncType {
            params: params.to_vec(),
            results: results.to_vec(),
        };
        let type_idx = self.add_type(func_type);

        let func_idx = self.imports.len() as u32 + self.functions.len() as u32;

        self.functions.push(type_idx);
        self.bodies.push(FuncBody {
            locals,
            instructions: body,
        });

        func_idx
    }

    /// Add an export
    pub fn add_export(&mut self, name: &str, func_idx: u32) {
        self.exports.push(Export {
            name: name.to_string(),
            kind: ExportKind::Func,
            index: func_idx,
        });
    }

    /// Emit the complete WASM module
    pub fn emit(&self) -> Vec<u8> {
        let mut module = Vec::new();

        // Magic number and version
        module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // \0asm
        module.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version 1

        // Type section
        if !self.types.is_empty() {
            module.push(0x01); // section id
            let section = encode_type_section(&self.types);
            module.extend_from_slice(&encode_u32(section.len() as u32));
            module.extend_from_slice(&section);
        }

        // Import section
        if !self.imports.is_empty() {
            module.push(0x02); // section id
            let section = encode_import_section(&self.imports);
            module.extend_from_slice(&encode_u32(section.len() as u32));
            module.extend_from_slice(&section);
        }

        // Function section
        if !self.functions.is_empty() {
            module.push(0x03); // section id
            let section = encode_function_section(&self.functions);
            module.extend_from_slice(&encode_u32(section.len() as u32));
            module.extend_from_slice(&section);
        }

        // Export section
        if !self.exports.is_empty() {
            module.push(0x07); // section id
            let section = encode_export_section(&self.exports);
            module.extend_from_slice(&encode_u32(section.len() as u32));
            module.extend_from_slice(&section);
        }

        // Code section
        if !self.bodies.is_empty() {
            module.push(0x0A); // section id
            let section = encode_code_section(&self.bodies);
            module.extend_from_slice(&encode_u32(section.len() as u32));
            module.extend_from_slice(&section);
        }

        module
    }
}

impl Default for WasmEmitter {
    fn default() -> Self {
        Self::new()
    }
}

// ===== Encoding helpers =====

fn encode_u32(n: u32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut value = n;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        result.push(byte);
        if value == 0 {
            break;
        }
    }
    result
}

fn encode_i32(n: i32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut value = n;
    let mut more = true;
    while more {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        result.push(byte);
    }
    result
}

fn encode_i64(n: i64) -> Vec<u8> {
    let mut result = Vec::new();
    let mut value = n;
    let mut more = true;
    while more {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        result.push(byte);
    }
    result
}

fn encode_type_section(types: &[FuncType]) -> Vec<u8> {
    let mut section = Vec::new();
    section.extend_from_slice(&encode_u32(types.len() as u32));

    for ty in types {
        section.push(0x60); // func type
        section.extend_from_slice(&encode_u32(ty.params.len() as u32));
        for param in &ty.params {
            section.push(param.to_byte());
        }
        section.extend_from_slice(&encode_u32(ty.results.len() as u32));
        for result in &ty.results {
            section.push(result.to_byte());
        }
    }

    section
}

fn encode_import_section(imports: &[Import]) -> Vec<u8> {
    let mut section = Vec::new();
    section.extend_from_slice(&encode_u32(imports.len() as u32));

    for import in imports {
        // Module name
        section.extend_from_slice(&encode_u32(import.module.len() as u32));
        section.extend_from_slice(import.module.as_bytes());

        // Field name
        section.extend_from_slice(&encode_u32(import.name.len() as u32));
        section.extend_from_slice(import.name.as_bytes());

        // Import kind (func = 0)
        section.push(0x00);
        section.extend_from_slice(&encode_u32(import.type_idx));
    }

    section
}

fn encode_function_section(functions: &[u32]) -> Vec<u8> {
    let mut section = Vec::new();
    section.extend_from_slice(&encode_u32(functions.len() as u32));

    for &type_idx in functions {
        section.extend_from_slice(&encode_u32(type_idx));
    }

    section
}

fn encode_export_section(exports: &[Export]) -> Vec<u8> {
    let mut section = Vec::new();
    section.extend_from_slice(&encode_u32(exports.len() as u32));

    for export in exports {
        // Name
        section.extend_from_slice(&encode_u32(export.name.len() as u32));
        section.extend_from_slice(export.name.as_bytes());

        // Kind
        section.push(export.kind as u8);

        // Index
        section.extend_from_slice(&encode_u32(export.index));
    }

    section
}

fn encode_code_section(bodies: &[FuncBody]) -> Vec<u8> {
    let mut section = Vec::new();
    section.extend_from_slice(&encode_u32(bodies.len() as u32));

    for body in bodies {
        let mut func_bytes = Vec::new();

        // Locals
        func_bytes.extend_from_slice(&encode_u32(body.locals.len() as u32));
        for local in &body.locals {
            func_bytes.extend_from_slice(&encode_u32(1)); // count
            func_bytes.push(local.to_byte());
        }

        // Instructions
        for instr in &body.instructions {
            func_bytes.extend_from_slice(&instr.encode());
        }

        // Size prefix
        section.extend_from_slice(&encode_u32(func_bytes.len() as u32));
        section.extend_from_slice(&func_bytes);
    }

    section
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_module() {
        let emitter = WasmEmitter::new();
        let module = emitter.emit();

        // Should have magic number and version
        assert_eq!(&module[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        assert_eq!(&module[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_simple_function() {
        let mut emitter = WasmEmitter::new();

        // Function that returns i32.const 42
        let func_idx = emitter.add_function(
            &[],
            &[ValType::I32],
            vec![],
            vec![Instruction::I32Const(42), Instruction::End],
        );

        emitter.add_export("main", func_idx);

        let module = emitter.emit();

        // Should have magic number
        assert_eq!(&module[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    }

    #[test]
    fn test_function_with_import() {
        let mut emitter = WasmEmitter::new();

        // Import a function
        emitter.add_import(
            "env",
            "log",
            FuncType {
                params: vec![ValType::I32],
                results: vec![],
            },
        );

        // Function that calls the import
        let func_idx = emitter.add_function(
            &[],
            &[],
            vec![],
            vec![
                Instruction::I32Const(42),
                Instruction::Call(0), // Call import 0
                Instruction::End,
            ],
        );

        emitter.add_export("main", func_idx);

        let module = emitter.emit();

        // Should have magic number
        assert_eq!(&module[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    }

    #[test]
    fn test_encode_u32() {
        assert_eq!(encode_u32(0), vec![0x00]);
        assert_eq!(encode_u32(1), vec![0x01]);
        assert_eq!(encode_u32(127), vec![0x7F]);
        assert_eq!(encode_u32(128), vec![0x80, 0x01]);
    }

    #[test]
    fn test_encode_i32() {
        assert_eq!(encode_i32(0), vec![0x00]);
        assert_eq!(encode_i32(1), vec![0x01]);
        assert_eq!(encode_i32(-1), vec![0x7F]);
        assert_eq!(encode_i32(42), vec![0x2A]);
    }
}
