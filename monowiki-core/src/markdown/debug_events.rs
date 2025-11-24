//! Debug helper to inspect pulldown-cmark events

use pulldown_cmark::{Event, Options, Parser};

#[test]
fn debug_what_events_are_generated() {
    let markdown = "Check out [[Rust Safety]] for more info.";

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown, options);
    let events: Vec<Event> = parser.collect();

    println!("\nEvents for: {}", markdown);
    for (i, event) in events.iter().enumerate() {
        println!("{}: {:?}", i, event);
    }
}

#[test]
fn debug_sidenote_without_footnotes() {
    let markdown = "Example: This documentation[^sidenote: Built with monowiki itself!] demonstrates the features.";

    let options = Options::empty();

    let parser = Parser::new_ext(markdown, options);
    let events: Vec<Event> = parser.collect();

    println!("\nEvents for sidenote WITHOUT footnotes:");
    println!("Markdown: {}", markdown);
    for (i, event) in events.iter().enumerate() {
        println!("{}: {:?}", i, event);
    }
}
