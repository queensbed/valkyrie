# Valkyrie Cup

Valkyrie Cup is a mobile-first football tournament MVP where players create AI football agents, train them with plain-language prompts, enter them into live competitions, and resolve the next round into updated standings.

## Why Rust

Rust is used for the backend domain core because tournament state, ratings, match events, and prize accounting benefit from predictable performance and memory safety. The current MVP intentionally uses only the Rust standard library so it is easy to audit and run in constrained environments. A production version can split the match simulator into a Rust service and expose a versioned API to native Android/iOS clients.

The requested historical criteria describe a mature source repository, not a property an empty repository can already possess. This repository currently starts at zero:

| Criterion | Current evidence | Status |
| --- | --- | --- |
| 100 merged PRs | Empty repository, no PR history | Not met |
| 100,000–10,000,000 LOC | MVP is intentionally small | Not met |
| 10+ contributors | No commits or contributors | Not met |
| 1,000+ commits | No commits | Not met |
| >25% test coverage | Two focused unit tests; no coverage baseline yet | Not met |
| Primarily human-written / pre-AI | No prior history to verify | Not verifiable |

These are acceptance gates for selecting an existing codebase or later project maturity targets; they cannot be truthfully manufactured in the initial commit. The app is structured so future work can add a simulator, persistence, matchmaking, authentication, payments, observability, and native clients without replacing the agent/tournament API boundary.

## Run

```bash
cargo run
```

Open http://localhost:8080 on a phone-sized browser viewport. Run tests with:

```bash
cargo test
```
