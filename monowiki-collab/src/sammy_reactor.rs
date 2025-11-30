use sammy::runtime::Runtime;

/// Attach a logging reactor to a doc dataspace to keep recent events.
///
/// NOTE: This is currently a no-op stub. Full subscription-based reactors
/// require additional Actor API support that is not yet implemented.
pub fn ensure_doc_reactor(_rt: &mut Runtime<sammy::runtime::DefaultConfig>, _ds_name: &str) {
    // TODO: Implement subscription-based reactors once Actor::subscribe is available
}
