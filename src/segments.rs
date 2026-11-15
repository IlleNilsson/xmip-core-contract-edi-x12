//! An interchange's segments, one at a time, as the Foundation's one segment
//! walk (`message::segment`) cuts them with the separators the `ISA`
//! declares: each identifier held to two or three capital letters or
//! digits, and a segment cut into its elements and components only when
//! asked. What [`Interchange::walk`](crate::syntax::Interchange::walk)
//! yields, and what [`Interchange::parse`](crate::syntax::Interchange::parse)
//! collects.

use contract::ValidationIssue;
// The segment is the capability's: EDIFACT and X12 read the same shape
// (ADR-0044); the syntax that cuts it out of an interchange is this crate's.
use contract::segment::Segment;
use message::segment::{Delimiters, Walk};

/// An interchange's segments, one at a time, in the order they lie, as
/// [`Interchange::walk`](crate::syntax::Interchange::walk) begins them.
/// After a refusal it yields nothing.
#[derive(Clone, Debug)]
pub struct Segments<'a> {
    delimiters: Delimiters,
    /// The text from its `ISA`, which is walked.
    text: &'a str,
    /// None once the walk has ended.
    walk: Option<Walk<'a>>,
}

impl<'a> Segments<'a> {
    /// `text`, from its `ISA`, walked with `delimiters`, the separators the
    /// `ISA` declares.
    pub(crate) const fn new(delimiters: Delimiters, text: &'a str) -> Self {
        Self {
            delimiters,
            text,
            walk: Some(delimiters.walk(text.as_bytes())),
        }
    }

    /// The separators the `ISA` declares, as the walk cuts by them.
    #[must_use]
    pub const fn delimiters(&self) -> Delimiters {
        self.delimiters
    }
}

impl<'a> Iterator for Segments<'a> {
    type Item = Result<Lying<'a>, ValidationIssue>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let walked = self.walk.as_mut()?.next()?;
            let lying = match walked {
                // Nothing between two terminators is no segment.
                Ok(walked) if walked.bytes.is_empty() => continue,
                Ok(walked) => Lying::cut(walked, self.text, self.delimiters),
                Err((reason, _)) => Err(ValidationIssue::malformed(reason)),
            };
            if lying.is_err() {
                self.walk = None;
            }
            return Some(lying);
        }
    }
}

/// One segment as it lies in the interchange: its identifier, and its text
/// up to its terminator.
#[derive(Clone, Copy, Debug)]
pub struct Lying<'a> {
    tag: &'a str,
    text: &'a str,
    walked: message::segment::Segment<'a>,
    delimiters: Delimiters,
}

impl<'a> Lying<'a> {
    /// `walked`, a segment of `text`, its identifier — what stands before
    /// its first element separator — held to two or three capital letters
    /// or digits.
    fn cut(
        walked: message::segment::Segment<'a>,
        text: &'a str,
        delimiters: Delimiters,
    ) -> Result<Self, ValidationIssue> {
        let text = &text[walked.offset..walked.offset + walked.bytes.len()];
        let tag = walked
            .bytes
            .iter()
            .position(|&byte| byte == delimiters.element)
            .map_or(text, |at| &text[..at]);
        let sound = (2..=3).contains(&tag.len())
            && tag
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if !sound {
            return Err(ValidationIssue::malformed(format!(
                "{tag:?} is not a segment identifier"
            )));
        }
        Ok(Self {
            tag,
            text,
            walked,
            delimiters,
        })
    }

    /// The segment's identifier: `ISA`, `ST`, `BEG`.
    #[must_use]
    pub const fn tag(&self) -> &'a str {
        self.tag
    }

    /// The segment, cut into its elements and their components in one
    /// pass over its text. `ISA` pads its fixed-width elements with spaces,
    /// so every component is trimmed as it is cut; the shared Segment
    /// answers what was read, and `Interchange::written` pads the `ISA` back
    /// to `ISA_WIDTHS`.
    #[must_use]
    pub fn segment(&self) -> Segment {
        let text: &'a str = self.text;
        let mut elements: Vec<Vec<String>> = Vec::new();
        let pieces = self.walked.pieces(&self.delimiters);
        // The identifier's own element is the identifier.
        for piece in pieces.skip_while(|piece| piece.element == 0) {
            let value = text[piece.at].trim().to_string();
            match elements.last_mut() {
                Some(element) if piece.component > 0 => element.push(value),
                _ => elements.push(vec![value]),
            }
        }
        Segment {
            tag: self.tag.to_string(),
            elements,
        }
    }
}
