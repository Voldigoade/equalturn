# Enforcement test safety

This is a permanent development and CI safety rule for all future Equalturn enforcement work. Milestone 1.5 implements no endpoint enforcement; this policy does not alter or test the active host.

## Never enforce on the active development host

Never run a test that can block, intercept, modify, or reconfigure the workstation currently being used for development. This includes firewall/WFP/nftables rules, DNS or routes, system proxies or VPNs, services, application-control policy, and any rule that could interrupt GitHub, package registries, relay access, or the development session. Do not assume the host can be restored after losing connectivity.

## Required isolation

- **Linux:** use a disposable VM for full-system behavior. For firewall/network tests, prefer a dedicated network namespace and apply rules only inside that namespace. A container is acceptable only when the behavior being tested is accurately represented in that container.
- **Windows:** use Windows Sandbox only when it supports the specific primitive under test. Use a disposable Hyper-V/other VM or a dedicated test machine for WFP, WDAC, AppLocker, or service behavior that Sandbox does not faithfully represent.
- **CI:** never intentionally block or reconfigure the runner's own network/control plane. Run dangerous integration scenarios only in a separately isolated test target with explicit setup and teardown.

If suitable isolation is unavailable, skip the dangerous test and record **NOT EXECUTED — SAFE ISOLATION UNAVAILABLE**. Never silently fall back to the host.

## Future test-target requirement

Future enforcement backends must take an explicit test target, such as a disposable VM or named network namespace, and refuse to run an automated enforcement test against the initial/default host namespace. Development builds should fail closed when no isolated target is selected. Any development-only override must be deliberate and must not be presented as a protection for competition participants.

These mechanisms protect developers and CI from accidental self-lockout. They are not production anti-bypass controls and must never be marketed as protection against a participant who controls their own machine.
