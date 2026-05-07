# M1 lab

A throwaway, all-in-Docker BGP fabric for developing the looking-glass
backend. Brings up RustyBGP (at the SHA pinned in
[`crates/bgplg-rustybgp/VENDOR.md`](../../crates/bgplg-rustybgp/VENDOR.md))
acting as a route-reflector inside `AS 65000`, with two FRR PEs as RR
clients announcing IPv4 + IPv6 unicast prefixes.

## Topology

```text
   ┌─────────────┐                ┌──────────────────┐                ┌─────────────┐
   │  frr-pe1    │  iBGP / v4+v6  │     rustybgp     │  iBGP / v4+v6  │  frr-pe2    │
   │  AS 65000   │ ◄────────────► │  AS 65000 (RR)   │ ◄────────────► │  AS 65000   │
   │  10.0.0.2   │   172.30.10.x  │   10.0.0.1       │                │  10.0.0.3   │
   └─────────────┘                └──────────────────┘                └─────────────┘
                                          ▲
                                          │ gRPC :50051 (host-mapped)
                                          ▼
                                   ┌──────────────────┐
                                   │  bgplg (host)    │
                                   │  cargo run …     │
                                   └──────────────────┘
```

### Announcements

| PE  | IPv4              | IPv6                |
| --- | ----------------- | ------------------- |
| pe1 | `192.0.2.0/24`    | `2001:db8:cafe::/48`|
| pe2 | `192.0.2.0/24`*   | —                   |
| pe2 | `198.51.100.0/24` | `2001:db8:beef::/48`|

`*` pe2 re-originates `192.0.2.0/24` with community `65000:200` so the
looking glass has a multi-path case to render.

## Run

```sh
# bring up RustyBGP + the two FRR PEs
docker compose -f deploy/lab/docker-compose.yml up --build -d

# once healthy, hit the gRPC API directly
cargo run -p bgplg-rustybgp --example probe

# tail RustyBGP
docker logs -f bgplg-lab-rustybgp

# inspect FRR's view
docker exec -it bgplg-lab-pe1 vtysh -c 'show ip bgp summary'
docker exec -it bgplg-lab-pe2 vtysh -c 'show ip bgp ipv6 unicast'

# tear down
docker compose -f deploy/lab/docker-compose.yml down
```

## What the probe checks

[`crates/bgplg-rustybgp/examples/probe.rs`](../../crates/bgplg-rustybgp/examples/probe.rs)
talks to RustyBGP at `127.0.0.1:50051` and dumps:

- BGP global config (AS, router-id) — confirms the daemon is up.
- `ListPeer` — both FRR sessions and their negotiated AFI/SAFIs.
- `ListPath` for every AFI/SAFI we plan to support, recording which ones
  the live RustyBGP actually returns paths for.

Output of that run is the empirical capability matrix referenced in
[`docs/PLAN.md`](../../docs/PLAN.md) §13.5. Re-run after every RustyBGP SHA
bump.

## Notes

- The daemon's gRPC server hardcodes `0.0.0.0:50051` (see upstream
  `daemon/src/event.rs`). We host-map only on `127.0.0.1` so the lab is
  not exposed to the network.
- `privileged: true` on the FRR containers is required by FRR's docker
  image, not by us.
- The lab is **not** a deployment artifact. The production compose / Helm
  chart land in M6.
