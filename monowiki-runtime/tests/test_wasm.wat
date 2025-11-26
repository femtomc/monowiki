;; Test WASM module for live cell execution
;; This module demonstrates basic signal and UI operations

(module
  ;; Import host functions from the signals interface
  (import "monowiki:runtime/signals" "signal-create" (func $signal_create (param i32 i32) (result i64)))
  (import "monowiki:runtime/signals" "signal-get" (func $signal_get (param i64 i32) (result i32)))
  (import "monowiki:runtime/signals" "signal-set" (func $signal_set (param i64 i32 i32)))

  ;; Import host functions from the ui interface
  (import "monowiki:runtime/ui" "show" (func $ui_show (param i32 i32)))
  (import "monowiki:runtime/ui" "slider" (func $ui_slider (param f64 f64 f64) (result i64)))

  ;; Memory for data
  (memory (export "memory") 1)

  ;; Data section with test values
  (data (i32.const 0) "\00\00\00\00")          ;; 4 bytes for i32 = 0
  (data (i32.const 4) "Hello, World!")         ;; 13 bytes string
  (data (i32.const 20) "\2a\00\00\00")         ;; 4 bytes for i32 = 42

  ;; Main entry point - creates a signal and shows a value
  (func (export "run")
    (local $sig i64)

    ;; Create a signal with initial value 0 (at offset 0, length 4)
    (local.set $sig
      (call $signal_create (i32.const 0) (i32.const 4)))

    ;; Show "Hello, World!" (at offset 4, length 13)
    (call $ui_show (i32.const 4) (i32.const 13))

    ;; Set the signal to 42 (at offset 20, length 4)
    (call $signal_set (local.get $sig) (i32.const 20) (i32.const 4))

    ;; Create a slider from 0 to 100, initial value 50
    (drop (call $ui_slider (f64.const 0.0) (f64.const 100.0) (f64.const 50.0)))
  )
)
