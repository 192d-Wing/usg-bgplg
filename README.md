# usg-bgplg

A BGP looking glass server with first-class support for VRFs and label-switched
underlays — MPLS, SR-MPLS, and SRv6 — in addition to plain IPv4 / IPv6 unicast.

- **Backend:** Rust
- **Frontend:** modern web UI (TypeScript)
- **License:** TBD
- **Status:** early scaffolding — see [docs/PLAN.md](docs/PLAN.md)

> Looking-glass servers are read-only views into a router's view of the global
> routing table. This project's twist: it is multi-VRF aware and understands
> labeled / segment-routed forwarding paths, not just the IPv4/IPv6 unicast
> AFI/SAFI.

## Features (planned)

- Read-only views of:
  - IPv4 / IPv6 unicast
  - VPNv4 / VPNv6 (per-VRF)
  - Labeled-unicast (RFC 8277)
  - SR-MPLS policies (BGP-LS, SR-Policy SAFI)
  - SRv6 SIDs / locators (BGP-LS, SRv6 L3 Service)
- Multi-router federation (query several speakers, merge results)
- Common looking-glass operations: `show route`, `traceroute`, `ping`, BGP
  `show ip bgp <prefix>`, AS-path / community filters
- Auth + rate limiting suitable for a public deployment
- Audit log of every query

See [docs/PLAN.md](docs/PLAN.md) for the implementation plan.

## Development

```bash
# Wire up the conventional-commits hook
git config core.hooksPath .githooks
```

Toolchain:

- Rust (stable)
- Node 20+ / pnpm
