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

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
