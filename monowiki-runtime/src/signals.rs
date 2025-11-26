//! Reactive signal primitives
//!
//! This module implements a reactive signal system for live cells.
//! Signals are the core primitive for reactivity in monowiki documents.

use crate::abi::{RuntimeError, RuntimeResult};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for a callback
pub type CallbackId = u64;

/// Internal representation of signal data
#[derive(Debug)]
pub struct SignalData {
    /// Serialized value
    pub(crate) value: Vec<u8>,
    /// Callbacks subscribed to this signal
    pub(crate) subscribers: Vec<CallbackId>,
}

impl SignalData {
    pub fn new(value: Vec<u8>) -> Self {
        Self {
            value,
            subscribers: Vec::new(),
        }
    }
}

/// A reactive signal with typed access
///
/// Signals are the primary reactive primitive in monowiki. They store a value
/// and notify subscribers when the value changes.
#[derive(Debug)]
pub struct Signal<T> {
    pub id: u64,
    pub(crate) value: T,
    pub(crate) subscribers: Vec<CallbackId>,
}

impl<T> Signal<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn new(id: u64, value: T) -> Self {
        Self {
            id,
            value,
            subscribers: Vec::new(),
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
    }

    pub fn subscribe(&mut self, callback: CallbackId) {
        if !self.subscribers.contains(&callback) {
            self.subscribers.push(callback);
        }
    }

    pub fn unsubscribe(&mut self, callback: CallbackId) {
        self.subscribers.retain(|&id| id != callback);
    }

    pub fn subscribers(&self) -> &[CallbackId] {
        &self.subscribers
    }
}

/// Store for all signals in a live cell runtime
///
/// The SignalStore manages signal creation, access, updates, and reactivity.
/// It maintains a queue of pending updates to process signal changes in order.
#[derive(Debug)]
pub struct SignalStore {
    signals: HashMap<u64, SignalData>,
    next_id: AtomicU64,
    pending_updates: VecDeque<u64>,
}

impl SignalStore {
    pub fn new() -> Self {
        Self {
            signals: HashMap::new(),
            next_id: AtomicU64::new(1),
            pending_updates: VecDeque::new(),
        }
    }

    /// Create a new signal with typed initial value
    pub fn create<T: Serialize>(&mut self, initial: T) -> RuntimeResult<u64> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let value = serde_json::to_vec(&initial)
            .map_err(|e| RuntimeError::SerializationError(e.to_string()))?;

        let data = SignalData::new(value);
        self.signals.insert(id, data);

        Ok(id)
    }

    /// Create a signal from raw bytes
    pub fn create_raw(&mut self, initial: Vec<u8>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let data = SignalData::new(initial);
        self.signals.insert(id, data);
        id
    }

    /// Get the current value of a signal
    pub fn get<T: DeserializeOwned>(&self, id: u64) -> RuntimeResult<T> {
        let data = self
            .signals
            .get(&id)
            .ok_or(RuntimeError::SignalNotFound(id))?;

        serde_json::from_slice(&data.value)
            .map_err(|e| RuntimeError::SerializationError(e.to_string()))
    }

    /// Get raw signal value
    pub fn get_raw(&self, id: u64) -> RuntimeResult<Vec<u8>> {
        let data = self
            .signals
            .get(&id)
            .ok_or(RuntimeError::SignalNotFound(id))?;

        Ok(data.value.clone())
    }

    /// Set the value of a signal and queue reactivity
    pub fn set<T: Serialize>(&mut self, id: u64, value: T) -> RuntimeResult<()> {
        let serialized = serde_json::to_vec(&value)
            .map_err(|e| RuntimeError::SerializationError(e.to_string()))?;

        self.set_raw(id, serialized)
    }

    /// Set raw signal value
    pub fn set_raw(&mut self, id: u64, value: Vec<u8>) -> RuntimeResult<()> {
        let data = self
            .signals
            .get_mut(&id)
            .ok_or(RuntimeError::SignalNotFound(id))?;

        data.value = value;

        // Queue this signal for reactivity processing
        if !self.pending_updates.contains(&id) {
            self.pending_updates.push_back(id);
        }

        Ok(())
    }

    /// Subscribe a callback to a signal
    pub fn subscribe(&mut self, signal_id: u64, callback: CallbackId) -> RuntimeResult<()> {
        let data = self
            .signals
            .get_mut(&signal_id)
            .ok_or(RuntimeError::SignalNotFound(signal_id))?;

        if !data.subscribers.contains(&callback) {
            data.subscribers.push(callback);
        }

        Ok(())
    }

    /// Unsubscribe a callback from a signal
    pub fn unsubscribe(&mut self, signal_id: u64, callback: CallbackId) -> RuntimeResult<()> {
        let data = self
            .signals
            .get_mut(&signal_id)
            .ok_or(RuntimeError::SignalNotFound(signal_id))?;

        data.subscribers.retain(|&id| id != callback);

        Ok(())
    }

    /// Get all subscribers for a signal
    pub fn subscribers(&self, signal_id: u64) -> RuntimeResult<Vec<CallbackId>> {
        let data = self
            .signals
            .get(&signal_id)
            .ok_or(RuntimeError::SignalNotFound(signal_id))?;

        Ok(data.subscribers.clone())
    }

    /// Process pending signal updates and return callbacks to invoke
    ///
    /// Returns a list of (signal_id, serialized_value) pairs for signals
    /// that have pending updates, along with their subscribers.
    pub fn process_pending(&mut self) -> Vec<(u64, Vec<u8>, Vec<CallbackId>)> {
        let mut results = Vec::new();

        while let Some(signal_id) = self.pending_updates.pop_front() {
            if let Some(data) = self.signals.get(&signal_id) {
                results.push((signal_id, data.value.clone(), data.subscribers.clone()));
            }
        }

        results
    }

    /// Check if there are pending updates
    pub fn has_pending(&self) -> bool {
        !self.pending_updates.is_empty()
    }

    /// Get the number of signals
    pub fn len(&self) -> usize {
        self.signals.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    /// Clear all signals and pending updates
    pub fn clear(&mut self) {
        self.signals.clear();
        self.pending_updates.clear();
    }
}

impl Default for SignalStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_create_and_get() {
        let mut store = SignalStore::new();

        let id = store.create(42i32).unwrap();
        assert_eq!(store.get::<i32>(id).unwrap(), 42);
    }

    #[test]
    fn test_signal_set_and_reactivity() {
        let mut store = SignalStore::new();

        let id = store.create(42i32).unwrap();
        store.subscribe(id, 100).unwrap();

        store.set(id, 100i32).unwrap();

        let updates = store.process_pending();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, id);
        assert_eq!(updates[0].2, vec![100]);
    }

    #[test]
    fn test_signal_multiple_subscribers() {
        let mut store = SignalStore::new();

        let id = store.create("hello".to_string()).unwrap();
        store.subscribe(id, 1).unwrap();
        store.subscribe(id, 2).unwrap();
        store.subscribe(id, 3).unwrap();

        store.set(id, "world".to_string()).unwrap();

        let updates = store.process_pending();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].2, vec![1, 2, 3]);
    }

    #[test]
    fn test_signal_unsubscribe() {
        let mut store = SignalStore::new();

        let id = store.create(42i32).unwrap();
        store.subscribe(id, 1).unwrap();
        store.subscribe(id, 2).unwrap();

        store.unsubscribe(id, 1).unwrap();

        let subs = store.subscribers(id).unwrap();
        assert_eq!(subs, vec![2]);
    }

    #[test]
    fn test_signal_raw_operations() {
        let mut store = SignalStore::new();

        let data = vec![1, 2, 3, 4];
        let id = store.create_raw(data.clone());

        let retrieved = store.get_raw(id).unwrap();
        assert_eq!(retrieved, data);

        let new_data = vec![5, 6, 7, 8];
        store.set_raw(id, new_data.clone()).unwrap();

        let retrieved = store.get_raw(id).unwrap();
        assert_eq!(retrieved, new_data);
    }

    #[test]
    fn test_signal_not_found() {
        let store = SignalStore::new();

        let result = store.get::<i32>(999);
        assert!(matches!(result, Err(RuntimeError::SignalNotFound(999))));
    }
}
