use latexide_contracts::bibliography::{BibliographyPort, BibliographyReport};
use latexide_core::services::bibliography::{FragmentKind, LocalBibliographyProvider, inspect_bibliography, parse_bibliography};

#[path = "fixtures/bibliography/golden_expected.rs"]
mod golden;
#[path = "fixtures/bibliography/binding_hash.rs"]
mod binding_hash;
#[path = "fixtures/bibliography/binding_pins.rs"]
mod binding_pins;

fn fixture(id: &str) -> golden::Golden { golden::fixture(id) }

fn check(id: &str) {
    let row = fixture(id);
    let source = row.source;
    let expected: &BibliographyReport = &row.report;
    let document = parse_bibliography(source);
    assert_eq!(document.source().as_bytes(), source.as_bytes(), "SOURCE_BYTES");
    assert_eq!(document.render().as_bytes(), source.as_bytes(), "LOSSLESS_RENDER");
    let provider: &dyn BibliographyPort = &LocalBibliographyProvider;
    let actual = provider.inspect_bibliography(source);
    assert_eq!(actual.entries.len(), expected.entries.len(), "BIB_ENTRIES");
    assert_eq!(&actual, expected, "GOLDEN_REPORT: {id}");
    assert_eq!(&inspect_bibliography(source), expected, "FREE_FUNCTION: {id}");
    assert_eq!(document.report(), expected, "LOSSLESS_REPORT: {id}");
    let mut cursor = 0;
    let mut kinds = Vec::new();
    for part in document.fragments() {
        assert_eq!(part.bytes.start, cursor, "NO_DROPPED_TRIVIA");
        assert!(part.bytes.end > part.bytes.start);
        assert_eq!(part.raw.as_bytes(), &source.as_bytes()[part.bytes.clone()]);
        cursor = part.bytes.end;
        kinds.push(match part.kind { FragmentKind::Trivia => "trivia", FragmentKind::Entry => "entry", FragmentKind::Directive => "directive", FragmentKind::Malformed => "malformed" });
    }
    assert_eq!(cursor, source.len(), "FULL_BYTE_PARTITION");
    assert_eq!(kinds, row.fragment_kinds, "FRAGMENT_ROLES");
    assert_eq!(source.encode_utf16().count(), row.source_utf16_length);
}

#[test] fn golden_01_simple() { check("golden_01_simple"); }
#[test] fn golden_02_duplicate_keys() { check("golden_02_duplicate_keys"); }
#[test] fn golden_03_nested_brace() { check("golden_03_nested_brace"); }
#[test] fn golden_04_escaped_quote() { check("golden_04_escaped_quote"); }
#[test] fn golden_05_comment() { check("golden_05_comment"); }
#[test] fn golden_06_macro_concat() { check("golden_06_macro_concat"); }
#[test] fn golden_07_missing_close() { check("golden_07_missing_close"); }
#[test] fn golden_08_partial_field() { check("golden_08_partial_field"); }
#[test] fn golden_09_astral_unicode() { check("golden_09_astral_unicode"); }
#[test] fn golden_10_logical_feff() { check("golden_10_logical_feff"); }
#[test] fn golden_11_crlf() { check("golden_11_crlf"); }
#[test] fn golden_12_empty() { check("golden_12_empty"); }

#[test]
fn binding_01_fixture_byte_hashes() {
    // Known digests are independent producer literals, not parser/getter output.
    assert_eq!(binding_hash::sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(binding_hash::sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    for id in golden::CASE_NAMES {
        let row = fixture(id);
        assert_eq!(binding_hash::sha256_hex(row.source.as_bytes()), row.source_sha256, "FROZEN_FIXTURE_HASH: {id}");
        let mut corrupted = row.source.as_bytes().to_vec();
        corrupted.push(b'x');
        assert_ne!(binding_hash::sha256_hex(&corrupted), row.source_sha256, "CORRUPTED_FIXTURE_NOT_ACCEPTED");
    }
}

#[test]
fn binding_02_frozen_oracle_and_generated_literals() {
    let original = include_bytes!("fixtures/bibliography/goldens.json");
    let generated = include_bytes!("fixtures/bibliography/golden_expected.rs");
    assert_eq!(binding_hash::sha256_hex(original), binding_pins::ORIGINAL_ORACLE_SHA256);
    assert_eq!(binding_hash::sha256_hex(generated), binding_pins::GENERATED_EXPECTATIONS_SHA256);
    let names = golden::CASE_NAMES.into_iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), 12);
    assert_eq!(fixture("golden_02_duplicate_keys").report.duplicate_keys, vec!["dup".to_owned()]);
}

#[test]
fn binding_03_frozen_provider_and_contract_sources() {
    let files: [(&[u8], &str); 4] = [
        (include_bytes!("../src/services/bibliography/fingerprint.rs"), binding_pins::FINGERPRINT_SHA256),
        (include_bytes!("../src/services/bibliography/mod.rs"), binding_pins::PROVIDER_MOD_SHA256),
        (include_bytes!("../src/services/bibliography/parser.rs"), binding_pins::PARSER_SHA256),
        (include_bytes!("../../latexide-contracts/src/bibliography.rs"), binding_pins::CONTRACT_SHA256),
    ];
    for (bytes, expected) in files { assert_eq!(binding_hash::sha256_hex(bytes), expected, "FROZEN_PROVIDER_CONTRACT_SOURCE"); }
}

#[test]
fn binding_04_canonical_library_hook_and_cargo() {
    assert_eq!(env!("CARGO_PKG_NAME"), "latexide-core", "CANONICAL_PACKAGE_NOT_STANDALONE_ALIAS");
    assert_eq!(std::any::type_name::<LocalBibliographyProvider>(), "latexide_core::services::bibliography::LocalBibliographyProvider");
    let files: [(&[u8], &str); 4] = [
        (include_bytes!("../src/lib.rs"), binding_pins::CANONICAL_LIB_SHA256),
        (include_bytes!("../Cargo.toml"), binding_pins::CORE_CARGO_SHA256),
        (include_bytes!("../../../Cargo.toml"), binding_pins::ROOT_CARGO_SHA256),
        (include_bytes!("../../../Cargo.lock"), binding_pins::ROOT_LOCK_SHA256),
    ];
    for (bytes, expected) in files { assert_eq!(binding_hash::sha256_hex(bytes), expected, "CANONICAL_HOOK_CARGO_BINDING"); }
}

#[test]
#[should_panic(expected = "WRONG_UTF16_SPAN")]
fn guard_wrong_utf16_span() {
    let row = fixture("golden_09_astral_unicode");
    let source = row.source;
    let expected = row.report;
    let mut forged = inspect_bibliography(source);
    assert_eq!(forged, expected, "GUARD_PRECONDITION");
    forged.entries[0].range.start = source.find('@').unwrap() as u32;
    assert_eq!(forged, expected, "WRONG_UTF16_SPAN");
}

#[test]
#[should_panic(expected = "LOST_TRIVIA")]
fn guard_lost_trivia() {
    let row = fixture("golden_05_comment");
    let source = row.source;
    let parsed = parse_bibliography(source);
    assert_eq!(parsed.render(), source, "GUARD_PRECONDITION");
    let forged: String = parsed.fragments().iter().filter(|part| part.kind == FragmentKind::Entry).map(|part| part.raw).collect();
    assert_eq!(forged.as_bytes(), source.as_bytes(), "LOST_TRIVIA");
}

#[path = "fixtures/bibliography/recovery_expected.rs"]
mod recovery;

fn check_recovery(id: &str) {
    let row = recovery::fixture(id);
    let source = row.source;
    let document = parse_bibliography(source);
    assert_eq!(document.source().as_bytes(), source.as_bytes(), "RECOVERY_SOURCE_BYTES");
    assert_eq!(document.render().as_bytes(), source.as_bytes(), "RECOVERY_RAW_BYTES");
    assert_eq!(binding_hash::sha256_hex(source.as_bytes()), row.source_sha256, "RECOVERY_SOURCE_HASH");
    let provider: &dyn BibliographyPort = &LocalBibliographyProvider;
    let actual = provider.inspect_bibliography(source);
    if !row.report.diagnostics.is_empty() {
        assert!(actual.diagnostics.iter().any(|d| d.code == "missing_entry_delimiter"), "MALFORMED_HEADER_DIAGNOSTIC");
    }
    let actual_keys: Vec<_> = actual.entries.iter().map(|e| e.key.as_str()).collect();
    let expected_keys: Vec<_> = row.report.entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(actual_keys, expected_keys, "NEXT_ENTRY_RECOVERED");
    assert_eq!(actual, row.report, "RECOVERY_REPORT: {id}");
    assert_eq!(inspect_bibliography(source), row.report, "RECOVERY_FREE_FUNCTION");
    assert_eq!(document.report(), &row.report, "RECOVERY_PARTITION_REPORT");
    let mut cursor = 0;
    let mut kinds = Vec::new();
    for part in document.fragments() {
        assert_eq!(part.bytes.start, cursor, "RECOVERY_CONTIGUOUS_PARTITION");
        assert!(part.bytes.end > part.bytes.start);
        assert_eq!(part.raw.as_bytes(), &source.as_bytes()[part.bytes.clone()]);
        cursor = part.bytes.end;
        kinds.push(match part.kind { FragmentKind::Trivia => "trivia", FragmentKind::Entry => "entry", FragmentKind::Directive => "directive", FragmentKind::Malformed => "malformed" });
    }
    assert_eq!(cursor, source.len(), "RECOVERY_FULL_SOURCE_PARTITION");
    assert_eq!(kinds, row.fragment_kinds, "RECOVERY_FRAGMENT_ROLES");
    assert_eq!(source.encode_utf16().count(), row.source_utf16_length);
}

#[test] fn recovery_01_malformed_next_entry() { check_recovery("recovery_01_malformed_next_entry"); }
#[test] fn recovery_02_valid_nextline_opener() { check_recovery("recovery_02_valid_nextline_opener"); }
#[test] fn recovery_03_nested_quoted_literal() { check_recovery("recovery_03_nested_quoted_literal"); }
#[test] fn recovery_04_comment_boundary() { check_recovery("recovery_04_comment_boundary"); }
