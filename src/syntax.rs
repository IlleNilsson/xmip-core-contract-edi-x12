//! ASC X12 syntax: reading an interchange into segments, and the soundness
//! checks the envelope segments carry.
//!
//! The `ISA` segment is fixed at 106 characters and declares the separators
//! by position: the element separator is the fourth character, the component
//! separator the 105th, and the segment terminator the 106th. Everything
//! after is read with those. An issue's `code` is `malformed` when the text
//! cannot be read as segments at all, else `envelope` for a departure of
//! `ISA`/`IEA`, `GS`/`GE` or `ST`/`SE`; its `path` is `segment N (TAG)`.

use contract::ValidationIssue;

/// The length `ISA` always has, terminator included.
pub const ISA_LENGTH: usize = 106;

/// The separators an interchange declares.
#[derive(Clone, Copy, Debug)]
pub struct Separators {
    pub element: char,
    pub component: char,
    pub terminator: char,
}

/// One segment: a tag and its elements, each a list of components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub tag: String,
    pub elements: Vec<Vec<String>>,
}

impl Segment {
    /// The components of element `index` (1-based, after the tag), or none.
    #[must_use]
    pub fn element(&self, index: usize) -> &[String] {
        self.elements.get(index - 1).map_or(&[], Vec::as_slice)
    }

    /// The first component of element `index`, trimmed, or empty.
    #[must_use]
    pub fn simple(&self, index: usize) -> &str {
        self.element(index).first().map_or("", |c| c.trim())
    }
}

/// A read interchange.
pub struct Interchange {
    separators: Separators,
    segments: Vec<Segment>,
}

impl Interchange {
    /// Read `text` with the separators its `ISA` declares.
    ///
    /// # Errors
    /// The one `malformed` issue when the text is not an interchange at all.
    pub fn parse(text: &str) -> Result<Self, ValidationIssue> {
        let text = text.trim_start();
        if !text.starts_with("ISA") {
            return Err(malformed("the interchange does not open with ISA"));
        }
        let head: Vec<char> = text.chars().take(ISA_LENGTH).collect();
        if head.len() < ISA_LENGTH {
            return Err(malformed("ISA is shorter than its 106 characters"));
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
            return Err(malformed("ISA does not declare distinct separators"));
        }
        let segments = segments(text, separators)?;
        if segments.is_empty() {
            return Err(malformed("no segment"));
        }
        Ok(Self {
            separators,
            segments,
        })
    }

    #[must_use]
    pub fn separators(&self) -> Separators {
        self.separators
    }

    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Every `ST`, numbered from 1, with the `GS` in force at it.
    pub fn transaction_sets(&self) -> impl Iterator<Item = (usize, &Segment, Option<&Segment>)> {
        let mut group = None;
        let mut ordinal = 0;
        self.segments.iter().filter_map(move |segment| {
            if segment.tag == "GS" {
                group = Some(segment);
            }
            if segment.tag != "ST" {
                return None;
            }
            ordinal += 1;
            Some((ordinal, segment, group))
        })
    }

    /// Every departure of the envelope segments from X12.
    #[must_use]
    pub fn soundness(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let at = |n: usize| format!("segment {} ({})", n + 1, self.segments[n].tag);
        let first = &self.segments[0];
        let last = self.segments.len() - 1;
        if self.segments[last].tag != "IEA" {
            issues.push(envelope(
                "the interchange does not close with IEA",
                &at(last),
            ));
        }
        let mut open_set: Option<(usize, String)> = None;
        let mut open_group: Option<(usize, String, usize)> = None;
        let mut sets = 0;
        let mut groups = 0;
        for (n, segment) in self.segments.iter().enumerate() {
            match segment.tag.as_str() {
                "ST" => {
                    if let Some((start, _)) = &open_set {
                        let message = format!("ST inside the set opened at segment {}", start + 1);
                        issues.push(envelope(&message, &at(n)));
                    }
                    open_set = Some((n, segment.simple(2).to_string()));
                    sets += 1;
                }
                "SE" => match open_set.take() {
                    Some((start, control)) => {
                        let counted = n - start + 1;
                        if segment.simple(1).parse::<usize>().ok() != Some(counted) {
                            let message = format!(
                                "SE counts {} segments, the set has {counted}",
                                segment.simple(1)
                            );
                            issues.push(envelope(&message, &at(n)));
                        }
                        if segment.simple(2) != control {
                            let message =
                                format!("SE closes {}, ST opened {control}", segment.simple(2));
                            issues.push(envelope(&message, &at(n)));
                        }
                    }
                    None => issues.push(envelope("SE with no set open", &at(n))),
                },
                "GS" => {
                    if open_group.is_some() {
                        issues.push(envelope("GS inside an open group", &at(n)));
                    }
                    open_group = Some((n, segment.simple(6).to_string(), sets));
                    groups += 1;
                }
                "GE" => match open_group.take() {
                    Some((_, control, before)) => {
                        let in_group = sets - before;
                        if segment.simple(1).parse::<usize>().ok() != Some(in_group) {
                            let message = format!(
                                "GE counts {} sets, the group has {in_group}",
                                segment.simple(1)
                            );
                            issues.push(envelope(&message, &at(n)));
                        }
                        if segment.simple(2) != control {
                            let message =
                                format!("GE closes {}, GS opened {control}", segment.simple(2));
                            issues.push(envelope(&message, &at(n)));
                        }
                    }
                    None => issues.push(envelope("GE with no group open", &at(n))),
                },
                "IEA" => {
                    if segment.simple(1).parse::<usize>().ok() != Some(groups) {
                        let message = format!(
                            "IEA counts {} groups, the interchange has {groups}",
                            segment.simple(1)
                        );
                        issues.push(envelope(&message, &at(n)));
                    }
                    if segment.simple(2) != first.simple(13) {
                        let message = format!(
                            "IEA closes {}, ISA opened {}",
                            segment.simple(2),
                            first.simple(13)
                        );
                        issues.push(envelope(&message, &at(n)));
                    }
                }
                _ => {}
            }
        }
        if let Some((start, _)) = open_set {
            issues.push(envelope("the set is never closed by SE", &at(start)));
        }
        if let Some((start, _, _)) = open_group {
            issues.push(envelope("the group is never closed by GE", &at(start)));
        }
        issues
    }
}

fn segments(text: &str, s: Separators) -> Result<Vec<Segment>, ValidationIssue> {
    let mut segments = Vec::new();
    for raw in text.split(s.terminator) {
        let raw = raw.trim_start_matches(['\r', '\n', ' ']);
        if raw.is_empty() {
            continue;
        }
        let mut parts = raw.split(s.element);
        let tag = parts.next().unwrap_or_default().to_string();
        let sound = (2..=3).contains(&tag.len())
            && tag
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if !sound {
            return Err(malformed(&format!("{tag:?} is not a segment identifier")));
        }
        let elements = parts
            .map(|element| element.split(s.component).map(str::to_string).collect())
            .collect();
        segments.push(Segment { tag, elements });
    }
    let tail = text.trim_end();
    if !tail.is_empty() && !tail.ends_with(s.terminator) {
        return Err(malformed("the last segment has no terminator"));
    }
    Ok(segments)
}

fn malformed(message: &str) -> ValidationIssue {
    ValidationIssue {
        code: "malformed".to_string(),
        message: message.to_string(),
        path: None,
    }
}

fn envelope(message: &str, path: &str) -> ValidationIssue {
    ValidationIssue {
        code: "envelope".to_string(),
        message: message.to_string(),
        path: Some(path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const SOUND: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       \
*260908*1030*U*00401*000000001*0*P*>~GS*PO*SENDER*RECEIVER*20260908*1030*1*X*004010~\
ST*850*0001~BEG*00*SA*PO4711**20260908~PO1*1*2*EA*10.00**VP*X001~SE*4*0001~\
GE*1*1~IEA*1*000000001~";

    #[test]
    fn a_sound_interchange_reads_and_has_no_issues() {
        let interchange = Interchange::parse(SOUND).expect("parse");
        assert_eq!(interchange.separators().element, '*');
        assert_eq!(interchange.separators().component, '>');
        assert_eq!(interchange.segments().len(), 8);
        assert!(interchange.soundness().is_empty());
        let sets: Vec<_> = interchange.transaction_sets().collect();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].1.simple(1), "850");
        assert_eq!(sets[0].2.expect("group").simple(8), "004010");
        assert_eq!(interchange.segments()[0].simple(13), "000000001");
    }

    #[test]
    fn line_breaks_after_terminators_are_tolerated() {
        let broken = SOUND.replace('~', "~\r\n");
        let interchange = Interchange::parse(&broken).expect("parse");
        assert_eq!(interchange.segments().len(), 8);
        assert!(interchange.soundness().is_empty());
    }

    #[test]
    fn every_envelope_departure_is_named() {
        let wrong_counts = SOUND
            .replace("SE*4*0001", "SE*5*0002")
            .replace("GE*1*1", "GE*2*9")
            .replace("IEA*1*000000001", "IEA*3*000000002");
        let issues = Interchange::parse(&wrong_counts)
            .expect("parse")
            .soundness();
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert_eq!(issues.len(), 6, "{messages:?}");
        assert!(messages[0].starts_with("SE counts 5 segments, the set has 4"));
        assert!(messages[1].starts_with("SE closes 0002, ST opened 0001"));
        assert!(messages[2].starts_with("GE counts 2 sets"));
        assert!(messages[3].starts_with("GE closes 9, GS opened 1"));
        assert!(messages[4].starts_with("IEA counts 3 groups"));
        assert!(messages[5].starts_with("IEA closes 000000002, ISA opened 000000001"));
        assert!(issues.iter().all(|i| i.code == "envelope"));
        assert_eq!(issues[0].path.as_deref(), Some("segment 6 (SE)"));

        let unclosed = SOUND.replace("SE*4*0001~GE*1*1~IEA*1*000000001~", "");
        let issues = Interchange::parse(&unclosed).expect("parse").soundness();
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("does not close with IEA"))
        );
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("never closed by SE"))
        );
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("never closed by GE"))
        );
    }

    #[test]
    fn what_is_not_x12_is_malformed() {
        assert!(Interchange::parse("UNB+UNOC:3+A+B'").is_err());
        assert!(Interchange::parse("ISA*00*short~").is_err());
        assert!(
            Interchange::parse(&SOUND[..ISA_LENGTH - 1]).is_err(),
            "no terminator"
        );
        let bad_tag = SOUND.replace("BEG*", "b*");
        assert!(Interchange::parse(&bad_tag).is_err());
        let same = SOUND.replace("*>~", "*>*");
        assert!(
            Interchange::parse(&same).is_err(),
            "element equals terminator"
        );
    }
}
