# Repository guidance

- Follow the documented trust boundaries and privacy rules in `docs/`.
- Distinguish verified facts, engineering inference, proposed design, and unresolved risk.
- Cite authoritative sources for OS and protocol claims and include the date checked for changing facts.
- Keep the AI registry structured, provenance-backed, versioned, and separate from enforcement code.
- Do not claim complete AI detection or resistance to a workstation administrator.
- Keep privileged operations in a small OS-specific service boundary; never put the UI in an elevated process.
- Do not collect unrelated personal data such as keystrokes, prompts, passwords, full browsing history, or filesystem contents.
- Keep commits focused and do not add generated binaries, credentials, machine-specific paths, or AI attribution.
