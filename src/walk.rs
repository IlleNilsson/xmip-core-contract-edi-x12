//! An interchange walked a segment at a time with the separators its `ISA`
//! declares. What reads one segment cuts only that one and walks the rest
//! uncut, and [`Interchange::parse`](crate::syntax::Interchange::parse) cuts
//! every one into the tree a write edits: one walk, so a segment is cut and
//! a malformed interchange refused in the same words by both.
//!
//! A segment is the text from its identifier to its terminator, the line
//! breaks and spaces before it skipped. It is cut into elements and
//! components only when asked, in one pass over its text; X12 has no release
//! character.

use contract::ValidationIssue;
// The segment is the capability's: EDIFACT and X12 read the same shape
// (ADR-0044); the syntax that cuts it out of an interchange is this crate's.
use contract::segment::Segment;

use std::iter::Peekable;
use std::str::Split;

use crate::syntax::{ISA_LENGTH, Separators};

/// The segments of an interchange, one at a time, in the order they lie.
#[derive(Clone, Debug)]
pub struct Walk<'a> {
    separators: Separators,
    /// The text cut at every terminator, by one search kept across the
    /// walk; none once the walk has ended.
    pieces: Option<Peekable<Split<'a, char>>>,
    walked: bool,
}

impl<'a> Walk<'a> {
    /// `text`, read with the separators its `ISA` declares by position.
    ///
    /// # Errors
    /// The `malformed` issue when the text does not open with an `ISA` of
    /// its 106 characters declaring distinct separators.
    pub fn over(text: &'a str) -> Result<Self, ValidationIssue> {
        let text = text.trim_start();
        if !text.starts_with("ISA") {
            return Err(ValidationIssue::malformed(
                "the interchange does not open with ISA",
            ));
        }
        let head: Vec<char> = text.chars().take(ISA_LENGTH).collect();
        if head.len() < ISA_LENGTH {
            return Err(ValidationIssue::malformed(
                "ISA is shorter than its 106 characters",
            ));
        }
        let separators = Separators {
            element: head[3],
            component: head[104],
            terminator: head[105],
        };
        if separators.element == separators.terminator
            || separators.element.is_alphanumeric()
            || separators.terminator.is_alphanumeric()
        {
            return Err(ValidationIssue::malformed(
                "ISA does not declare distinct separators",
            ));
        }
        Ok(Self {
            separators,
            pieces: Some(text.split(separators.terminator).peekable()),
            walked: false,
        })
    }

    /// The separators the `ISA` declares.
    #[must_use]
    pub const fn separators(&self) -> Separators {
        self.separators
    }
}

impl<'a> Iterator for Walk<'a> {
    type Item = Result<Lying<'a>, ValidationIssue>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let pieces = self.pieces.as_mut()?;
            let piece = pieces.next()?;
            // The piece after the last terminator: what is there ends it.
            let last = pieces.peek().is_none();
            let raw = piece.trim_start_matches(['\r', '\n', ' ']);
            if last {
                self.pieces = None;
            }
            if raw.is_empty() {
                if last && !self.walked {
                    return Some(Err(ValidationIssue::malformed("no segment")));
                }
                continue;
            }
            let lying = Lying::cut(raw, self.separators);
            if last && lying.is_ok() {
                return Some(Err(ValidationIssue::malformed(
                    "the last segment has no terminator",
                )));
            }
            match lying {
                Ok(_) => self.walked = true,
                Err(_) => self.pieces = None,
            }
            return Some(lying);
        }
    }
}

/// One segment as it lies in the interchange: its identifier, and its
/// elements' text after it, up to its terminator.
#[derive(Clone, Copy, Debug)]
pub struct Lying<'a> {
    tag: &'a str,
    /// After the identifier's element separator; none where there is none.
    elements: Option<&'a str>,
    separators: Separators,
}

impl<'a> Lying<'a> {
    /// `text`, one segment without its terminator, its identifier read and
    /// held to two or three capital letters or digits.
    fn cut(text: &'a str, s: Separators) -> Result<Self, ValidationIssue> {
        // A character at a time: an identifier is two or three long, too
        // short for a search to pay for itself.
        let (tag, elements) = match text.char_indices().find(|&(_, ch)| ch == s.element) {
            Some((at, ch)) => (&text[..at], Some(&text[at + ch.len_utf8()..])),
            None => (text, None),
        };
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
            elements,
            separators: s,
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
        let s = self.separators;
        let mut elements = Vec::new();
        if let Some(text) = self.elements {
            let mut components = Vec::new();
            let mut start = 0;
            for (at, ch) in text.char_indices() {
                // An element separator ends an element even where the
                // component separator is the same character.
                if ch == s.element || ch == s.component {
                    components.push(text[start..at].trim().to_string());
                    start = at + ch.len_utf8();
                    if ch == s.element {
                        elements.push(std::mem::take(&mut components));
                    }
                }
            }
            components.push(text[start..].trim().to_string());
            elements.push(components);
        }
        Segment {
            tag: self.tag.to_string(),
            elements,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISA: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       \
*260908*1030*U*00401*000000001*0*P*>";

    #[test]
    fn a_walk_yields_a_segment_at_a_time_and_stops_where_it_is_left() {
        let text = format!("{ISA}~ST*850*0001~BEG*00*SA*PO4711~SE*3*0001~IEA*1*000000001");
        let mut walk = Walk::over(&text).expect("declared");
        assert_eq!(walk.separators().terminator, '~');
        assert_eq!(walk.next().expect("ISA").expect("sound").tag(), "ISA");
        let st = walk.next().expect("ST").expect("sound");
        assert_eq!(st.segment().element(1), ["850"]);
        assert_eq!(walk.next().expect("BEG").expect("sound").tag(), "BEG");
        assert_eq!(walk.next().expect("SE").expect("sound").tag(), "SE");
        let open = walk.next().expect("IEA").expect_err("no terminator");
        assert_eq!(open.message, "the last segment has no terminator");
        assert!(walk.next().is_none(), "a walk ends where it refuses");
    }

    /// A line break the `ISA` declares the terminator ends every segment,
    /// the last one too: the end is the walk's, never the text trimmed.
    #[test]
    fn a_terminator_that_is_a_line_break_ends_a_segment() {
        let text = format!("{ISA}\nST*850*0001\nSE*2*0001\nIEA*1*000000001\n");
        let walked: Vec<String> = Walk::over(&text)
            .expect("declared")
            .map(|lying| lying.expect("sound").tag().to_string())
            .collect();
        assert_eq!(walked, ["ISA", "ST", "SE", "IEA"]);
    }
}
