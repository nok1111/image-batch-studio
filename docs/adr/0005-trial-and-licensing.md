# ADR 0005 — 3-day trial and licensing (design; implemented in Phase 5)

Status: proposed

- Trial: 3 days of full functionality from first launch, then processing requires a license (Lemon Squeezy License API: activate / validate / deactivate; no secret key is embedded — the License API only needs the customer's key).
- Anti-reset measures (reinstalling must not grant a new trial):
  - Trial start stored in several independent places outside the install dir (app-data SQLite, a second file under the local app-data root, and the HKCU registry on Windows), each record HMAC-bound to a machine fingerprint (e.g. Windows `MachineGuid`). The earliest valid record wins; deleting some of them does not reset the trial; a missing-everywhere state after a previous detection is treated as expired.
  - Clock-rollback detection: store a monotonic "last seen" timestamp; a clock earlier than it by more than a tolerance ends the trial.
  - Uninstaller does not remove trial markers.
- Limitation (documented): any purely client-side trial can be bypassed by a determined user (e.g. a VM or full registry wipe). Stronger guarantees need a server-side trial registry; out of scope unless decided otherwise.
- Development mode uses a mock `LicenseProvider`; release builds cannot select it.
