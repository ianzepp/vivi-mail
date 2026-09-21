use std::fs;

use mail_parser::MessageParser;

use crate::catalog::CatalogEntry;
use crate::error::VivariumError;

/// Extracted text from an email message.
#[derive(Debug, Clone)]
pub struct ExtractedText {
    pub body_text: String,
    pub format: ExtractionFormat,
    pub quality: ExtractionQuality,
}

/// How the body was extracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionFormat {
    Plain,
    HtmlStripped,
    None,
}

/// Quality of the extracted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionQuality {
    Full,
    Partial,
    None,
}

/// Extract text from raw .eml bytes.
///
/// # Errors
/// Returns an error if the email cannot be parsed.
pub fn extract_text(data: &[u8]) -> Result<ExtractedText, VivariumError> {
    let parsed = MessageParser::default()
        .parse(data)
        .ok_or_else(|| VivariumError::Parse("failed to parse email for extraction".into()))?;

    // Try plain text body first
    if let Some(body) = parsed.body_text(0) {
        let body = body.trim();
        if !body.is_empty() {
            return Ok(ExtractedText {
                body_text: body.to_string(),
                format: ExtractionFormat::Plain,
                quality: ExtractionQuality::Full,
            });
        }
    }

    // Try HTML body with strip
    if let Some(html) = parsed.body_html(0) {
        let text = html_to_text(&html);
        if !text.trim().is_empty() {
            return Ok(ExtractedText {
                body_text: text.trim().to_string(),
                format: ExtractionFormat::HtmlStripped,
                quality: ExtractionQuality::Full,
            });
        }
    }

    // Fallback: try any text body
    if let Some(body) = parsed.body_text(0) {
        return Ok(ExtractedText {
            body_text: body.to_string(),
            format: ExtractionFormat::Plain,
            quality: ExtractionQuality::Partial,
        });
    }

    Ok(ExtractedText {
        body_text: String::new(),
        format: ExtractionFormat::None,
        quality: ExtractionQuality::None,
    })
}

/// Strip HTML tags and produce plain text.
fn html_to_text(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut prev_was_newline = false;

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            // Convert block elements to newlines
            continue;
        }
        if !in_tag {
            if ch == '\n' || ch == '\r' {
                if !prev_was_newline {
                    result.push('\n');
                    prev_was_newline = true;
                }
            } else {
                result.push(ch);
                prev_was_newline = false;
            }
        }
    }

    result
}

/// Extract text from all catalog entries, counting successes and failures.
///
/// # Errors
/// Returns an error if reading a catalog entry's blob file fails and the error
/// is not handled as a per-entry extraction error.
pub fn extract_catalog_entries(entries: &[CatalogEntry]) -> Result<(usize, usize), VivariumError> {
    let mut extracted = 0;
    let mut errors = 0;

    for entry in entries {
        match fs::read(&entry.blob_path)
            .map_err(VivariumError::from)
            .and_then(|data| extract_text(&data).map(|_| ()))
        {
            Ok(()) => extracted += 1,
            Err(_) => errors += 1,
        }
    }

    Ok((extracted, errors))
}

#[cfg(test)]
#[path = "extract_test.rs"]
mod tests;
