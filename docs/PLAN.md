# usg-bgplg — Architecture & Implementation Plan

> A multi-VRF, MPLS / SR-MPLS / SRv6-aware BGP looking-glass server.
> Rust backend, modern web UI.

---

## 1. Goals

A looking-glass operator should be able to ask *any participating router* the
questions below — across the **default table** and **any configured VRF** —
and get a normalized answer back through one web UI:

1. `show ip bgp <prefix>` (IPv4, IPv6, VPNv4, VPNv6, labeled-unicast)
2. `show ip route <prefix>` (RIB / FIB)
3. `traceroute <target>` and `ping <target>` (sourced from a chosen VRF)
4. BGP attribute filters: AS-path regex, community / large-community / extcomm
5. SR-MPLS / SRv6 forwarding info:
   - SR-MPLS: SID labels, prefix-SID, Adj-SID, SR-Policy candidate paths
   - SRv6: locator, End / End.X / End.DT4 / End.DT6 / End.DT46 SIDs, uSID
   - Service-side: VPN label / SRv6 Service SID per VRF route

Non-goals (v1): writing config, modifying state on routers, authenticating
users via SSO. Read-only by construction.

## 2. Top-level architecture

```text
                          ┌──────────────────────────┐
                          │        Web UI            │
                          │  (Vite + React + TS)     │
                          └──────────────┬───────────┘
                                         │ HTTPS / JSON
                          ┌──────────────▼───────────┐
                          │     bgplg-api (axum)     │
                          │  REST + SSE/WebSocket    │
                          │  authn, ratelimit, audit │
                          └──────────────┬───────────┘
                                         │
            ┌────────────────────────────┼─────────────────────────┐
            │                            │                         │
   ┌────────▼────────┐         ┌─────────▼────────┐       ┌────────▼────────┐
   │ bgplg-rustybgp  │         │  bgplg-router    │       │   bgplg-probe   │
   │   gRPC client   │         │  vendor adapters │       │  ICMP/UDP probes│
   │   to RustyBGP   │         │  (gNMI, NETCONF, │       │   in-VRF via    │
   │  + BMP receiver │         │   IOS-XR XML,    │       │   network ns /  │
   │   (fallback for │         │   JunOS RPC,     │       │   VPP / FRR)    │
   │   missing SAFIs)│         │   FRR vtysh,     │       └────────┬────────┘
   └────────┬────────┘         │   SR Linux JSON) │                │
            │                  └─────────┬────────┘                │
            │                            │                         │
            └─────────────► RIB store ◄──┘                         │
                          (bgplg-rib)                              │
                                │                                  │
                                ▼                                  ▼
                       ┌────────────────────────────────────────────┐
                       │   normalized result types (serde)          │
                       └────────────────────────────────────────────┘

       ┌───────────────────────────────────┐
       │   RustyBGP (separate process)     │  ← peers with PEs/RRs over BGP
       │   github.com/osrg/rustybgp        │  ← exposes GoBGP-compatible gRPC
       └───────────────────────────────────┘
```

The looking glass uses **RustyBGP as its BGP data plane**. RustyBGP runs as a
companion process — peering with PEs / route-reflectors — and exposes a
GoBGP-compatible gRPC API. `bgplg-rustybgp` is a thin gRPC client that
streams updates and route lookups out of RustyBGP into our normalized RIB.

State can also be learned via:

- **BMP receiver** (in-process, in `bgplg-rustybgp`): used for AFI/SAFIs that
  RustyBGP does not yet parse (BGP-LS, SR-Policy, SRv6 Service — see §4).
  We accept BMP from the device directly and decode the embedded BGP-UPDATE
  ourselves, sidestepping RustyBGP for those families.
- **Adapter mode** (`bgplg-router`): on-demand RPC into the router (gNMI /
  NETCONF / vtysh / vendor JSON-RPC). Slower, leaks queries onto the device,
  but useful as a backstop and for non-BGP info.

For the **MVP** we ship the RustyBGP path with IPv4/IPv6 unicast +
VPNv4/VPNv6 (whatever RustyBGP supports natively), plus a single FRR-vtysh
adapter to prove the adapter path. BMP fallback lights up at M3/M4.

## 3. Crate / module layout (Rust workspace)

```text
crates/
  bgplg-api/         binary  — axum HTTP + WebSocket; depends on the rest
  bgplg-core/        lib     — shared types: Prefix, RouteEntry, Vrf, Sid, etc.
  bgplg-rustybgp/    lib     — gRPC client to RustyBGP + BMP receiver
  bgplg-rib/         lib     — in-mem RIB; per-VRF, per-AFI/SAFI tables
  bgplg-router/      lib     — vendor adapters (trait Router)
  bgplg-probe/       lib     — ping / traceroute, optionally VRF-scoped
  bgplg-config/      lib     — config schema (serde + figment)
  bgplg-auth/        lib     — token / OIDC verification (post-MVP)
xtask/               binary  — release packaging, schema dump, etc.
frontend/            Vite + React + TypeScript app
```

Shared types live in `bgplg-core` so the API crate can reuse them in its
OpenAPI schema and tests.

### `bgplg-core` types (sketch)

```rust
pub enum Afi  { Ipv4, Ipv6 }
pub enum Safi { Unicast, LabeledUnicast, MplsVpn, Srv6L3vpn, SrPolicy }

pub struct Vrf { pub name: String, pub rd: RouteDistinguisher, pub rts: Vec<RouteTarget> }

pub enum LabelStack {
    Mpls(Vec<u32>),                    // SR-MPLS or LDP
    Srv6 { sids: Vec<Ipv6Addr>, behavior: Srv6Behavior },
    None,
}

pub struct RouteEntry {
    pub prefix: IpNet,
    pub vrf:    Option<String>,
    pub afi:    Afi,
    pub safi:   Safi,
    pub nexthop: IpAddr,
    pub as_path: Vec<u32>,
    pub local_pref: Option<u32>,
    pub med:    Option<u32>,
    pub origin: Origin,
    pub communities:       Vec<Community>,
    pub large_communities: Vec<LargeCommunity>,
    pub ext_communities:   Vec<ExtCommunity>,    // includes RTs, Color, etc.
    pub label_stack: LabelStack,
    pub origin_router: RouterId,
    pub seen_at: DateTime<Utc>,
    pub raw: Option<String>,                     // vendor verbatim, for power users
}
```

## 4. BGP via RustyBGP (`bgplg-rustybgp`)

We do **not** implement BGP ourselves. We run [RustyBGP] as a companion
process and drive it over its GoBGP-compatible gRPC API.

[RustyBGP]: https://github.com/osrg/rustybgp

> **Pinned to upstream commit
> [`9e2a907a`](https://github.com/osrg/rustybgp/commit/9e2a907a3c1ec82c2d648cbfb56a536d98ad2952)
> (master @ 2026-04-25).** RustyBGP publishes no git tags, so we pin by
> commit SHA. The protobuf schema is vendored under
> [`crates/bgplg-rustybgp/proto/`](../crates/bgplg-rustybgp/proto/) — see
> [`VENDOR.md`](../crates/bgplg-rustybgp/VENDOR.md) for the SHA-256
> fingerprints and refresh procedure.

### Why RustyBGP

- Same author as GoBGP, written in async Rust — fits our toolchain.
- gRPC API is identical to GoBGP's, so the protobufs are stable and
  well-documented (`proto/gobgp.proto` etc.).
- Lets `usg-bgplg` stay focused on the *looking-glass* problem (RIB
  modeling, VRF/SID semantics, UI) instead of BGP FSM + parser maintenance.

### How we use it

`bgplg-rustybgp` is a small library crate that:

1. Spawns a `tonic` client against RustyBGP's gRPC endpoint.
2. On startup, calls `ListPeer` / `ListPath` to seed the RIB.
3. Subscribes to `WatchEvent` (UPDATE stream) and translates each path
   into a `bgplg_core::RouteEntry`, posting it into `bgplg-rib`.
4. Issues `AddPeer` calls based on `[[routers]]` config so RustyBGP comes
   up with the right neighbors when we start.

We never use RustyBGP to *originate* routes — looking-glass is read-only.
RustyBGP runs with an empty Loc-RIB policy and we egress-filter everything
(or just don't peer outbound).

### Capability gaps and the BMP fallback

RustyBGP's AFI/SAFI coverage is narrower than GoBGP's. The exact list shifts
with upstream releases, so we **runtime-detect** what RustyBGP advertises and
gracefully fall back to BMP for the rest. As of writing the picture is:

| AFI/SAFI                 | Source           | Notes                                                   |
| ------------------------ | ---------------- | ------------------------------------------------------- |
| `1/1`   IPv4 unicast     | RustyBGP         | first-class                                             |
| `2/1`   IPv6 unicast     | RustyBGP         | first-class                                             |
| `1/4`   IPv4 lbl-unicast | RustyBGP / BMP   | verify per upstream release                             |
| `2/4`   IPv6 lbl-unicast | RustyBGP / BMP   | verify per upstream release                             |
| `1/128` VPNv4 unicast    | RustyBGP         | confirm path attribute decode coverage                  |
| `2/128` VPNv6 unicast    | RustyBGP         | confirm path attribute decode coverage                  |
| `1/73`  SR-Policy v4     | **BMP fallback** | parse SR-Policy attributes ourselves                    |
| `2/73`  SR-Policy v6     | **BMP fallback** |                                                         |
| `16388/71` BGP-LS        | **BMP fallback** | TLV decode in `bgplg-rustybgp::bmp`                     |
| SRv6 Service (RFC 9252)  | RustyBGP / BMP   | depends on upstream prefix-SID + Srv6 attribute support |
| `1/129` MCAST-VPN        | deferred         |                                                         |

### BMP receiver

Independent of RustyBGP, `bgplg-rustybgp::bmp` listens on a TCP port for BMP
(RFC 7854) sessions from PEs/RRs. Each BMP PEER\_UP / ROUTE\_MONITORING
message embeds a real BGP UPDATE, which we parse with our own MP-BGP decoder
(this is the part of the project we *do* own, scoped to the AFI/SAFIs above).
This is how we get BGP-LS, SR-Policy, SRv6 Service even if RustyBGP can't
parse them.

We accept-only — we never send updates back to peers (RustyBGP's policy
engine handles ingress filtering, BMP is by definition receive-only).

## 5. RIB (`bgplg-rib`)

Per-VRF tables keyed by `(afi, safi, vrf)` →  IP-trie of `RouteEntry`. Backed
by `ip_network_table::IpNetworkTable` or a `patricia_tree` for IPv4/IPv6, with
a flat `HashMap` for `MplsVpn` keyed by `(rd, prefix)`.

Index structures we will need for the UI:

- AS-path regex search → secondary index over a compacted AS-path table.
- Community / large-community lookup → multimap.
- Per-router origin index (for "what does router X currently see").

All operations are read-only after BGP UPDATE processing, so `Arc<RwLock<…>>`
plus copy-on-write snapshotting per query is fine for MVP.

## 6. VRF + label/SID model

A VRF on a router is identified by `(rd, [import-rts], [export-rts])`; the
**name** is just human label config. The same `(rd, prefix)` can appear with
many different label/SID values across PEs.

For each `RouteEntry` in a VPN AFI we keep:

- The **VPN label** or **SRv6 Service SID** (advertised in the BGP attribute).
- The **PE loopback** as next-hop.
- The **transport** label/SID stack (looked up by joining onto the IPv4/IPv6
  unicast or BGP-LS SR view of that PE's loopback).

The UI surfaces both the *advertised* service label/SID and the *resolved*
end-to-end stack. That's the differentiator vs. a vanilla looking glass.

## 7. SR-MPLS / SRv6 specifics

- **SR-MPLS:**
  - Parse Prefix-SID (subtype 1) and SRGB attribute from BGP-LS.
  - Show absolute label = SRGB.start + index for prefix-SIDs.
  - SR-Policy SAFI: render endpoint, color, candidate-paths, segment-list.
- **SRv6:**
  - SRv6 L3 Service TLV (RFC 9252) carries the locator + function bits.
  - Decode behavior code (End, End.X, End.DT4, End.DT6, End.DT46, End.DT2U,
    End.B6.Encaps, …) per RFC 8986 + draft-ietf-spring-srv6-srh-compression
    (uSIDs / G-SID).
  - Display SID in `LOC:FUNC:ARG` form alongside the raw 128-bit address.

## 8. API surface (`bgplg-api`)

REST + SSE for streaming, WebSocket for live BGP UPDATE feeds (bonus).

```text
GET  /api/v1/routers                              -> [RouterSummary]
GET  /api/v1/vrfs?router=<id>                     -> [Vrf]
GET  /api/v1/routes?prefix=&vrf=&afi=&safi=&...   -> [RouteEntry]
GET  /api/v1/routes/best?prefix=&vrf=             -> RouteEntry
POST /api/v1/query                                -> normalized free-form
   { "kind": "show_bgp" | "show_route" | "ping" | "traceroute",
     "target": "...", "vrf": "...", "router": "..." }
GET  /api/v1/topology                             -> BGP-LS graph (IGP-ish)
GET  /api/v1/sr/policies?router=<id>              -> [SrPolicy]
GET  /api/v1/srv6/locators?router=<id>            -> [Srv6Locator]
GET  /api/v1/health
GET  /api/v1/openapi.json
```

All responses normalized to `bgplg-core` types. Probe endpoints stream output
as SSE so traceroute hops appear as they arrive.

### AuthN / rate-limit / audit (MVP scope)

- API tokens (random 256-bit, hashed at rest) — sufficient for v1.
- Per-token + per-IP token-bucket rate limit on probe endpoints.
- Append-only audit log: `(time, principal, query, router, vrf, dur, ok)`.

## 9. Frontend

- **Stack:** Vite + React + TypeScript + TanStack Query + Tailwind +
  shadcn/ui. Generate the API client from the backend's OpenAPI document
  (`openapi-typescript-codegen` or `orval`).
- **Look:** dark-first, dense tables, monospace for prefix/AS-path/labels,
  copy-on-click for any prefix or label, keyboard-driven (Cmd-K palette to
  jump router/VRF/prefix).
- **Pages:**
  - `/` — query box (autodetect: prefix, AS#, community, AS-path regex)
  - `/router/:id` — per-router VRFs, neighbors, RIB stats
  - `/router/:id/vrf/:name` — RIB browser, filter chips
  - `/route/:prefix` — combined view across routers/VRFs with label/SID
    resolution, AS-path tree, raw vendor output expander
  - `/sr` — SR-MPLS/SRv6 explorer (SR-Policies, locators, BGP-LS topo)
  - `/probe` — ping / traceroute with VRF picker, live SSE stream
- **Topology view:** BGP-LS-driven graph using `react-flow` (lazy-loaded).

## 10. Configuration

A single `config.toml` (figment for layered env-vars / file). Sketch:

```toml
[server]
listen = "0.0.0.0:8443"
tls_cert = "./certs/server.pem"
tls_key  = "./certs/server.key"

# RustyBGP runs as a companion process (e.g. another container in the same pod).
# We do not configure BGP listen/router-id here — that lives in RustyBGP's own
# config. We just tell ourselves how to reach it.
[rustybgp]
grpc_endpoint = "http://127.0.0.1:50051"

# Optional in-process BMP receiver for AFI/SAFIs RustyBGP doesn't decode
# (BGP-LS, SR-Policy, SRv6 Service). Routers send BMP straight to us.
[bmp]
listen = "0.0.0.0:11019"

[[routers]]
id          = "pe1"
peer_ip     = "10.0.0.11"
remote_as   = 65000
source      = "rustybgp"   # RustyBGP will be told to peer with this neighbor
afi_safis   = ["ipv4-unicast", "ipv6-unicast", "vpnv4", "vpnv6"]

[[routers]]
id          = "pe2"
peer_ip     = "10.0.0.12"
source      = "bmp"        # this router streams BMP into us instead of BGP
afi_safis   = ["bgp-ls", "sr-policy-v4", "sr-policy-v6"]

[[routers]]
id        = "pe3"
source    = "adapter"      # on-demand RPC, no peering
adapter   = { kind = "frr-vtysh", host = "10.0.0.13", user = "lg" }

[[vrf_aliases]]   # human-friendly names for RDs we'll see
rd   = "65000:100"
name = "RED"
```

## 11. Deployment

- The looking glass ships as **two containers** that always run together:
  1. `bgplg` — our axum API + embedded frontend (single static binary,
     `rust-embed`).
  2. `rustybgp` — the BGP data plane.
- Networking: `rustybgp` binds `:179` for BGP and `:50051` for gRPC; `bgplg`
  binds `:8443` for HTTPS and (optionally) `:11019` for BMP. In Kubernetes
  they live in one Pod so gRPC stays on `localhost`.
- Provided artifacts:
  - `Dockerfile` for `bgplg` (multi-stage: rust → distroless).
  - `docker-compose.yml` for local dev: `bgplg` + `rustybgp` + a small FRR
    lab to peer against.
  - `Containerfile` mirror for podman.
  - Helm chart in `deploy/helm/` (post-MVP) that ships both containers in a
    single Pod with a shared `emptyDir` for sockets/certs.

## 12. Roadmap / milestones

| Milestone                   | Scope                                                                                                |
| --------------------------- | ---------------------------------------------------------------------------------------------------- |
| **M0 – Skeleton (done)**    | Repo scaffolding, conventional commits, build green, hello-world API + UI.                           |
| **M1 – Speaker MVP**        | RustyBGP companion, gRPC plumbing, IPv4/IPv6 unicast, in-mem RIB, REST `/routes` endpoint, basic UI. |
| **M2 – VPNs**               | VPNv4/VPNv6 via RustyBGP, per-VRF RIB, RD/RT model, VRF picker in UI.                                |
| **M3 – Labels & SIDs**      | Labeled-unicast, SR-MPLS Prefix-SID, SRv6 Service SID decode + display (BMP path lights up).         |
| **M4 – BGP-LS + SR-Policy** | Topology view, SR-Policy SAFI, segment-list rendering (BMP-fed).                                     |
| **M5 – Probes**             | In-VRF ping / traceroute via FRR adapter, SSE streaming in UI.                                       |
| **M6 – Hardening**          | Auth tokens, rate limit, audit log, Helm chart, docs site.                                           |

## 13. Open questions

1. Auth: tokens-only for v1, or wire up OIDC from day one?
2. Probe execution: shell into the router via the adapter, or run our own
   probes from a network namespace that imports the VRF? The latter is more
   honest but assumes Linux + FRR/VPP at the looking-glass host.
3. Persistence: do we keep route history (time-series) or strictly snapshot
   the current view? History is hugely useful but expensive — flag for M7.
4. License — MIT, Apache-2.0, or dual?
5. ~~**RustyBGP capability matrix:** before M2 starts, run the AFI/SAFI table
   in §4 against the pinned RustyBGP version we plan to ship.~~ **Resolved
   (partly):** pinned to upstream commit `9e2a907a` (master @ 2026-04-25)
   since RustyBGP does not cut tags. The empirical AFI/SAFI verification
   (which families actually round-trip via gRPC vs. need the BMP fallback)
   still happens during M1 once we have a peering harness running.
6. ~~**gRPC protobuf source:** vendor the `gobgp.proto` from RustyBGP's repo,
   or take a pre-built crate?~~ **Resolved:** vendored, see
   [`crates/bgplg-rustybgp/proto/`](../crates/bgplg-rustybgp/proto/) and
   [`VENDOR.md`](../crates/bgplg-rustybgp/VENDOR.md). Six files
   (`gobgp.proto` + 5 imports), MIT-licensed in their headers (inherited
   from GoBGP); the upstream Apache-2.0 LICENSE is preserved alongside.
