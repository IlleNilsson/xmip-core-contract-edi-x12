# xmip-core-contract-edi-x12

ASC X12 content contract: a sound interchange always, of a bound transaction set and version when a Location names one. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

An interchange is read by the Foundation's one segment walk
([xmip-core-message](https://github.com/IlleNilsson/xmip-core-message)'s
`segment`, which the X12 shape sections by too), a segment at a time with
the separators its `ISA` declares, a line break among them:
`syntax::Interchange::walk` hands it the separators and holds each
identifier to two or three capitals or digits. The `ISA` is read by the X12
shape's one reader
([xmip-core-message-edi-x12](https://github.com/IlleNilsson/xmip-core-message-edi-x12)'s
`delimiters`), the shape of the same standard and the one technology this
contract depends on (ADR-0044, amendment 2026-11-15). The contract walks the
interchange whole into the segments it checks, and the `edi` path technology
([xmip-core-path-edi](https://github.com/IlleNilsson/xmip-core-path-edi))
cuts only the segment it reads and walks the rest uncut, so a malformed
interchange is refused in the same words by both.

## The declaration

What a Location naming this contract is configured with is two `const`s in
its `declaration` module, `src/declaration.rs`, beside the code, which its
`ContractFactory` reads (ADR-0064, amendment 2026-11-26):
`declaration::SETTINGS`, `reference`, the transaction set an interchange
carries, and `NAME`, `edi-x12`, the name a Validate step's `format` names it
by. Built with its `declaration` feature alone (`--no-default-features
--features declaration`), the crate is those and nothing more: its code and
every crate the code is built on are behind the default feature. So the
library the surfaces load can carry it without the code, for the language
server and the desktop to suggest and check a Location's contract settings
though no node in their process loaded the technology, once the gathering
crate of open problem 48 links it. `test/Declaration.Test.ps1` in the estate
holds that the declaration builds nothing the contract capability does not.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
