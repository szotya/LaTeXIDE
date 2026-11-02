# P09 controlled offline artifact fixtures

`known-good.pdf` is an existing one-page XeTeX/xdvipdfmx output from the repository's
controlled `tests/fixtures/basic/main.tex` example. It was copied from the historical
local `evidence/acquisition/compiler-review/texlive-core-fixture/output/main.pdf`.
It contains the synthetic “Natív LaTeX IDE / Helyi mintaprojekt” example, without
runtime/session data. P09 did not launch that compiler or regenerate the PDF.

| File | Bytes | SHA256 | Purpose |
| --- | ---: | --- | --- |
| known-good.pdf | 12406 | 008864ba823685a89b895f49f9341e5b055a26eafed88b9b48319ee19702f2a1 | Existing real PDF, positive parser/oracle control |
| bad-page.pdf | 410 | 7786aa3e40541b169634a9cd893fe7029e58e13e83b1f591dd9b48b78505294b | Synthetic page with zero-size MediaBox |
| bad-stream.pdf | 441 | 5251d9231b004d3b0dcfa469f372d2dccd092edda0c8dc9d255c20740f721166 | Synthetic broken FlateDecode page stream |

The two negative fixtures use four simple objects and explicit xref offsets. This
fixture construction is not a PDF validator. Empty, five-byte, header-only and
truncated cases are derived directly in `tests/pipeline_contract.rs`.

Fresh P09 oracle: existing offline Poppler `pdfinfo -box` reports one unencrypted
A4 page, PDF 1.7; `pdftoppm` renders that page successfully. Its rendered image was
also inspected. The native adapter separately opens the same exact bytes, checks
all pages and decodes/scans their page content streams. The local report records
actual executable/fixture hashes, commands and logs. An optional `pdftotext` probe
could not start because that executable is absent; it is not oracle evidence.

Limits: CoreGraphics repairs some malformed data. This first slice rejects a
non-empty stream that decodes to zero bytes, including valid compressed-empty
streams. It does not establish ISO PDF conformance, nested-resource/font/image
integrity, arbitrary damaged-file acceptance, rendering parity or parser isolation.
Windows/Linux require their own real parser/digest adapter and actual test run.

The service tests supply synthetic manifests, completion and compiler identities;
they do not establish that a compiler produced this PDF from those test inputs.
Real input capture and final-pass provenance are separate host integrations.


## Publication context — 2026-11-02

The Poppler/render observations above belong to the historical first-slice producer
on d5ab; they are not a new compiler/render run on the publication base. The fresh
publisher ran the exact 19 macOS service controls and 32 contracts, included in a
151-test Rust workspace regression, with a new test executable and compiler-bound
three-Rust/three-PDF input set. No PDF was regenerated and no renderer output is
versioned. See `docs/development/results/P09.md` for the current cohort, the separate
408-source review applicability and unchanged parser/resource/binary-input limits.
This in-process native adapter remains an unregistered controlled-fixture policy,
not a general untrusted-PDF validator, memory-isolated parser or compiler oracle.
