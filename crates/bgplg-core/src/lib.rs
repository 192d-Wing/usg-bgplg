//! Shared, vendor-neutral types for the looking-glass backend and any
//! out-of-process consumer. See `docs/PLAN.md` §3 for the full data model.

use std::net::{IpAddr, Ipv6Addr};

use chrono::{DateTime, Utc};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Afi {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Safi {
    Unicast,
    LabeledUnicast,
    MplsVpn,
    Srv6L3vpn,
    SrPolicy,
    BgpLs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Igp,
    Egp,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RouteDistinguisher(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RouteTarget(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrf {
    pub name: String,
    pub rd: RouteDistinguisher,
    pub import_rts: Vec<RouteTarget>,
    pub export_rts: Vec<RouteTarget>,
}

/// SRv6 endpoint behaviors (RFC 8986 + uSID drafts). Stored as a code so the
/// backend doesn't have to be rebuilt to learn a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Srv6Behavior(pub u16);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LabelStack {
    None,
    Mpls {
        labels: Vec<u32>,
    },
    Srv6 {
        sids: Vec<Ipv6Addr>,
        behavior: Srv6Behavior,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeCommunity(pub u32, pub u32, pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtCommunity(pub [u8; 8]);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub prefix: IpNet,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vrf: Option<String>,
    pub afi: Afi,
    pub safi: Safi,
    pub nexthop: IpAddr,
    pub as_path: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_pref: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub med: Option<u32>,
    pub origin: Origin,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub communities: Vec<Community>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub large_communities: Vec<LargeCommunity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ext_communities: Vec<ExtCommunity>,
    pub label_stack: LabelStack,
    pub origin_router: RouterId,
    pub seen_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}
