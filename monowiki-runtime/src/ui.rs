//! UI widget host bindings (stub implementation)
//!
//! This module provides the host-side implementation of UI widgets for live cells.
//! Widgets are created in WASM code and rendered by the host environment.

use crate::abi::{RuntimeError, RuntimeResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Widget identifier
pub type WidgetId = u64;

/// Base widget interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Widget {
    Slider {
        id: WidgetId,
        min: f64,
        max: f64,
        value: f64,
    },
    TextInput {
        id: WidgetId,
        placeholder: String,
        value: String,
    },
    Button {
        id: WidgetId,
        label: String,
        clicked: bool,
    },
}

impl Widget {
    pub fn id(&self) -> WidgetId {
        match self {
            Widget::Slider { id, .. } => *id,
            Widget::TextInput { id, .. } => *id,
            Widget::Button { id, .. } => *id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Widget::Slider { .. } => "slider",
            Widget::TextInput { .. } => "text-input",
            Widget::Button { .. } => "button",
        }
    }
}

/// Store for all widgets in a live cell runtime
#[derive(Debug)]
pub struct WidgetStore {
    widgets: HashMap<WidgetId, Widget>,
    next_id: AtomicU64,
    output_buffer: Vec<Vec<u8>>,
}

impl WidgetStore {
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
            next_id: AtomicU64::new(1),
            output_buffer: Vec::new(),
        }
    }

    /// Create a slider widget
    pub fn create_slider(&mut self, min: f64, max: f64, initial: f64) -> WidgetId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let widget = Widget::Slider {
            id,
            min,
            max,
            value: initial.clamp(min, max),
        };
        self.widgets.insert(id, widget);
        id
    }

    /// Create a text input widget
    pub fn create_text_input(&mut self, placeholder: String, initial: String) -> WidgetId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let widget = Widget::TextInput {
            id,
            placeholder,
            value: initial,
        };
        self.widgets.insert(id, widget);
        id
    }

    /// Create a button widget
    pub fn create_button(&mut self, label: String) -> WidgetId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let widget = Widget::Button {
            id,
            label,
            clicked: false,
        };
        self.widgets.insert(id, widget);
        id
    }

    /// Get a widget by ID
    pub fn get(&self, id: WidgetId) -> RuntimeResult<&Widget> {
        self.widgets
            .get(&id)
            .ok_or(RuntimeError::WidgetNotFound(id))
    }

    /// Get a mutable reference to a widget
    pub fn get_mut(&mut self, id: WidgetId) -> RuntimeResult<&mut Widget> {
        self.widgets
            .get_mut(&id)
            .ok_or(RuntimeError::WidgetNotFound(id))
    }

    /// Update slider value
    pub fn update_slider(&mut self, id: WidgetId, value: f64) -> RuntimeResult<()> {
        let widget = self.get_mut(id)?;
        match widget {
            Widget::Slider {
                min, max, value: v, ..
            } => {
                *v = value.clamp(*min, *max);
                Ok(())
            }
            _ => Err(RuntimeError::WasmError(format!(
                "Widget {} is not a slider",
                id
            ))),
        }
    }

    /// Update text input value
    pub fn update_text_input(&mut self, id: WidgetId, value: String) -> RuntimeResult<()> {
        let widget = self.get_mut(id)?;
        match widget {
            Widget::TextInput { value: v, .. } => {
                *v = value;
                Ok(())
            }
            _ => Err(RuntimeError::WasmError(format!(
                "Widget {} is not a text input",
                id
            ))),
        }
    }

    /// Mark button as clicked
    pub fn click_button(&mut self, id: WidgetId) -> RuntimeResult<()> {
        let widget = self.get_mut(id)?;
        match widget {
            Widget::Button { clicked, .. } => {
                *clicked = true;
                Ok(())
            }
            _ => Err(RuntimeError::WasmError(format!(
                "Widget {} is not a button",
                id
            ))),
        }
    }

    /// Reset button clicked state
    pub fn reset_button(&mut self, id: WidgetId) -> RuntimeResult<()> {
        let widget = self.get_mut(id)?;
        match widget {
            Widget::Button { clicked, .. } => {
                *clicked = false;
                Ok(())
            }
            _ => Err(RuntimeError::WasmError(format!(
                "Widget {} is not a button",
                id
            ))),
        }
    }

    /// Show a value in the output area
    pub fn show(&mut self, value: Vec<u8>) {
        self.output_buffer.push(value);
    }

    /// Get all output values and clear the buffer
    pub fn take_output(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.output_buffer)
    }

    /// Get all widgets
    pub fn widgets(&self) -> impl Iterator<Item = &Widget> {
        self.widgets.values()
    }

    /// Get the number of widgets
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    /// Clear all widgets and output
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.output_buffer.clear();
    }
}

impl Default for WidgetStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_slider() {
        let mut store = WidgetStore::new();
        let id = store.create_slider(0.0, 100.0, 50.0);

        let widget = store.get(id).unwrap();
        match widget {
            Widget::Slider { min, max, value, .. } => {
                assert_eq!(*min, 0.0);
                assert_eq!(*max, 100.0);
                assert_eq!(*value, 50.0);
            }
            _ => panic!("Expected slider widget"),
        }
    }

    #[test]
    fn test_update_slider() {
        let mut store = WidgetStore::new();
        let id = store.create_slider(0.0, 100.0, 50.0);

        store.update_slider(id, 75.0).unwrap();

        let widget = store.get(id).unwrap();
        match widget {
            Widget::Slider { value, .. } => {
                assert_eq!(*value, 75.0);
            }
            _ => panic!("Expected slider widget"),
        }
    }

    #[test]
    fn test_slider_clamping() {
        let mut store = WidgetStore::new();
        let id = store.create_slider(0.0, 100.0, 50.0);

        store.update_slider(id, 150.0).unwrap();

        let widget = store.get(id).unwrap();
        match widget {
            Widget::Slider { value, .. } => {
                assert_eq!(*value, 100.0);
            }
            _ => panic!("Expected slider widget"),
        }
    }

    #[test]
    fn test_create_text_input() {
        let mut store = WidgetStore::new();
        let id = store.create_text_input("Enter text".to_string(), "Hello".to_string());

        let widget = store.get(id).unwrap();
        match widget {
            Widget::TextInput {
                placeholder, value, ..
            } => {
                assert_eq!(placeholder, "Enter text");
                assert_eq!(value, "Hello");
            }
            _ => panic!("Expected text input widget"),
        }
    }

    #[test]
    fn test_create_button() {
        let mut store = WidgetStore::new();
        let id = store.create_button("Click me".to_string());

        let widget = store.get(id).unwrap();
        match widget {
            Widget::Button {
                label, clicked, ..
            } => {
                assert_eq!(label, "Click me");
                assert!(!clicked);
            }
            _ => panic!("Expected button widget"),
        }
    }

    #[test]
    fn test_button_click() {
        let mut store = WidgetStore::new();
        let id = store.create_button("Click me".to_string());

        store.click_button(id).unwrap();

        let widget = store.get(id).unwrap();
        match widget {
            Widget::Button { clicked, .. } => {
                assert!(clicked);
            }
            _ => panic!("Expected button widget"),
        }

        store.reset_button(id).unwrap();

        let widget = store.get(id).unwrap();
        match widget {
            Widget::Button { clicked, .. } => {
                assert!(!clicked);
            }
            _ => panic!("Expected button widget"),
        }
    }

    #[test]
    fn test_show_output() {
        let mut store = WidgetStore::new();

        store.show(vec![1, 2, 3]);
        store.show(vec![4, 5, 6]);

        let output = store.take_output();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0], vec![1, 2, 3]);
        assert_eq!(output[1], vec![4, 5, 6]);

        let output2 = store.take_output();
        assert_eq!(output2.len(), 0);
    }
}
