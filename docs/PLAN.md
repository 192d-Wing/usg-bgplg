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

```
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
   │  bgplg-bgp      │         │  bgplg-router    │       │   bgplg-probe   │
   │  passive BGP    │         │  vendor adapters │       │  ICMP/UDP probes│
   │  speaker        │         │  (gNMI, NETCONF, │       │  in-VRF via    │
   │  (RFC 4271 +    │         │   IOS-XR XML,    │       │   network ns / │
   │   AFI/SAFIs)    │         │   JunOS RPC,     │       │   VPP / FRR)   │
   └────────┬────────┘         │   SR Linux JSON, │       └────────┬────────┘
            │                  │   FRR vtysh)     │                │
            │                  └─────────┬────────┘                │
            │                            │                         │
            └─────────────► RIB store ◄──┘                         │
                          (bgplg-rib)                              │
                                │                                  │
                                ▼                                  ▼
                       ┌────────────────────────────────────────────┐
                       │   normalized result types (serde)          │
                       └────────────────────────────────────────────┘
```

Two ways the looking glass can learn routing state — both supported, picked
per-router in config:

- **Speaker mode** (`bgplg-bgp`): we run a passive BGP speaker, peer the
  router into us as a route-reflector client (or just a regular peer with
  `route-reflector-client` from their side, or BMP). We hold the Adj-RIB-In
  ourselves. Best fidelity, no per-query latency.
- **Adapter mode** (`bgplg-router`): we shell out / RPC to the router on
  demand (gNMI subscribe / NETCONF get / vtysh / vendor JSON-RPC). Easier to
  deploy, slower, leaks operational queries onto the device.

For the **MVP** we ship Speaker mode + a single FRR-vtysh adapter (trivial)
to prove both paths.

## 3. Crate / module layout (Rust workspace)

```
crates/
  bgplg-api/         binary  — axum HTTP + WebSocket; depends on the rest
  bgplg-core/        lib     — shared types: Prefix, RouteEntry, Vrf, Sid, etc.
  bgplg-bgp/         lib     — passive BGP speaker (BMP optional)
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

## 4. BGP speaker (`bgplg-bgp`)

We don't implement BGP from scratch in v1. Choices, in priority order:

1. **`holo-bgp`** (the Holo project) — pure-Rust, async, AFI/SAFI extensible.
2. **`rotonda-fsm` + `routecore`** (NLnet Labs) — Rust BGP toolkit, very
   active, BMP-friendly.
3. Bind to **GoBGP** via gRPC if pure-Rust isn't fast enough to ship — easy
   escape hatch.

We will likely start with `routecore`/`rotonda-fsm` — message parsing is the
hard part and they already cover MP-BGP for VPNv4/v6, labeled-unicast, BGP-LS,
SR-Policy SAFI, and SRv6 service SIDs.

**Required AFI/SAFIs to advertise capabilities for:**

- `1/1`   IPv4 unicast
- `2/1`   IPv6 unicast
- `1/4`   IPv4 labeled-unicast
- `2/4`   IPv6 labeled-unicast
- `1/128` VPNv4 unicast
- `2/128` VPNv6 unicast
- `1/73`  SR Policy IPv4         (RFC 9256 SAFI)
- `2/73`  SR Policy IPv6
- `16388/71` BGP-LS              (for topology / SR-MPLS / SRv6 TLVs)
- `1/129` MCAST-VPN              (deferred)

We accept-only (egress-filter everything). Routes go straight into `bgplg-rib`.

### BMP

Optional but recommended: also accept BMP (RFC 7854) so we can passively
mirror Adj-RIB-In/Out from devices that support it without full BGP peering.

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

```
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

[bgp]
router_id   = "10.0.0.1"
local_as    = 65000
listen      = "0.0.0.0:179"
hold_time   = 90
afi_safis   = ["ipv4-unicast", "ipv6-unicast", "ipv4-labeled-unicast",
               "ipv6-labeled-unicast", "vpnv4", "vpnv6",
               "sr-policy-v4", "sr-policy-v6", "bgp-ls"]

[[routers]]
id        = "pe1"
peer_ip   = "10.0.0.11"
remote_as = 65000
mode      = "speaker"   # or "adapter"

[[routers]]
id        = "pe2"
mode      = "adapter"
adapter   = { kind = "frr-vtysh", host = "10.0.0.12", user = "lg" }

[[vrf_aliases]]   # human-friendly names for RDs we'll see
rd   = "65000:100"
name = "RED"
```

## 11. Deployment

- Single static binary for the API + speaker.
- Frontend served as embedded static assets (`rust-embed`) so the deploy is
  one container.
- Provided artifacts:
  - `Dockerfile` (multi-stage: rust → distroless)
  - `docker-compose.yml` for local dev (looking-glass + a small FRR lab)
  - `Containerfile` mirror for podman
  - Helm chart in `deploy/helm/` (post-MVP)

## 12. Roadmap / milestones

| Milestone | Scope |
|----|----|
| **M0 – Skeleton (this PR)**       | Repo scaffolding, conventional commits, build green, hello-world API + UI. |
| **M1 – Speaker MVP**              | Passive BGP, IPv4/IPv6 unicast only, in-mem RIB, REST `/routes` endpoint, basic UI. |
| **M2 – VPNs**                     | VPNv4/VPNv6, per-VRF RIB, RD/RT model, VRF picker in UI. |
| **M3 – Labels & SIDs**            | Labeled-unicast, SR-MPLS Prefix-SID, SRv6 Service SID decode + display. |
| **M4 – BGP-LS + SR-Policy**       | Topology view, SR-Policy SAFI, segment-list rendering. |
| **M5 – Probes**                   | In-VRF ping / traceroute via FRR adapter, SSE streaming in UI. |
| **M6 – Hardening**                | Auth tokens, rate limit, audit log, Helm chart, docs site. |

## 13. Open questions

1. Auth: tokens-only for v1, or wire up OIDC from day one?
2. Probe execution: shell into the router via the adapter, or run our own
   probes from a network namespace that imports the VRF? The latter is more
   honest but assumes Linux + FRR/VPP at the looking-glass host.
3. Persistence: do we keep route history (time-series) or strictly snapshot
   the current view? History is hugely useful but expensive — flag for M7.
4. License — MIT, Apache-2.0, or dual?
