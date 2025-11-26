//! Integration tests for monowiki-runtime

#[cfg(test)]
mod integration_tests {
    use crate::{
        abi::{Capabilities, Severity, Span},
        diagnostics::Diagnostic,
        host::RuntimeHost,
        interpreter::{BinOp, Interpreter, SimpleExpr, Stmt, Value},
        signals::SignalStore,
        ui::WidgetStore,
    };

    #[test]
    fn test_full_signal_lifecycle() {
        let mut store = SignalStore::new();

        // Create multiple signals
        let s1 = store.create(42i32).unwrap();
        let s2 = store.create("hello".to_string()).unwrap();
        let s3 = store.create(3.14f64).unwrap();

        // Subscribe to signals
        store.subscribe(s1, 100).unwrap();
        store.subscribe(s2, 101).unwrap();
        store.subscribe(s2, 102).unwrap();

        // Update signals
        store.set(s1, 100i32).unwrap();
        store.set(s2, "world".to_string()).unwrap();

        // Process pending updates
        let updates = store.process_pending();
        assert_eq!(updates.len(), 2);

        // Verify s1 update
        let s1_update = updates.iter().find(|(id, _, _)| *id == s1).unwrap();
        assert_eq!(s1_update.2, vec![100]);

        // Verify s2 update
        let s2_update = updates.iter().find(|(id, _, _)| *id == s2).unwrap();
        assert_eq!(s2_update.2, vec![101, 102]);

        // Verify s3 was not updated
        assert!(!updates.iter().any(|(id, _, _)| *id == s3));
    }

    #[test]
    fn test_widget_interaction() {
        let mut store = WidgetStore::new();

        // Create widgets
        let slider = store.create_slider(0.0, 100.0, 50.0);
        let text = store.create_text_input("Enter text".to_string(), "".to_string());
        let button = store.create_button("Submit".to_string());

        // Update slider
        store.update_slider(slider, 75.0).unwrap();

        // Update text input
        store.update_text_input(text, "Hello, world!".to_string()).unwrap();

        // Click button
        store.click_button(button).unwrap();

        // Show some output
        store.show(b"Result: 75".to_vec());

        let output = store.take_output();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0], b"Result: 75");
    }

    #[test]
    fn test_diagnostic_collection() {
        let mut host = RuntimeHost::new(Capabilities::new().with_diagnostics());

        let span1 = Span::new(1, 0, 1, 10);
        let span2 = Span::new(2, 5, 2, 15);
        let span3 = Span::new(3, 0, 3, 20);

        host.emit_diagnostic(Severity::Error, span1, "Syntax error").unwrap();
        host.emit_diagnostic(Severity::Warning, span2, "Unused variable").unwrap();
        host.emit_diagnostic(Severity::Info, span3, "Consider refactoring").unwrap();

        assert_eq!(host.diagnostics.diagnostic_count(), 3);
        assert_eq!(host.diagnostics.errors().len(), 1);
        assert_eq!(host.diagnostics.warnings().len(), 1);
    }

    #[test]
    fn test_interpreter_with_variables() {
        let host = RuntimeHost::with_default_capabilities();
        let mut interp = Interpreter::new(host);

        // x = 10
        interp.exec_stmt(&Stmt::Assign(
            "x".to_string(),
            SimpleExpr::Const(Value::Int(10)),
        )).unwrap();

        // y = 20
        interp.exec_stmt(&Stmt::Assign(
            "y".to_string(),
            SimpleExpr::Const(Value::Int(20)),
        )).unwrap();

        // z = x + y
        interp.exec_stmt(&Stmt::Assign(
            "z".to_string(),
            SimpleExpr::BinOp(
                Box::new(SimpleExpr::Var("x".to_string())),
                BinOp::Add,
                Box::new(SimpleExpr::Var("y".to_string())),
            ),
        )).unwrap();

        assert_eq!(interp.get_local("z"), Some(&Value::Int(30)));
    }

    #[test]
    fn test_interpreter_nested_expressions() {
        let host = RuntimeHost::with_default_capabilities();
        let mut interp = Interpreter::new(host);

        // (5 + 3) * (10 - 2)
        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::BinOp(
                Box::new(SimpleExpr::Const(Value::Int(5))),
                BinOp::Add,
                Box::new(SimpleExpr::Const(Value::Int(3))),
            )),
            BinOp::Mul,
            Box::new(SimpleExpr::BinOp(
                Box::new(SimpleExpr::Const(Value::Int(10))),
                BinOp::Sub,
                Box::new(SimpleExpr::Const(Value::Int(2))),
            )),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::Int(64)); // 8 * 8
    }

    #[test]
    fn test_runtime_host_integration() {
        let mut host = RuntimeHost::new(
            Capabilities::new()
                .with_ui()
                .with_diagnostics()
                .with_dataspace(),
        );

        // Create a signal
        let signal = host.signal_create(b"42").unwrap();
        host.signal_subscribe(signal, 1).unwrap();

        // Create a slider
        let slider = host.ui_slider(0.0, 100.0, 50.0).unwrap();

        // Emit a diagnostic
        let span = Span::new(1, 0, 1, 10);
        host.emit_diagnostic(Severity::Info, span, "Interactive cell initialized").unwrap();

        // Publish to dataspace
        let assertion = host.dataspace_publish("cell.value", b"42").unwrap();

        // Update signal
        host.signal_set(signal, b"100").unwrap();

        // Process signal updates
        let updates = host.process_signals();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].2, vec![1]);

        // Clean up
        host.dataspace_retract(assertion).unwrap();
    }

    #[test]
    fn test_capability_isolation() {
        // Host with only UI capability
        let mut host1 = RuntimeHost::new(Capabilities::new().with_ui());
        assert!(host1.ui_slider(0.0, 100.0, 50.0).is_ok());
        assert!(host1.dataspace_publish("test", b"value").is_err());

        // Host with only dataspace capability
        let mut host2 = RuntimeHost::new(Capabilities::new().with_dataspace());
        assert!(host2.dataspace_publish("test", b"value").is_ok());
        assert!(host2.ui_slider(0.0, 100.0, 50.0).is_err());
    }

    #[test]
    fn test_live_cell_simulation() {
        // Simulate a simple live cell that creates a slider and displays its value

        let mut host = RuntimeHost::new(Capabilities::new().with_ui().with_diagnostics());
        let mut interp = Interpreter::new(RuntimeHost::with_default_capabilities());

        // Create slider (simulating runtime host call)
        let slider_id = host.ui_slider(0.0, 100.0, 50.0).unwrap();

        // Simulate user dragging slider to 75
        host.widgets.update_slider(slider_id, 75.0).unwrap();

        // Compute squared value (simulating interpreter execution)
        interp.set_local("slider_value".to_string(), Value::Float(75.0));

        let squared_expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Var("slider_value".to_string())),
            BinOp::Mul,
            Box::new(SimpleExpr::Var("slider_value".to_string())),
        );

        let result = interp.eval(&squared_expr).unwrap();
        assert_eq!(result, Value::Float(5625.0));

        // Show result
        host.ui_show(&serde_json::to_vec(&result).unwrap()).unwrap();

        let output = host.take_output();
        assert_eq!(output.len(), 1);
    }
}
