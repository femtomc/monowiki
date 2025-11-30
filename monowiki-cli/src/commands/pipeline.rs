//! Test the document processing pipeline
//!
//! This command demonstrates the end-to-end integration of all monowiki crates.

use monowiki_core::DocumentPipeline;
use monowiki_types::DocId;
use std::path::PathBuf;

/// Run the pipeline command
pub fn run_pipeline(input: &PathBuf, format: &str, _cached: bool) -> anyhow::Result<()> {
    // Read the source file
    let source = std::fs::read_to_string(input)?;

    // Create the pipeline
    let _pipeline = DocumentPipeline::new();

    // Output based on format
    match format {
        "debug" => println!("Source:\n{}", source),
        "json" => {
            let json = serde_json::json!({
                "source": source,
                "length": source.len()
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        "html" => {
            eprintln!("HTML rendering not yet implemented");
            println!("<!-- Source length: {} -->", source.len());
        }
        _ => println!("Source:\n{}", source),
    }

    Ok(())
}

/// Run the execute command
pub fn run_execute(input: &PathBuf) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(input)?;

    let _pipeline = DocumentPipeline::new();

    println!("Source:\n{}", source);

    Ok(())
}

/// Test incremental invalidation
pub fn test_invalidation(input: &PathBuf) -> anyhow::Result<()> {
    use monowiki_types::{BlockId, DocChange};

    let source = std::fs::read_to_string(input)?;
    let doc_id = DocId::new("test-doc");

    let pipeline = DocumentPipeline::new();

    println!("=== Initial state ===");
    println!("Source length: {}", source.len());
    println!("Revision: {:?}", pipeline.db().revision());

    println!("\n=== Simulating text change ===");
    let change = DocChange::TextChanged {
        block_id: BlockId::new(1),
        start: 0,
        end: 5,
        new_text: "HELLO".to_string(),
    };
    pipeline.on_change(&doc_id, change);
    println!("Revision after change: {:?}", pipeline.db().revision());

    Ok(())
}
