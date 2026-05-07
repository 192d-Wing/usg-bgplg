//! Capability probe — answers `docs/PLAN.md` §13.5 empirically.
//!
//! Connects to a running RustyBGP at `127.0.0.1:50051`, dumps the global
//! config + peers, and tries `ListPath` for every AFI/SAFI we care about
//! so we can see which ones round-trip end-to-end vs. which need the BMP
//! fallback path.
//!
//! Usage:
//!   docker compose -f deploy/lab/docker-compose.yml up --build -d
//!   cargo run -p bgplg-rustybgp --example probe

use std::time::Duration;

use anyhow::{Context, Result};
use bgplg_rustybgp::proto::{
    family::{Afi, Safi},
    go_bgp_service_client::GoBgpServiceClient,
    Family, GetBgpRequest, ListPathRequest, ListPeerRequest, TableType,
};
use tonic::transport::Endpoint;

const ENDPOINT: &str = "http://127.0.0.1:50051";

/// AFI/SAFIs the looking glass plans to support, in roadmap order.
const PROBE_FAMILIES: &[(&str, Afi, Safi)] = &[
    ("ipv4-unicast", Afi::Ip, Safi::Unicast),
    ("ipv6-unicast", Afi::Ip6, Safi::Unicast),
    ("ipv4-labeled-unicast", Afi::Ip, Safi::MplsLabel),
    ("ipv6-labeled-unicast", Afi::Ip6, Safi::MplsLabel),
    ("vpnv4", Afi::Ip, Safi::MplsVpn),
    ("vpnv6", Afi::Ip6, Safi::MplsVpn),
    ("sr-policy-v4", Afi::Ip, Safi::SrPolicy),
    ("sr-policy-v6", Afi::Ip6, Safi::SrPolicy),
    ("bgp-ls", Afi::Ls, Safi::Ls),
];

#[tokio::main]
async fn main() -> Result<()> {
    let channel = Endpoint::from_static(ENDPOINT)
        .connect_timeout(Duration::from_secs(5))
        .connect()
        .await
        .with_context(|| format!("connect {ENDPOINT}"))?;
    let mut client = GoBgpServiceClient::new(channel);

    println!("# RustyBGP capability probe");
    println!("# endpoint: {ENDPOINT}");

    probe_global(&mut client).await;
    probe_peers(&mut client).await?;
    probe_paths(&mut client).await?;
    Ok(())
}

async fn probe_global(client: &mut GoBgpServiceClient<tonic::transport::Channel>) {
    println!("\n## Global");
    match client.get_bgp(GetBgpRequest {}).await {
        Ok(resp) => match resp.into_inner().global {
            Some(g) => println!(
                "  AS {asn}  router-id {rid}  bgp-port :{port}",
                asn = g.asn,
                rid = if g.router_id.is_empty() {
                    "(unset)"
                } else {
                    &g.router_id
                },
                port = g.listen_port
            ),
            None => println!("  (no Global returned)"),
        },
        Err(e) => println!("  ERROR: {e}"),
    }
}

async fn probe_peers(client: &mut GoBgpServiceClient<tonic::transport::Channel>) -> Result<()> {
    println!("\n## Peers");
    let mut stream = client
        .list_peer(ListPeerRequest::default())
        .await?
        .into_inner();
    let mut count = 0usize;
    while let Some(resp) = stream.message().await? {
        count += 1;
        let Some(peer) = resp.peer else { continue };
        let conf = peer.conf.unwrap_or_default();
        let state = peer.state.unwrap_or_default();
        let session = state.session_state();
        let afi_safis: Vec<String> = peer
            .afi_safis
            .iter()
            .filter_map(|a| a.config.as_ref().and_then(|c| c.family.as_ref()))
            .map(family_label)
            .collect();
        println!(
            "  - {addr}  AS {peer_as}  fsm:{session:?}  afi-safis:[{afis}]",
            addr = if conf.neighbor_address.is_empty() {
                "(no addr)"
            } else {
                &conf.neighbor_address
            },
            peer_as = conf.peer_asn,
            afis = afi_safis.join(", "),
        );
    }
    if count == 0 {
        println!(
            "  (no peers — is the lab up? `docker compose -f deploy/lab/docker-compose.yml ps`)"
        );
    }
    Ok(())
}

async fn probe_paths(client: &mut GoBgpServiceClient<tonic::transport::Channel>) -> Result<()> {
    println!("\n## ListPath (TABLE_TYPE_GLOBAL)");
    println!("  {:<22}  {:>6}  result", "family", "paths");
    println!("  {:-<22}  {:->6}  {:-<32}", "", "", "");
    for &(label, afi, safi) in PROBE_FAMILIES {
        let req = ListPathRequest {
            table_type: TableType::Global as i32,
            family: Some(Family {
                afi: afi as i32,
                safi: safi as i32,
            }),
            ..Default::default()
        };
        let outcome = match client.list_path(req).await {
            Ok(resp) => count_stream(resp.into_inner()).await,
            Err(status) => Outcome::Rejected(status.to_string()),
        };
        match outcome {
            Outcome::Ok(n) => println!("  {label:<22}  {n:>6}  ok"),
            Outcome::Stream(n, e) => println!("  {label:<22}  {n:>6}  stream error: {e}"),
            Outcome::Rejected(msg) => {
                println!("  {label:<22}  {dash:>6}  rejected: {msg}", dash = "-")
            }
        }
    }
    Ok(())
}

enum Outcome {
    Ok(usize),
    Stream(usize, String),
    Rejected(String),
}

async fn count_stream<T>(mut s: tonic::Streaming<T>) -> Outcome {
    let mut n = 0usize;
    loop {
        match s.message().await {
            Ok(Some(_)) => n += 1,
            Ok(None) => return Outcome::Ok(n),
            Err(e) => return Outcome::Stream(n, e.to_string()),
        }
    }
}

fn family_label(f: &Family) -> String {
    let afi = match f.afi() {
        Afi::Ip => "v4",
        Afi::Ip6 => "v6",
        Afi::L2vpn => "l2vpn",
        Afi::Ls => "ls",
        Afi::Opaque => "opaque",
        Afi::Unspecified => "?",
    };
    let safi = match f.safi() {
        Safi::Unicast => "unicast",
        Safi::Multicast => "multicast",
        Safi::MplsLabel => "labeled-unicast",
        Safi::MplsVpn => "vpn",
        Safi::SrPolicy => "sr-policy",
        Safi::Ls => "ls",
        Safi::Evpn => "evpn",
        _ => "?",
    };
    format!("{afi}-{safi}")
}
