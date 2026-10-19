# Prior art and project wedge

**Research date:** 19 October 2026. This is a targeted public-source review, not proof that no similar project exists.

## Strong adjacent systems

| Product/category | What it already solves well | Gap relative to this project |
| --- | --- | --- |
| [Safe Exam Browser](https://safeexambrowser.org/about_overview_en.html) | Controlled exam browser, kiosk-oriented test environment, and platform-specific configuration for exams. | Exam browser/session model, not a privacy-minimal background agent for normal CTF/programming workflows across Windows and Linux. |
| [Respondus LockDown Browser](https://web.respondus.com/ldb-vs-extensions/) and Respondus Monitor | Closes/blocks selected applications and restricts exam browser behavior; optional webcam-based monitoring. | Exam/classroom workflow and institution integration; webcam monitoring is outside this project’s privacy goal. |
| [CTFd](https://ctfd.io/v1/) | Mature CTF challenge delivery, scoreboard, teams, and competition workflow. | Does not provide endpoint-side AI access controls, local protection state, or signed integrity reports. |
| [CTFd AI Cheater Detector](https://github.com/7h30th3r0n3/ctfd-ai-cheater-detector) | An example of server-side CTF answer-pattern analysis for suspicious submissions. | Submission heuristics do not control endpoint access or provide a mutually visible local protection/liveness timeline. |
| Windows App Control for Business / WDAC and AppLocker | Enterprise application allow/deny controls and policy audit/enforcement on managed Windows devices. | Generic application control, not a cross-platform session relay or privacy-limited competition report; strong policy assumes managed deployment and conflicts with arbitrary CTF tools. |
| Microsoft Defender for Cloud Apps / Purview; enterprise endpoint DLP | Organization-wide discovery, policy enforcement, and audit of managed cloud/app use. | Enterprise identity/tenant and administrator model; broader monitoring than a participant-owned, nickname-only competition tool needs. |
| Netskope, DNSFilter and related secure web gateways | Centrally maintained cloud-app/category controls, including AI categories in some products. | Network/organization policy rather than transparent local competition evidence; encrypted/private/local/custom model paths remain difficult. |
| Game anti-cheat and tournament integrity systems | Deep platform-specific process/memory/driver telemetry, centralized tournament administration, sometimes kernel components. | Often invasive and game-specific; unsuitable default for open-source CTF participants and private development workstations. |
| Freedom, Cold Turkey and local productivity blockers | User-controlled schedules and app/site blocking for self-management. | Personal productivity, not shared session identities, peer-visible integrity events, tamper/gap reporting, or explicit trust boundaries. |
| Local AI blockers/browser extensions | Easy domain-level suppression in a browser or device profile. | Narrow surface, user-removable, often brand-list based; does not cover API/CLI/IDE/local inference or create a shared auditable record. |
| OpenLock (research-stage Linux exam browser) | Demonstrates a community Linux lockdown-browser direction. | Its repository describes an early project; browser kiosk scope differs from cross-platform background integrity. |

Sources for the enterprise, OS control, and AI network categories are linked in [research sources](research/sources.md#prior-art-and-ecosystem-directories).

## What is already solved and what remains

Known-domain blocking, managed application allow/deny policy, exam kiosk workflows, CTF challenge hosting, enterprise app discovery, and personal distraction blocking already exist. A new project should not rebuild those as standalone products.

The gap is a voluntarily installed and clearly limited Windows/Linux client that combines selected best-effort AI access controls with session-scoped participant keys, a relay that hides peer IP addresses, visible liveness/protection transitions, and a privacy-minimized signed end report. Existing products are adjacent building blocks; the public review found no one product that clearly combines this exact scope. That is an inference from the reviewed products, not a proof of uniqueness.

The credible open-source reason to exist is transparency: participants can inspect the registry, enforcement gaps, telemetry, relay protocol, and report format; organizers can self-host the relay; and contributors can improve platform coverage without pretending that endpoint software can control a second device or defeat local administrators.

## Review method and limits

Reviewed product documentation and repository pages for exam lockdown, CTF hosting, an example CTF AI-cheater detector, enterprise endpoint/application controls, network AI filtering, consumer productivity blockers, and Linux exam kiosk research. Search covered GitHub repositories and direct product/project sources. It did not include a systematic patent search, exhaustive package registry search, private commercial product database, or interviews with tournament organizers. Re-run before public claims of novelty.
