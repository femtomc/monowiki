//! Interpreter fallback for trivial expressions
//!
//! This module provides an interpreter for simple MRL expressions that
//! don't require full WASM compilation. For complex expressions with
//! control flow, we use the WASM emitter instead.

use crate::abi::{RuntimeError, RuntimeResult};
use crate::host::RuntimeHost;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simple expression AST for interpreter
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleExpr {
    /// Constant value
    Const(Value),

    /// Variable reference
    Var(String),

    /// Binary operation
    BinOp(Box<SimpleExpr>, BinOp, Box<SimpleExpr>),

    /// Unary operation
    UnOp(UnOp, Box<SimpleExpr>),

    /// Field access
    FieldAccess(Box<SimpleExpr>, String),

    /// Function/method call
    Call(String, Vec<SimpleExpr>),

    /// Method call on an object
    MethodCall(Box<SimpleExpr>, String, Vec<SimpleExpr>),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

/// Statement in a live cell body
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Variable assignment
    Assign(String, SimpleExpr),

    /// Expression statement
    Expr(SimpleExpr),
}

/// Runtime value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Value {
    pub fn as_bool(&self) -> RuntimeResult<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err(RuntimeError::SerializationError(format!(
                "Expected bool, got {:?}",
                self
            ))),
        }
    }

    pub fn as_int(&self) -> RuntimeResult<i64> {
        match self {
            Value::Int(i) => Ok(*i),
            _ => Err(RuntimeError::SerializationError(format!(
                "Expected int, got {:?}",
                self
            ))),
        }
    }

    pub fn as_float(&self) -> RuntimeResult<f64> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Int(i) => Ok(*i as f64),
            _ => Err(RuntimeError::SerializationError(format!(
                "Expected float, got {:?}",
                self
            ))),
        }
    }

    pub fn as_string(&self) -> RuntimeResult<String> {
        match self {
            Value::String(s) => Ok(s.clone()),
            _ => Err(RuntimeError::SerializationError(format!(
                "Expected string, got {:?}",
                self
            ))),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::None => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
        }
    }
}

/// Interpreter for simple expressions
///
/// This interpreter can handle trivial expressions without control flow.
/// For complex expressions, use the WASM emitter instead.
pub struct Interpreter {
    pub(crate) host: RuntimeHost,
    pub(crate) locals: HashMap<String, Value>,
}

impl Interpreter {
    pub fn new(host: RuntimeHost) -> Self {
        Self {
            host,
            locals: HashMap::new(),
        }
    }

    /// Evaluate an expression
    pub fn eval(&mut self, expr: &SimpleExpr) -> RuntimeResult<Value> {
        match expr {
            SimpleExpr::Const(v) => Ok(v.clone()),

            SimpleExpr::Var(name) => self
                .locals
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::WasmError(format!("Undefined variable: {}", name))),

            SimpleExpr::BinOp(lhs, op, rhs) => {
                let lhs_val = self.eval(lhs)?;
                let rhs_val = self.eval(rhs)?;
                self.eval_binop(lhs_val, *op, rhs_val)
            }

            SimpleExpr::UnOp(op, operand) => {
                let val = self.eval(operand)?;
                self.eval_unop(*op, val)
            }

            SimpleExpr::FieldAccess(obj, field) => {
                let obj_val = self.eval(obj)?;
                match obj_val {
                    Value::Object(ref map) => map
                        .get(field)
                        .cloned()
                        .ok_or_else(|| {
                            RuntimeError::WasmError(format!("Field not found: {}", field))
                        }),
                    _ => Err(RuntimeError::WasmError(format!(
                        "Cannot access field on non-object: {:?}",
                        obj_val
                    ))),
                }
            }

            SimpleExpr::Call(name, args) => {
                let arg_vals: Result<Vec<_>, _> = args.iter().map(|e| self.eval(e)).collect();
                self.eval_call(name, arg_vals?)
            }

            SimpleExpr::MethodCall(obj, method, args) => {
                let obj_val = self.eval(obj)?;
                let arg_vals: Result<Vec<_>, _> = args.iter().map(|e| self.eval(e)).collect();
                self.eval_method(obj_val, method, arg_vals?)
            }
        }
    }

    /// Evaluate a binary operation
    fn eval_binop(&self, lhs: Value, op: BinOp, rhs: Value) -> RuntimeResult<Value> {
        match op {
            BinOp::Add => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + b as f64)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                _ => Err(RuntimeError::WasmError("Invalid operands for +".to_string())),
            },

            BinOp::Sub => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),
                _ => Err(RuntimeError::WasmError("Invalid operands for -".to_string())),
            },

            BinOp::Mul => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),
                _ => Err(RuntimeError::WasmError("Invalid operands for *".to_string())),
            },

            BinOp::Div => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => {
                    if b == 0 {
                        Err(RuntimeError::WasmError("Division by zero".to_string()))
                    } else {
                        Ok(Value::Int(a / b))
                    }
                }
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 / b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / b as f64)),
                _ => Err(RuntimeError::WasmError("Invalid operands for /".to_string())),
            },

            BinOp::Rem => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => {
                    if b == 0 {
                        Err(RuntimeError::WasmError("Modulo by zero".to_string()))
                    } else {
                        Ok(Value::Int(a % b))
                    }
                }
                _ => Err(RuntimeError::WasmError("Invalid operands for %".to_string())),
            },

            BinOp::Eq => Ok(Value::Bool(lhs == rhs)),
            BinOp::Ne => Ok(Value::Bool(lhs != rhs)),

            BinOp::Lt => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
                _ => Err(RuntimeError::WasmError("Invalid operands for <".to_string())),
            },

            BinOp::Le => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
                _ => Err(RuntimeError::WasmError(
                    "Invalid operands for <=".to_string(),
                )),
            },

            BinOp::Gt => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
                _ => Err(RuntimeError::WasmError("Invalid operands for >".to_string())),
            },

            BinOp::Ge => match (lhs, rhs) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
                _ => Err(RuntimeError::WasmError(
                    "Invalid operands for >=".to_string(),
                )),
            },

            BinOp::And => Ok(Value::Bool(lhs.is_truthy() && rhs.is_truthy())),
            BinOp::Or => Ok(Value::Bool(lhs.is_truthy() || rhs.is_truthy())),
        }
    }

    /// Evaluate a unary operation
    fn eval_unop(&self, op: UnOp, operand: Value) -> RuntimeResult<Value> {
        match op {
            UnOp::Neg => match operand {
                Value::Int(i) => Ok(Value::Int(-i)),
                Value::Float(f) => Ok(Value::Float(-f)),
                _ => Err(RuntimeError::WasmError(
                    "Invalid operand for negation".to_string(),
                )),
            },

            UnOp::Not => Ok(Value::Bool(!operand.is_truthy())),
        }
    }

    /// Evaluate a function call
    fn eval_call(&mut self, name: &str, args: Vec<Value>) -> RuntimeResult<Value> {
        // Built-in functions
        match name {
            "str" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WasmError("str() takes 1 argument".to_string()));
                }
                Ok(Value::String(format!("{:?}", args[0])))
            }

            "len" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WasmError("len() takes 1 argument".to_string()));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::Int(s.len() as i64)),
                    Value::Array(a) => Ok(Value::Int(a.len() as i64)),
                    _ => Err(RuntimeError::WasmError(
                        "len() requires string or array".to_string(),
                    )),
                }
            }

            _ => Err(RuntimeError::WasmError(format!(
                "Unknown function: {}",
                name
            ))),
        }
    }

    /// Evaluate a method call
    fn eval_method(&mut self, _obj: Value, method: &str, _args: Vec<Value>) -> RuntimeResult<Value> {
        // Stub - method calls not yet implemented
        Err(RuntimeError::WasmError(format!(
            "Method calls not yet implemented: {}",
            method
        )))
    }

    /// Execute a statement
    pub fn exec_stmt(&mut self, stmt: &Stmt) -> RuntimeResult<()> {
        match stmt {
            Stmt::Assign(name, expr) => {
                let value = self.eval(expr)?;
                self.locals.insert(name.clone(), value);
                Ok(())
            }

            Stmt::Expr(expr) => {
                self.eval(expr)?;
                Ok(())
            }
        }
    }

    /// Execute a live cell body (multiple statements)
    pub fn eval_live_cell(&mut self, body: &[Stmt]) -> RuntimeResult<()> {
        for stmt in body {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    /// Get a local variable value
    pub fn get_local(&self, name: &str) -> Option<&Value> {
        self.locals.get(name)
    }

    /// Set a local variable value
    pub fn set_local(&mut self, name: String, value: Value) {
        self.locals.insert(name, value);
    }

    /// Clear all local variables
    pub fn clear_locals(&mut self) {
        self.locals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::Capabilities;

    #[test]
    fn test_const_eval() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let expr = SimpleExpr::Const(Value::Int(42));
        let result = interp.eval(&expr).unwrap();

        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_binop_add() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Const(Value::Int(40))),
            BinOp::Add,
            Box::new(SimpleExpr::Const(Value::Int(2))),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_binop_mul() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Const(Value::Int(6))),
            BinOp::Mul,
            Box::new(SimpleExpr::Const(Value::Int(7))),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_variable_assignment() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let stmt = Stmt::Assign("x".to_string(), SimpleExpr::Const(Value::Int(42)));
        interp.exec_stmt(&stmt).unwrap();

        assert_eq!(interp.get_local("x"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_variable_reference() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        interp.set_local("x".to_string(), Value::Int(42));

        let expr = SimpleExpr::Var("x".to_string());
        let result = interp.eval(&expr).unwrap();

        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_string_concat() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Const(Value::String("Hello, ".to_string()))),
            BinOp::Add,
            Box::new(SimpleExpr::Const(Value::String("World!".to_string()))),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::String("Hello, World!".to_string()));
    }

    #[test]
    fn test_comparison_ops() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Const(Value::Int(5))),
            BinOp::Lt,
            Box::new(SimpleExpr::Const(Value::Int(10))),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_logical_ops() {
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Const(Value::Bool(true))),
            BinOp::And,
            Box::new(SimpleExpr::Const(Value::Bool(false))),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }
}
