# Security Policy

## Reporting a vulnerability

Please report vulnerabilities privately via GitHub Security Advisories on this repository
("Report a vulnerability"). Do not open public issues for security reports. You should receive
an acknowledgement within a week.

## Scope

- The `membench` / `membench-server` / `membench-leaderboard` binaries and the dashboard SPA.
- The dashboard server is a **local development tool**: it serves the local run registry
  without authentication and is not hardened for exposure to untrusted networks. Do not bind
  it to a public interface.
- Static leaderboard deploys contain only the committed `membench.leaderboard.v1` snapshot —
  no credentials and no server-side code.

## Secrets

Provider credentials live only in ignored local env files (`.env.test.local`). Tracked records
and templates must never contain real keys, raw provider payloads, or secrets; report it as a
vulnerability if you find any.
