// Compile the vendored RustyBGP / GoBGP gRPC schema.
//
// The schema in proto/ is pinned to a specific upstream commit — see
// VENDOR.md. We mirror the import set used by RustyBGP's own api/build.rs:
// only gobgp.proto + the two top-level dependents need to be passed
// explicitly, the rest are pulled in by `import` directives.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto");

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(
            &[
                "proto/gobgp.proto",
                "proto/attribute.proto",
                "proto/capability.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
