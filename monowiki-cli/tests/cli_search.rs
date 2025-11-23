use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
fn search_json_with_links_outputs_results() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let docs = dir.path().join("docs");
    fs::create_dir_all(&docs)?;

    // Minimal config pointing to docs output
    fs::write(
        dir.path().join("monowiki.yml"),
        r#"
site:
  title: "Test"
  author: "Tester"
  description: "Desc"
  url: "https://example.com"
paths:
  vault: "vault"
  output: "docs"
  templates: null
  theme: null
base_url: "/"
enable_backlinks: true
"#,
    )?;

    // Search index with one entry
    fs::write(
        docs.join("index.json"),
        r#"[{
  "id": "rust#intro",
  "url": "/rust.html#intro",
  "title": "Rust Guide",
  "section_title": "Intro",
  "content": "Rust language overview",
  "snippet": "Rust language overview",
  "tags": ["rust", "guide"],
  "type": "essay"
}]"#,
    )?;

    // Graph file to supply link context
    fs::write(
        docs.join("graph.json"),
        r#"{
  "edges": [
    { "source": "rust", "target": "memory" }
  ],
  "nodes": []
}"#,
    )?;

    #[allow(deprecated)]
    let assert = Command::cargo_bin("monowiki")?
        .current_dir(dir.path())
        .args([
            "search",
            "rust",
            "--json",
            "--limit",
            "1",
            "--with-links",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())?;
    let value: Value = serde_json::from_str(&stdout)?;
    let arr = value.as_array().expect("json array");
    assert_eq!(arr.len(), 1);
    let first = &arr[0];
    assert_eq!(first["id"], "rust#intro");
    assert!(first["outgoing"]
        .as_array()
        .expect("outgoing array")
        .contains(&Value::String("memory".to_string())));

    Ok(())
}
