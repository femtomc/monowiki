//! Document-level parser for mixed prose and code
//!
//! This module handles parsing documents that contain both Markdown prose
//! and MRL code blocks. The `!` character transitions from prose to code.

use crate::error::{MrlError, Result, Span};
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::shrubbery::Shrubbery;

/// An element in a document: either prose or code
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentElement {
    /// Prose text (Markdown content)
    Prose(String, Span),
    /// MRL code block
    Code(Shrubbery),
}

/// Document parser handles mixed prose and code content
pub struct DocumentParser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> DocumentParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    /// Parse a document with mixed prose and code
    pub fn parse(&mut self) -> Result<Vec<DocumentElement>> {
        let mut elements = Vec::new();

        while !self.is_eof() {
            if self.peek_char() == Some('!') && self.is_code_start() {
                elements.push(self.parse_code_element()?);
            } else {
                elements.push(self.parse_prose()?);
            }
        }

        Ok(elements)
    }

    /// Check if we're at the end of the source
    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// Peek at the current character without consuming it
    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    /// Get the character at a specific offset from current position
    fn peek_ahead(&self, offset: usize) -> Option<char> {
        let mut chars = self.source[self.pos..].chars();
        for _ in 0..offset {
            chars.next()?;
        }
        chars.next()
    }

    /// Advance position by one character
    fn advance(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    /// Check if `!` starts a code block
    /// Returns true if:
    /// - `!` is followed by a letter, `(`, or `[`
    /// - NOT if it's `!!` (escaped exclamation)
    /// - NOT if it's `![` followed by `]` or `(` (Markdown image)
    fn is_code_start(&self) -> bool {
        if self.peek_char() != Some('!') {
            return false;
        }

        // Check for !!
        if self.peek_ahead(1) == Some('!') {
            return false;
        }

        // Check for Markdown image syntax: ![...](...) or ![alt](url)
        if self.peek_ahead(1) == Some('[') {
            // This could be Markdown image or MRL code
            // For now, treat ![...] as Markdown if followed by (...)
            // This is a simplification - full implementation would be more sophisticated
            return false;
        }

        // Check if followed by identifier start, paren, or bracket
        if let Some(next) = self.peek_ahead(1) {
            next.is_alphabetic() || next == '(' || next == '[' || next == '_'
        } else {
            false
        }
    }

    /// Parse prose text until we hit a code block or EOF
    fn parse_prose(&mut self) -> Result<DocumentElement> {
        let start = self.pos;
        let mut text = String::new();

        while !self.is_eof() {
            if self.peek_char() == Some('!') && self.is_code_start() {
                break;
            }

            // Handle !! as escaped !
            if self.peek_char() == Some('!') && self.peek_ahead(1) == Some('!') {
                self.advance(); // consume first !
                self.advance(); // consume second !
                text.push('!'); // add single ! to output
                continue;
            }

            if let Some(ch) = self.advance() {
                text.push(ch);
            }
        }

        let end = self.pos;
        Ok(DocumentElement::Prose(text, Span::new(start, end)))
    }

    /// Parse a code element starting with `!`
    fn parse_code_element(&mut self) -> Result<DocumentElement> {
        let start = self.pos;

        // Find the end of the code block
        // For now, we'll parse until we hit a newline followed by non-indented content
        // or until we find a balanced bracket/paren
        let mut code = String::new();

        // Consume the initial `!`
        if self.peek_char() == Some('!') {
            code.push('!');
            self.advance();
        }

        // Parse the rest of the code
        // This is simplified - a full implementation would handle:
        // - Balanced brackets/parens/braces
        // - Indented blocks
        // - String literals
        // - Comments

        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        while !self.is_eof() {
            let ch = self.peek_char().unwrap();

            // Handle string literals
            if ch == '"' && !escape_next {
                in_string = !in_string;
            }

            // Handle escape sequences
            escape_next = ch == '\\' && !escape_next;

            // Track bracket depth (only when not in string)
            if !in_string {
                match ch {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => {
                        depth -= 1;
                        if depth < 0 {
                            break; // Unbalanced - stop here
                        }
                    }
                    '\n' => {
                        // If we hit a newline and depth is 0, check if next line is dedented
                        if depth == 0 {
                            code.push(ch);
                            self.advance();
                            // Peek at next line - if it starts with non-whitespace, we're done
                            if let Some(next_ch) = self.peek_char() {
                                if !next_ch.is_whitespace() || next_ch == '\n' {
                                    break;
                                }
                            } else {
                                break;
                            }
                            continue;
                        }
                    }
                    _ => {}
                }
            }

            code.push(ch);
            self.advance();

            // If we've closed all brackets and hit whitespace, stop
            if depth == 0 && !in_string && ch.is_whitespace() {
                break;
            }
        }

        let end = self.pos;

        // Tokenize and parse the code
        let tokens = tokenize(&code)?;
        let shrub = parse(&tokens)?;

        Ok(DocumentElement::Code(shrub))
    }
}

/// Parse a document with mixed prose and MRL code
pub fn parse_document(source: &str) -> Result<Vec<DocumentElement>> {
    let mut parser = DocumentParser::new(source);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pure_prose() {
        let source = "This is plain text with no code.";
        let elements = parse_document(source).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], DocumentElement::Prose(text, _) if text.contains("plain text")));
    }

    #[test]
    fn test_parse_escaped_exclamation() {
        let source = "Use !! to get a literal exclamation!";
        let elements = parse_document(source).unwrap();
        assert_eq!(elements.len(), 1);
        if let DocumentElement::Prose(text, _) = &elements[0] {
            assert!(text.contains("!"));
            assert!(!text.contains("!!"));
        } else {
            panic!("Expected prose element");
        }
    }

    #[test]
    fn test_parse_inline_code() {
        let source = "Today is !today()";
        let elements = parse_document(source).unwrap();
        assert_eq!(elements.len(), 2);
        assert!(matches!(&elements[0], DocumentElement::Prose(_, _)));
        assert!(matches!(&elements[1], DocumentElement::Code(_)));
    }

    #[test]
    fn test_parse_mixed_content() {
        let source = r#"# Title

This is a paragraph with !inline_code.

!def greet(name: String):
  text("Hello")

More prose here."#;

        let elements = parse_document(source).unwrap();
        // Should have prose, code (inline), prose, code (def block), prose
        assert!(elements.len() >= 3);
    }

    #[test]
    fn test_markdown_image_not_code() {
        let source = "Here's an image: ![alt text](url)";
        let elements = parse_document(source).unwrap();
        // Should be pure prose since ![...] is Markdown image syntax
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], DocumentElement::Prose(_, _)));
    }
}
