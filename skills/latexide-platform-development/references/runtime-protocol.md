# Native v1 reply validation

Read `docs/DEVELOPER_HANDOFF.md` and the latest decoder entry in
`docs/development/results/P00.md`. The current macOS implementation is
`apps/macos/Sources/LaTeXIDEKit/NativeRuntime.swift`, tested by
`apps/macos/Tests/LaTeXIDEKitTests/RuntimeReplyTests.swift`. It consumes the
existing Rust C ABI and single authoritative runtime; it adds no provider,
second workspace, wire field or capability negotiation.

## Preserve the checked adapter contract

`RuntimeReply` checks the 1 MiB encoded envelope, depth limit, strict booleans,
required operation fields and exact UInt64 values. Optional explicit version/op
fields must match. Malformed replies throw `RuntimeFailure`; remote domain errors
retain their machine code and message. Duplicate keys, including escaped aliases,
are rejected. Legacy v1 has no mandatory request/operation ID, so same-shaped
replies are not fully associated by this guard. Manual checks are not a generated
complete operation schema.

Decode all JSON string tokens, including keys and error messages, with
`JSONDecoder`. On the measured macOS27.2 host, Foundation `JSONSerialization`
loses a leading logical U+FEFF before its NSString/String bridge; changing the
byte-count guard would hide the loss. Logical content is separate from the
physical UTF-8 encoding BOM. Preserve literal/escaped, leading/interior and
chunk-boundary U+FEFF bytes. Keep Foundation's exact numeric token representation,
while representing literal raw `-0` as `NSNumber(value: -0.0)`. Existing unsigned
validators must reject it in every typed field and nested syntax-span array;
unknown signed extension fields are not globally forbidden.

`NativeRuntime.readDocument` pins document ID, revision including zero, offsets
and total byte count. Require forward progress, valid UTF-8 byte lengths, bounded
chunks and consistent completion. Return no partial snapshot after failure.
UTF-8 chunk offsets and zero-based UTF-16 edit coordinates are different units.
Keep direct edits, explicit Save and undo after Save. Free the returned C string
with the existing matching function. Concurrent free during an in-flight native
call remains forbidden and unqualified.

## Reproduce in an isolated checkout

Use the existing pinned Rust toolchain and offline dependency cache. Build the
current checkout's static FFI before Swift; never qualify an old library after a
failed build. The Swift package links `target/debug/liblatexide_ffi.a`. A private
external target/scratch/cache may be exposed by ignored generated-output aliases;
source paths remain ordinary repository files. Keep all build jobs serial:

```sh
cargo +1.99.1 build -p latexide-ffi -p latexide-mcp --locked --offline --jobs 1
swift build --package-path apps/macos --scratch-path <own-swift-scratch> \
  --disable-sandbox --build-system native --jobs 1 --build-tests
swift test --package-path apps/macos --scratch-path <own-swift-scratch> \
  --disable-sandbox --build-system native --jobs 1 --skip-build \
  --filter RuntimeReplyTests
```

Bind the exact selected nonzero test identities, fresh static/test binaries,
source/tool/fixture/log hashes and failed attempts. Fake-ABI vector counts,
actual Rust C ABI controls, XCTest methods and the full native suite are separate
cohorts; never add overlapping counts. Stored evidence commands are inert data.
The publication's current base/integration result must be separate from the
producer and same-host P03 review's frozen source/receipt.

Mac linked builds or Foundation/C ABI tests do not qualify live UI, AX/speech,
Windows11/WinUI or Linux runtime/desktop. R2 wire dedup, R3 events, R4 concurrent
pointer lifetime and R5 immutable chunk/memory qualification remain separate.
Full MCP-03/schema/capability/wire-ID negotiation and QA08/QA14/OS10 acceptance
are not granted by a checked adapter. Windows/Linux hosts need their own exact
integer/JSON/native-lifetime implementation and actual runtime evidence.
