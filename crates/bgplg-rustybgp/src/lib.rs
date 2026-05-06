//! gRPC client to a RustyBGP companion process.
//!
//! M0 scaffolding: this crate currently only compiles the vendored protobuf
//! schema (see [`VENDOR.md`](../../VENDOR.md)). The actual session, polling,
//! and update-stream logic land in M1.

/// Generated gRPC types for RustyBGP / GoBGP's `api` package.
///
/// All vendored .proto files declare `package api;`, so `tonic` emits a
/// single module here. Lints below target `prost`-generated code we don't
/// own — keep this list narrow.
#[allow(
    clippy::large_enum_variant,
    clippy::doc_markdown,
    clippy::derive_partial_eq_without_eq,
    clippy::enum_variant_names
)]
pub mod proto {
    tonic::include_proto!("api");
}
