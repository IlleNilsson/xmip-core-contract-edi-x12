#![forbid(unsafe_code)]

//! The ASC X12 content contract — a technology of `xmip-core-contract`.
//!
//! Two claims, decided 2026-09-07 (ADR-0042): **well-formedness is a given**
//! and **conformance is a given once a contract is named**.
//!
//! Well-formed here is a *sound interchange*: segments read with the
//! separators `ISA` declares by position; `ISA` opens and `IEA` closes with
//! the same control number and the right group count; every `GS` is closed
//! by a `GE` with the same control number and the right set count; every
//! `ST` by an `SE` with the same control number and the right segment count.
//! That is what a partner's interchange must satisfy before any transaction
//! set in it means anything.
//!
//! Conformance is the *transaction set*: a Location that names this contract
//! with `850:004010` bound has every set's `ST01` held to that identifier and
//! its group's `GS08` to that version, and `850` alone holds the identifier.
//! Every X12 version is a version of this one technology and lives in this
//! repository, as EDIFACT's directories do in its; the segment tables that
//! would hold a set to its version's structure are the next layer here.

pub mod syntax;

use contract::{
    Contract, ContractDescriptor, ContractError, ContractFactory, ContractId, ValidationIssue,
    ValidationResult,
};
use stream::Stream;
use syntax::{Interchange, Segment};

/// The bound transaction set: identifier, and optionally the version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionSet {
    pub identifier: String,
    pub version: Option<String>,
}

impl TransactionSet {
    /// `850`, `850:004010`, `837:005010X222A1`.
    ///
    /// # Errors
    /// An identifier that is not three digits, or more than two parts.
    pub fn parse(reference: &str) -> Result<Self, ContractError> {
        let parts: Vec<&str> = reference.split(':').map(str::trim).collect();
        let identifier = parts[0];
        let sound = identifier.len() == 3 && identifier.bytes().all(|b| b.is_ascii_digit());
        match parts.as_slice() {
            [_] if sound => Ok(Self {
                identifier: identifier.to_string(),
                version: None,
            }),
            [_, version] if sound && !version.is_empty() => Ok(Self {
                identifier: identifier.to_string(),
                version: Some(version.to_ascii_uppercase()),
            }),
            _ => Err(ContractError {
                message: format!("{reference:?} is not SET or SET:VERSION, the set three digits"),
            }),
        }
    }

    fn reference(&self) -> String {
        match &self.version {
            Some(version) => format!("{}:{version}", self.identifier),
            None => self.identifier.clone(),
        }
    }
}

/// The X12 contract, bare or bound to a transaction set.
pub struct X12 {
    descriptor: ContractDescriptor,
    transaction_set: Option<TransactionSet>,
}

impl X12 {
    /// A sound interchange, of any transaction sets.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: descriptor("edi-x12"),
            transaction_set: None,
        }
    }

    /// A sound interchange whose every set is `transaction_set`.
    #[must_use]
    pub fn of(transaction_set: TransactionSet) -> Self {
        Self {
            descriptor: descriptor(&format!("edi-x12:{}", transaction_set.reference())),
            transaction_set: Some(transaction_set),
        }
    }

    /// Whether a transaction set is bound.
    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.transaction_set.is_some()
    }
}

impl Default for X12 {
    fn default() -> Self {
        Self::new()
    }
}

fn descriptor(id: &str) -> ContractDescriptor {
    ContractDescriptor {
        id: ContractId(id.to_string()),
        version: "1".to_string(),
        representation: "application/EDI-X12".to_string(),
    }
}

impl Contract for X12 {
    fn descriptor(&self) -> &ContractDescriptor {
        &self.descriptor
    }

    fn identify(&self, stream: &Stream) -> Result<bool, ContractError> {
        if stream.media_type().is_some_and(|m| {
            m.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/EDI-X12")
        }) {
            return Ok(true);
        }
        Ok(stream.bytes().starts_with(b"ISA"))
    }

    fn validate(&self, stream: &Stream) -> Result<ValidationResult, ContractError> {
        let text = match std::str::from_utf8(stream.bytes()) {
            Ok(text) => text,
            Err(error) => {
                return Ok(result(vec![issue(
                    "malformed",
                    &format!("not text: {error}"),
                    None,
                )]));
            }
        };
        let interchange = match Interchange::parse(text) {
            Ok(interchange) => interchange,
            Err(unsound) => return Ok(result(vec![unsound])),
        };
        let mut issues = interchange.soundness();
        if let Some(wanted) = &self.transaction_set {
            for (ordinal, header, group) in interchange.transaction_sets() {
                if let Some(message) = mismatch(wanted, header, group) {
                    issues.push(issue(
                        "transaction-set",
                        &message,
                        Some(format!("set {ordinal}")),
                    ));
                }
            }
        }
        Ok(result(issues))
    }
}

/// Why an `ST` is not the bound set, or `None` when it is.
fn mismatch(wanted: &TransactionSet, header: &Segment, group: Option<&Segment>) -> Option<String> {
    let identifier = header.simple(1);
    if identifier != wanted.identifier {
        return Some(format!(
            "is {identifier}, the contract is {}",
            wanted.identifier
        ));
    }
    if let Some(version) = &wanted.version {
        let actual = group.map_or("", |g| g.simple(8)).to_ascii_uppercase();
        if actual != *version {
            return Some(format!(
                "is {identifier} {actual}, the contract is {}",
                wanted.reference()
            ));
        }
    }
    None
}

fn issue(code: &str, message: &str, path: Option<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        message: message.to_string(),
        path,
    }
}

fn result(issues: Vec<ValidationIssue>) -> ValidationResult {
    ValidationResult {
        valid: issues.is_empty(),
        issues,
    }
}

/// Loads the contract a Location names: an empty reference is the bare
/// contract, anything else a transaction set, `850` or `850:004010`.
pub struct X12Factory;

impl ContractFactory for X12Factory {
    fn technology(&self) -> &'static str {
        "edi-x12"
    }

    fn load(&self, reference: &str) -> Result<Box<dyn Contract>, ContractError> {
        if reference.trim().is_empty() {
            return Ok(Box::new(X12::new()));
        }
        Ok(Box::new(X12::of(TransactionSet::parse(reference)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcore::StreamId;

    const ORDER: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       \
*260908*1030*U*00401*000000001*0*P*>~GS*PO*SENDER*RECEIVER*20260908*1030*1*X*004010~\
ST*850*0001~BEG*00*SA*PO4711**20260908~PO1*1*2*EA*10.00**VP*X001~SE*4*0001~\
GE*1*1~IEA*1*000000001~";

    fn stream(text: &str, media_type: Option<&str>) -> Stream {
        Stream::new(
            StreamId::new(1),
            text.as_bytes().to_vec(),
            media_type.map(str::to_string),
        )
    }

    #[test]
    fn a_sound_interchange_holds_bare_and_bound() {
        let bare = X12::new();
        assert!(bare.identify(&stream(ORDER, None)).expect("identify"));
        assert!(
            bare.identify(&stream("x", Some("application/edi-x12; charset=us-ascii")))
                .expect("identify")
        );
        assert!(!bare.identify(&stream("UNB+", None)).expect("identify"));
        assert!(bare.validate(&stream(ORDER, None)).expect("validate").valid);
        let bound = X12Factory.load("850:004010").expect("load");
        assert_eq!(bound.descriptor().id.0, "edi-x12:850:004010");
        assert!(
            bound
                .validate(&stream(ORDER, None))
                .expect("validate")
                .valid
        );
        assert!(X12::of(TransactionSet::parse("850").expect("parse")).is_bound());
        assert!(
            !X12Factory
                .load(" ")
                .expect("bare")
                .descriptor()
                .id
                .0
                .contains(':')
        );
    }

    #[test]
    fn a_set_or_version_that_is_not_the_contract_is_named() {
        let bound = X12::of(TransactionSet::parse("810:004010").expect("parse"));
        let result = bound.validate(&stream(ORDER, None)).expect("validate");
        assert!(!result.valid);
        assert_eq!(result.issues[0].code, "transaction-set");
        assert_eq!(result.issues[0].message, "is 850, the contract is 810");
        assert_eq!(result.issues[0].path.as_deref(), Some("set 1"));
        let bound = X12::of(TransactionSet::parse("850:005010").expect("parse"));
        let result = bound.validate(&stream(ORDER, None)).expect("validate");
        assert_eq!(
            result.issues[0].message,
            "is 850 004010, the contract is 850:005010"
        );
    }

    #[test]
    fn what_is_unsound_does_not_hold() {
        let result = X12::new()
            .validate(&stream(&ORDER.replace("IEA*1*", "IEA*2*"), None))
            .expect("validate");
        assert!(!result.valid);
        assert_eq!(result.issues[0].code, "envelope");
        let result = X12::new()
            .validate(&stream("ISA*short~", None))
            .expect("validate");
        assert_eq!(result.issues[0].code, "malformed");
        let binary = Stream::new(StreamId::new(1), vec![0xff, 0xfe], None);
        assert!(!X12::new().validate(&binary).expect("validate").valid);
        assert!(TransactionSet::parse("85").is_err());
        assert!(TransactionSet::parse("850:").is_err());
        assert!(TransactionSet::parse("a:b:c").is_err());
    }
}
