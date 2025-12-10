//! CLI command implementations.

pub mod adapters;
pub mod build;
pub mod changes;
pub mod comment;
pub mod dev;
pub mod export;
pub mod github_pages;
pub mod graph;
pub mod init;
pub mod note;
pub mod search;
pub mod status;
pub mod verify;
pub mod watch;

pub use build::build_site;
pub use changes::{changes, compute_changes};
pub use comment::{add_comment, list_comments};
pub use dev::dev_server;
pub use export::export_sections;
pub use github_pages::setup_github_pages;
pub use graph::{graph_neighbors, graph_path};
pub use init::init_project;
pub use note::show_note;
pub use search::{search_site, SearchOptions};
pub use status::status;
pub use verify::verify_site;
pub use watch::watch_changes;
