# xmip-core-contract-edi-x12

ASC X12 content contract: a sound interchange always, of a bound transaction set and version when a Location names one. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

An interchange is read by one walk, `walk::Walk`, a segment at a time with
the separators its `ISA` declares, a line break among them: the contract
walks it whole into the segments it checks, and the `edi` path technology
([xmip-core-path-edi](https://github.com/IlleNilsson/xmip-core-path-edi))
cuts only the segment it reads and walks the rest uncut, so a malformed
interchange is refused in the same words by both.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
