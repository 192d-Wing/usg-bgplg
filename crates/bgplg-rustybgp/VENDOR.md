# Vendored protos — RustyBGP

The files in [`proto/`](proto/) are vendored verbatim from the upstream
RustyBGP project so the looking glass talks to RustyBGP's gRPC API with a
schema we control and pin.

## Pin

| Field             | Value                                                                  |
| ----------------- | ---------------------------------------------------------------------- |
| Upstream repo     | <https://github.com/osrg/rustybgp>                                     |
| Path              | `api/proto/`                                                           |
| Pinned commit     | `9e2a907a3c1ec82c2d648cbfb56a536d98ad2952`                             |
| Pin reason        | Upstream publishes no git tags; pinning master HEAD as of 2026-04-25.  |
| Vendored on       | 2026-05-06                                                             |

> RustyBGP does not cut releases. If/when it does, replace the SHA above
> with the tag and prefer that going forward.

## SHA-256 fingerprints (verify before bumping)

```text
attribute.proto    d0a942f22f2e204103d08623127c7ac6195e3bcc1aec78c5a6e759efb98e5ba0
capability.proto   e70485b06564b87795a0a005677711e099ece9d11443107692fa9cab0760bc6d
common.proto       87cb2ebc674d55654b2668f2daec23a0f6f72fa48c9248a5b99f51ac5e52bc99
extcom.proto       6554368e6f70de73dc8c580b7c7a6eed603947f643890a8ee45365f3fd6ec892
gobgp.proto        bb68cb2923cc4fccf70db9284d4fd58a93b5f958e9166244c48112a6fc9334a9
nlri.proto         ca022cbcb2668a4a3434faf2a8eed94f84ed8afb19b1fbfd3e53de8280f4bdcc
LICENSE-RUSTYBGP   c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4
```

## Refreshing the vendor

```sh
SHA=<new-rustybgp-sha-or-tag>
for f in gobgp.proto attribute.proto capability.proto common.proto extcom.proto nlri.proto; do
  gh api -H "Accept: application/vnd.github.raw" \
    "repos/osrg/rustybgp/contents/api/proto/${f}?ref=${SHA}" > "proto/${f}"
done
gh api -H "Accept: application/vnd.github.raw" \
  "repos/osrg/rustybgp/contents/LICENSE?ref=${SHA}" > LICENSE-RUSTYBGP
shasum -a 256 proto/*.proto LICENSE-RUSTYBGP
```

Update the pinned commit, the vendored-on date, and the SHA-256 fingerprints
above. Bump the workspace `Cargo.lock` and run `cargo build -p bgplg-rustybgp`
before committing.

## Licensing

- The protobuf files (`proto/*.proto`) carry an MIT-style permission notice
  in their per-file headers (inherited from upstream GoBGP); preserve those
  headers in any redistribution.
- The RustyBGP project as a whole is Apache-2.0; the upstream `LICENSE` file
  is preserved here as `LICENSE-RUSTYBGP` for attribution. RustyBGP does not
  ship a `NOTICE` file.

We do not vendor any RustyBGP Rust source — only the proto schema. Generated
gRPC client code from these protos is built locally by `build.rs` and is the
property of this project's user under the same Apache-2.0 terms as the rest
of `usg-bgplg`.
