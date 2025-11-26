//! Test the new document processing pipeline
//!
//! This command demonstrates the end-to-end integration of all monowiki crates.

use monowiki_core::{DocumentPipeline, PipelineError};
use monowiki_types::DocId;
use std::path::PathBuf;

/// Run the pipeline command
pub fn run_pipeline(
    input: &PathBuf,
    format: &str,
    cached: bool,
) -> anyhow::Result<()> {
    // Read the source file
    let source = std::fs::read_to_string(input)?;

    // Create the pipeline
    let pipeline = DocumentPipeline::new();

    // Process based on mode
    let content = if cached {
        // Use incremental caching
        let doc_id = DocId::new(
            input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document")
        );
        pipeline.process_cached(&doc_id, &source)?
    } else {
        // Direct processing without cache
        pipeline.process_source(&source)?
    };

    // Output based on format
    match format {
        "debug" => println!("{:#?}", content),
        "json" => {
            let json = serde_json::to_string_pretty(&content)?;
            println!("{}", json);
        }
        "html" => {
            // TODO: Add HTML rendering
            eprintln!("HTML rendering not yet implemented");
            println!("<!-- Content: {:?} -->", content);
        }
        _ => println!("{:#?}", content),
    }

    Ok(())
}

/// Run the execute command (full interpreter)
pub fn run_execute(input: &PathBuf) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(input)?;

    let pipeline = DocumentPipeline::new();
    let content = pipeline.execute(&source)?;

    println!("{:#?}", content);

    Ok(())
}

/// Test incremental invalidation
pub fn test_invalidation(input: &PathBuf) -> anyhow::Result<()> {
    use monowiki_types::{BlockId, DocChange};

    let source = std::fs::read_to_string(input)?;
    let doc_id = DocId::new("test-doc");

    let pipeline = DocumentPipeline::new();

    println!("=== Initial processing ===");
    let content1 = pipeline.process_cached(&doc_id, &source)?;
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

    println!("\n=== Reprocessing after invalidation ===");
    let content2 = pipeline.process_cached(&doc_id, &source)?;

    println!("\nBoth results are structurally the same: {}",
             format!("{:?}", content1) == format!("{:?}", content2));

    Ok(())
}
