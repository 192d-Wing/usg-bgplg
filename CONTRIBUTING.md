# Contributing

## Commit messages

This repository uses **[Conventional Commits](https://www.conventionalcommits.org/)**.

Format:

```
<type>(<optional-scope>)!?: <subject>

<optional body>

<optional footer(s)>
```

Allowed `type` values:

| type     | meaning                                                |
| -------- | ------------------------------------------------------ |
| feat     | A new user-visible feature                             |
| fix      | A bug fix                                              |
| docs     | Documentation only                                     |
| style    | Formatting, whitespace, semicolons, no logic change    |
| refactor | Code change that is neither a fix nor a feature        |
| perf     | Performance improvement                                |
| test     | Adding or correcting tests                             |
| build    | Build system, dependencies, packaging                  |
| ci       | CI configuration / pipelines                           |
| chore    | Routine maintenance not covered above                  |
| revert   | Reverting a prior commit                               |

Add `!` after the type/scope (e.g. `feat(api)!: ...`) to flag a breaking change,
or include a `BREAKING CHANGE:` footer.

### Local enforcement

Hooks live in `.githooks/`. They are wired up by:

```bash
git config core.hooksPath .githooks
```

The `pnpm install` / `npm install` step in the frontend will also configure this
automatically (see `package.json` `prepare` script once the frontend lands).

### CI enforcement

`.github/workflows/commitlint.yml` runs `commitlint` over every PR and push.

## Suggested scopes

- `api`        — HTTP / gRPC API surface
- `bgp`        — BGP session / RIB code
- `vrf`        — VRF / VPNv4 / VPNv6 handling
- `mpls`       — MPLS / SR-MPLS label handling
- `srv6`       — SRv6 SID / locator handling
- `ui`         — frontend
- `infra`      — Docker / deployment / k8s
- `docs`       — documentation
