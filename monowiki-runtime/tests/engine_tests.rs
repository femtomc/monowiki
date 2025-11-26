//! Integration tests for the WASM runtime engine

use monowiki_runtime::{Capabilities, LiveCellEngine, RuntimeHost};

#[test]
fn test_load_and_run_wasm() {
    let engine = LiveCellEngine::new().unwrap();

    // Compile WAT to WASM
    let wat = include_str!("test_wasm.wat");
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let mut instance = engine.instantiate(&wasm, host).unwrap();

    instance.run().unwrap();

    // Check that ui.show was called
    let outputs = instance.host_mut().take_output();
    assert!(!outputs.is_empty());
    assert_eq!(outputs[0], b"Hello, World!");
}

#[test]
fn test_signal_create_and_set() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = include_str!("test_wasm.wat");
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let mut instance = engine.instantiate(&wasm, host).unwrap();

    instance.run().unwrap();

    // Check that signals were created
    assert!(instance.host().signals.len() > 0);
}

#[test]
fn test_signal_roundtrip() {
    let engine = LiveCellEngine::new().unwrap();

    // WASM that creates a signal, sets it, and reads it back
    let wat = r#"
        (module
          (import "monowiki:runtime/signals" "signal-create" (func $create (param i32 i32) (result i64)))
          (import "monowiki:runtime/signals" "signal-set" (func $set (param i64 i32 i32)))
          (import "monowiki:runtime/signals" "signal-get" (func $get (param i64 i32) (result i32)))
          (import "monowiki:runtime/ui" "show" (func $show (param i32 i32)))

          (memory (export "memory") 1)
          (data (i32.const 0) "\2a\00\00\00")  ;; 42 as i32 little-endian
          (data (i32.const 100) "\00\00\00\00") ;; buffer for reading back

          (func (export "run")
            (local $sig i64)
            ;; Create signal with initial value 42
            (local.set $sig (call $create (i32.const 0) (i32.const 4)))
            ;; Set signal to 42 again (redundant but tests the API)
            (call $set (local.get $sig) (i32.const 0) (i32.const 4))
            ;; Read signal value back into buffer at offset 100
            (drop (call $get (local.get $sig) (i32.const 100)))
            ;; Show the value we read back
            (call $show (i32.const 100) (i32.const 4))
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // Check that the signal value was read back correctly
    let outputs = instance.host_mut().take_output();
    assert_eq!(outputs.len(), 1);
    // The output should be the i32 value 42 in little-endian: [0x2a, 0x00, 0x00, 0x00]
    assert_eq!(outputs[0], vec![0x2a, 0x00, 0x00, 0x00]);
}

#[test]
fn test_ui_slider_creation() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = r#"
        (module
          (import "monowiki:runtime/ui" "slider" (func $slider (param f64 f64 f64) (result i64)))

          (memory (export "memory") 1)

          (func (export "run")
            ;; Create a slider from 0 to 100 with initial value 50
            (drop (call $slider (f64.const 0.0) (f64.const 100.0) (f64.const 50.0)))
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // Check that a widget was created
    assert_eq!(instance.host().widgets.len(), 1);
}

#[test]
fn test_ui_button_creation() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = r#"
        (module
          (import "monowiki:runtime/ui" "button" (func $button (param i32 i32) (result i64)))

          (memory (export "memory") 1)
          (data (i32.const 0) "Click me!")

          (func (export "run")
            ;; Create a button with label "Click me!"
            (drop (call $button (i32.const 0) (i32.const 9)))
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // Check that a widget was created
    assert_eq!(instance.host().widgets.len(), 1);
}

#[test]
fn test_diagnostics_emission() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = r#"
        (module
          (import "monowiki:runtime/diagnostics" "emit-diagnostic"
            (func $emit (param i32 i32 i32 i32 i32 i32 i32)))

          (memory (export "memory") 1)
          (data (i32.const 0) "Test error message")

          (func (export "run")
            ;; Emit an error diagnostic
            ;; severity=0 (Error), span=(1,0)-(1,10), message="Test error message"
            (call $emit
              (i32.const 0)    ;; severity: Error
              (i32.const 1)    ;; start_line
              (i32.const 0)    ;; start_col
              (i32.const 1)    ;; end_line
              (i32.const 10)   ;; end_col
              (i32.const 0)    ;; msg_ptr
              (i32.const 18))  ;; msg_len
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::new(Capabilities::new().with_diagnostics());
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // Check that a diagnostic was emitted
    assert_eq!(instance.host().diagnostics.diagnostic_count(), 1);
    assert!(instance.host().diagnostics.has_errors());
}

#[test]
fn test_multiple_signals() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = r#"
        (module
          (import "monowiki:runtime/signals" "signal-create" (func $create (param i32 i32) (result i64)))
          (import "monowiki:runtime/signals" "signal-set" (func $set (param i64 i32 i32)))

          (memory (export "memory") 1)
          (data (i32.const 0) "\01\00\00\00")
          (data (i32.const 4) "\02\00\00\00")
          (data (i32.const 8) "\03\00\00\00")

          (func (export "run")
            (local $sig1 i64)
            (local $sig2 i64)
            (local $sig3 i64)

            ;; Create three signals
            (local.set $sig1 (call $create (i32.const 0) (i32.const 4)))
            (local.set $sig2 (call $create (i32.const 4) (i32.const 4)))
            (local.set $sig3 (call $create (i32.const 8) (i32.const 4)))

            ;; Update the second signal
            (call $set (local.get $sig2) (i32.const 8) (i32.const 4))
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // Check that three signals were created
    assert_eq!(instance.host().signals.len(), 3);

    // Check that the second signal was updated (has pending update)
    let updates = instance.host_mut().process_signals();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].1, vec![0x03, 0x00, 0x00, 0x00]); // value 3
}

#[test]
fn test_capability_enforcement_no_ui() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = r#"
        (module
          (import "monowiki:runtime/ui" "slider" (func $slider (param f64 f64 f64) (result i64)))

          (memory (export "memory") 1)

          (func (export "run")
            ;; Try to create a slider without UI capability
            (drop (call $slider (f64.const 0.0) (f64.const 100.0) (f64.const 50.0)))
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    // Create host without UI capability
    let host = RuntimeHost::new(Capabilities::new());
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // The slider should return -1 (error) but won't crash
    // No widgets should be created
    assert_eq!(instance.host().widgets.len(), 0);
}

#[test]
fn test_dataspace_publish_with_capability() {
    let engine = LiveCellEngine::new().unwrap();

    let wat = r#"
        (module
          (import "monowiki:runtime/dataspace" "publish"
            (func $publish (param i32 i32 i32 i32) (result i64)))

          (memory (export "memory") 1)
          (data (i32.const 0) "test.pattern")
          (data (i32.const 20) "value data")

          (func (export "run")
            ;; Publish to dataspace
            (drop (call $publish
              (i32.const 0)   ;; pattern_ptr
              (i32.const 12)  ;; pattern_len
              (i32.const 20)  ;; value_ptr
              (i32.const 10)  ;; value_len
            ))
          )
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::new(Capabilities::new().with_dataspace());
    let mut instance = engine.instantiate(&wasm, host).unwrap();
    instance.run().unwrap();

    // Should succeed - no panic means it worked
}

#[test]
fn test_memory_limit() {
    let engine = LiveCellEngine::new().unwrap();

    // Module that tries to allocate too much memory
    let wat = r#"
        (module
          ;; Try to allocate 20MB (above the 16MB limit)
          (memory (export "memory") 320)  ;; 320 pages * 64KB = 20MB

          (func (export "run"))
        )
    "#;
    let wasm = wat::parse_str(wat).unwrap();

    let host = RuntimeHost::with_default_capabilities();
    let result = engine.instantiate(&wasm, host);

    // Should fail due to memory limit
    assert!(result.is_err());
}
