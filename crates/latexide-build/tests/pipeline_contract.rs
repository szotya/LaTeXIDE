#[path = "../src/pipeline/mod.rs"]
mod pipeline;

// Controlled, offline service inputs. No compiler, host capture or runtime registration.
#[cfg(target_os = "macos")]
mod macos {
    use super::pipeline::{
        ArtifactChecks, ArtifactEvaluator, ArtifactLimits, ArtifactRecord, BuildCompletion,
        CheckError, CoreGraphicsChecks, FinalPass, UnavailableChecks,
    };
    use latexide_contracts::{
        SnapshotFile, SnapshotLimits, WorkspaceSnapshot,
        build::{ArtifactError, ArtifactState, BuildInput, BuildManifest, BuildPort},
    };

    const PDF: &[u8] = include_bytes!("../src/pipeline/fixtures/known-good.pdf");
    const BAD_PAGE: &[u8] = include_bytes!("../src/pipeline/fixtures/bad-page.pdf");
    const BAD_DATA: &[u8] = include_bytes!("../src/pipeline/fixtures/bad-stream.pdf");
    const PDF_SHA: &str = "008864ba823685a89b895f49f9341e5b055a26eafed88b9b48319ee19702f2a1";
    const COMPILER: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    const OTHER_COMPILER: &str = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";

    fn file(path: &str, bytes: &[u8], revision: Option<u64>) -> SnapshotFile {
        SnapshotFile {
            path: path.into(),
            revision,
            content_sha256: CoreGraphicsChecks::default().sha256(bytes).unwrap(),
            utf8: bytes.to_vec(),
            dirty: false,
        }
    }
    fn inputs() -> (BuildManifest, WorkspaceSnapshot) {
        let files = vec![
            file("main.tex", b"main\n", Some(7)),
            file("chapter.tex", b"chapter\n", None),
            file("references.bib", b"@book{key,title={Book}}\n", None),
            file("style.sty", b"% style\n", None),
        ];
        let manifest = BuildManifest {
            generation: "generation-7".into(),
            snapshot_id: "snapshot-7".into(),
            inputs: files
                .iter()
                .map(|f| BuildInput {
                    path: f.path.clone(),
                    content_sha256: f.content_sha256.clone(),
                })
                .collect(),
            compiler_sha256: COMPILER.into(),
        };
        (
            manifest,
            WorkspaceSnapshot {
                id: "snapshot-7".into(),
                epoch: 7,
                files,
            },
        )
    }
    fn record(manifest: &BuildManifest) -> ArtifactRecord {
        ArtifactRecord {
            manifest: manifest.clone(),
            snapshot_epoch: 7,
            pdf_sha256: PDF_SHA.into(),
            completion: BuildCompletion::Exited {
                code: 0,
                final_pass: FinalPass::Complete,
            },
        }
    }
    fn evaluate(
        m: &BuildManifest,
        c: &WorkspaceSnapshot,
        pdf: &[u8],
    ) -> Result<ArtifactState, ArtifactError> {
        ArtifactEvaluator::new(
            &record(m),
            COMPILER,
            &CoreGraphicsChecks::default(),
            ArtifactLimits::default(),
        )
        .evaluate_artifact(m, c, pdf)
    }
    fn change(c: &mut WorkspaceSnapshot, index: usize, bytes: &[u8]) {
        c.id = "snapshot-8".into();
        c.epoch = 8;
        c.files[index].utf8 = bytes.to_vec();
        c.files[index].content_sha256 = CoreGraphicsChecks::default().sha256(bytes).unwrap();
    }

    #[test]
    fn contract_v1_first_slice() {
        let (m, c) = inputs();
        let nonpdf = evaluate(&m, &c, b"abcde"); // controlled exit-0 input
        let mut changed = c.clone();
        assert_eq!(changed.files[1].revision, None);
        change(&mut changed, 1, b"changed chapter\n");
        assert_eq!(
            (nonpdf, evaluate(&m, &changed, PDF)),
            (Err(ArtifactError::InvalidPdf), Ok(ArtifactState::Stale))
        );
    }
    #[test]
    fn current_fixture_is_parsed_and_bound_to_exact_bytes() {
        let checks = CoreGraphicsChecks::default();
        let (m, c) = inputs();
        assert_eq!(checks.sha256(PDF).unwrap(), PDF_SHA);
        assert_eq!(checks.validate_pdf(PDF), Ok(()));
        assert_eq!(evaluate(&m, &c, PDF), Ok(ArtifactState::Current));
    }
    #[test]
    fn review_focus_r1() {
        let (m, c) = inputs();
        let cases: &[(&str, &[u8])] = &[
            ("empty", b""),
            ("five-bytes", b"abcde"),
            ("header-only", b"%PDF-1.7\n%%EOF\n"),
            ("truncated-header", &PDF[..64]),
            ("truncated-tail", &PDF[..PDF.len() - 32]),
            ("bad-page", BAD_PAGE),
            ("bad-stream", BAD_DATA),
        ];
        for (name, pdf) in cases {
            assert_eq!(
                evaluate(&m, &c, pdf),
                Err(ArtifactError::InvalidPdf),
                "{name}"
            );
        }
    }
    #[test]
    fn review_focus_r2() {
        let (m, c) = inputs();
        for i in 1..c.files.len() {
            assert_eq!(c.files[i].revision, None);
            let mut changed = c.clone();
            change(&mut changed, i, b"changed unopened logical input\n");
            assert_eq!(
                evaluate(&m, &changed, PDF),
                Ok(ArtifactState::Stale),
                "{}",
                c.files[i].path
            );
        }
    }
    #[test]
    fn review_focus_r3() {
        let (m, c) = inputs();
        let original = (m.clone(), c.clone(), PDF.to_vec());
        let r = record(&m);
        let checks = CoreGraphicsChecks::default();
        let s = ArtifactEvaluator::new(&r, COMPILER, &checks, ArtifactLimits::default());
        let mut changed = c.clone();
        change(&mut changed, 0, b"dirty build-time edit\n");
        changed.files[0].revision = Some(8);
        changed.files[0].dirty = true;
        assert_eq!(
            s.evaluate_artifact(&m, &changed, PDF),
            Ok(ArtifactState::Stale)
        );
        let mut wrong = m.clone();
        wrong.generation = "generation-8".into();
        assert_eq!(
            s.evaluate_artifact(&wrong, &c, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
        assert_eq!((m, c, PDF.to_vec()), original);
    }
    #[test]
    fn review_focus_r4() {
        let (m, c) = inputs();
        let checks = CoreGraphicsChecks::default();
        let previous = PDF.to_vec();
        let original = previous.clone();
        for completion in [
            BuildCompletion::Cancelled,
            BuildCompletion::TimedOut,
            BuildCompletion::Exited {
                code: 1,
                final_pass: FinalPass::Complete,
            },
        ] {
            let mut r = record(&m);
            r.completion = completion;
            let s = ArtifactEvaluator::new(&r, COMPILER, &checks, ArtifactLimits::default());
            assert_eq!(
                s.evaluate_artifact(&m, &c, PDF),
                Err(ArtifactError::InconsistentSnapshot)
            );
            assert_eq!(previous, original);
            assert_eq!(checks.validate_pdf(&previous), Ok(()));
        }
        assert_eq!(evaluate(&m, &c, b"abcde"), Err(ArtifactError::InvalidPdf));
        assert_eq!(previous, original);
    }
    #[test]
    fn review_focus_r5() {
        let (m, c) = inputs();
        let checks = CoreGraphicsChecks::default();
        for final_pass in [FinalPass::NeedsRerun, FinalPass::MissingDependency] {
            let mut r = record(&m);
            r.completion = BuildCompletion::Exited {
                code: 0,
                final_pass,
            };
            assert_eq!(
                ArtifactEvaluator::new(&r, COMPILER, &checks, ArtifactLimits::default())
                    .evaluate_artifact(&m, &c, PDF),
                Err(ArtifactError::InconsistentSnapshot)
            );
        }
    }
    #[test]
    fn missing_input_preserves_the_old_pdf() {
        let (m, mut c) = inputs();
        c.files.remove(1);
        let old = PDF.to_vec();
        assert_eq!(evaluate(&m, &c, PDF), Err(ArtifactError::MissingInput));
        assert_eq!(PDF, old);
    }
    #[test]
    fn independent_current_compiler_identity() {
        let (m, c) = inputs();
        let r = record(&m);
        let checks = CoreGraphicsChecks::default();
        let s = ArtifactEvaluator::new(&r, OTHER_COMPILER, &checks, ArtifactLimits::default());
        assert_eq!(s.evaluate_artifact(&m, &c, PDF), Ok(ArtifactState::Stale));
        let mut wrong = m.clone();
        wrong.compiler_sha256 = OTHER_COMPILER.into();
        assert_eq!(
            s.evaluate_artifact(&wrong, &c, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
    }
    #[test]
    fn every_recorded_input_and_artifact_identity_is_checked() {
        let (m, c) = inputs();
        let r = record(&m);
        let checks = CoreGraphicsChecks::default();
        let s = ArtifactEvaluator::new(&r, COMPILER, &checks, ArtifactLimits::default());
        let mut omitted = m.clone();
        omitted.inputs.pop();
        let mut snapshot = m.clone();
        snapshot.snapshot_id = "other-snapshot".into();
        let mut hash = m.clone();
        hash.inputs[1].content_sha256 = COMPILER.into();
        for wrong in [omitted, snapshot, hash] {
            assert_eq!(
                s.evaluate_artifact(&wrong, &c, PDF),
                Err(ArtifactError::InconsistentSnapshot)
            );
        }
        let mut other = PDF.to_vec();
        other.push(b'\n');
        assert_eq!(checks.validate_pdf(&other), Ok(()));
        assert_eq!(
            s.evaluate_artifact(&m, &c, &other),
            Err(ArtifactError::InconsistentSnapshot)
        );
    }
    #[test]
    fn input_order_is_not_identity() {
        let (mut m, mut c) = inputs();
        let r = record(&m);
        let checks = CoreGraphicsChecks::default();
        m.inputs.reverse();
        c.files.reverse();
        assert_eq!(
            ArtifactEvaluator::new(&r, COMPILER, &checks, ArtifactLimits::default())
                .evaluate_artifact(&m, &c, PDF),
            Ok(ArtifactState::Current)
        );
    }
    #[test]
    fn immutable_snapshot_identity_is_enforced() {
        let (m, c) = inputs();
        let mut changed = c.clone();
        change(&mut changed, 1, b"different\n");
        changed.id = m.snapshot_id.clone();
        assert_eq!(
            evaluate(&m, &changed, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
        let mut epoch = c.clone();
        epoch.epoch = 8;
        assert_eq!(
            evaluate(&m, &epoch, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
        let mut fresh = c.clone();
        fresh.id = "snapshot-8".into();
        fresh.epoch = 8;
        assert_eq!(evaluate(&m, &fresh, PDF), Ok(ArtifactState::Stale));
    }
    #[test]
    fn producer_digest_is_recomputed() {
        let (m, mut c) = inputs();
        c.files[1].utf8 = b"changed with old digest\n".to_vec();
        assert_eq!(
            evaluate(&m, &c, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
    }
    #[test]
    fn invalid_manifest_is_rejected() {
        let (m, c) = inputs();
        let mut cases = vec![];
        let mut v = m.clone();
        v.generation.clear();
        cases.push(v);
        let mut v = m.clone();
        v.snapshot_id = "bad\nidentity".into();
        cases.push(v);
        let mut v = m.clone();
        v.compiler_sha256 = "not-a-digest".into();
        cases.push(v);
        let mut v = m.clone();
        v.inputs.push(v.inputs[0].clone());
        cases.push(v);
        let mut v = m.clone();
        v.inputs[1].path = "../outside.tex".into();
        cases.push(v);
        let mut v = m.clone();
        v.inputs.clear();
        cases.push(v);
        for v in cases {
            assert_eq!(
                evaluate(&v, &c, PDF),
                Err(ArtifactError::InconsistentSnapshot)
            );
        }
    }
    #[test]
    fn invalid_snapshot_and_binary_assets_are_rejected() {
        let (m, c) = inputs();
        let mut cases = vec![];
        let mut v = c.clone();
        v.files.push(v.files[0].clone());
        cases.push(v);
        let mut v = c.clone();
        v.files[1] = file("chapter.tex", b"raw\r\n", None);
        cases.push(v);
        let mut v = c.clone();
        v.files[1].dirty = true;
        cases.push(v);
        let mut v = c.clone();
        v.files
            .push(file("image.png", &[137, 80, 78, 71, 13, 10, 26, 10], None));
        cases.push(v);
        for v in cases {
            assert_eq!(
                evaluate(&m, &v, PDF),
                Err(ArtifactError::InconsistentSnapshot)
            );
        }
        // A binary-image/font input provider is still required; a logical UTF-8 DTO cannot supply it.
    }
    #[test]
    fn unicode_logical_feff_and_large_revision_are_preserved() {
        let (mut m, mut c) = inputs();
        c.files[0] = file(
            "árvíz 😀.tex",
            "\u{feff}Árvíztűrő 😀 e\u{301}\n".as_bytes(),
            Some(9_007_199_254_740_993),
        );
        c.files[0].dirty = true;
        m.inputs[0] = BuildInput {
            path: c.files[0].path.clone(),
            content_sha256: c.files[0].content_sha256.clone(),
        };
        let before = c.clone();
        assert_eq!(evaluate(&m, &c, PDF), Ok(ArtifactState::Current));
        assert_eq!(c, before);
    }
    #[test]
    fn explicit_budgets_fail_closed() {
        let (m, c) = inputs();
        let r = record(&m);
        let checks = CoreGraphicsChecks::default();
        let limits = ArtifactLimits {
            snapshot: SnapshotLimits {
                max_files: 2,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 8 * 1024 * 1024,
            },
            max_pdf_bytes: 16 * 1024 * 1024,
        };
        assert_eq!(
            ArtifactEvaluator::new(&r, COMPILER, &checks, limits).evaluate_artifact(&m, &c, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
        let mut limits = ArtifactLimits::default();
        limits.max_pdf_bytes = PDF.len() - 1;
        assert_eq!(
            ArtifactEvaluator::new(&r, COMPILER, &checks, limits).evaluate_artifact(&m, &c, PDF),
            Err(ArtifactError::InvalidPdf)
        );
        let mut huge = c.clone();
        huge.files[0] = file("main.tex", &vec![b'a'; 1024 * 1024 + 1], Some(7));
        assert_eq!(
            evaluate(&m, &huge, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
    }
    #[test]
    fn missing_backend_cannot_accept() {
        let (m, c) = inputs();
        let r = record(&m);
        assert_eq!(
            UnavailableChecks.validate_pdf(PDF),
            Err(CheckError::Unavailable)
        );
        assert_eq!(
            ArtifactEvaluator::new(&r, COMPILER, &UnavailableChecks, ArtifactLimits::default())
                .evaluate_artifact(&m, &c, PDF),
            Err(ArtifactError::InconsistentSnapshot)
        );
    }
    #[test]
    fn sha256_known_vectors() {
        let checks = CoreGraphicsChecks::default();
        for (bytes, expected) in [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".as_slice(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
        ] {
            assert_eq!(checks.sha256(bytes).unwrap(), expected);
        }
    }
}
