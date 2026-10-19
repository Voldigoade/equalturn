# Registry indicator expansion review

**Review date:** 19 October 2026
**Registry:** schema v3, semantic version 0.4.0, 131 product/provider records, 152 source records, 84 indicators.
**Scope:** researched identifiers only. This work did not add detection or enforcement code.

## What changed

The curated v3 snapshot now has 26 additional structured indicators beyond the 58 carried over from the audited v2 snapshot. Two separate product records were added for Gemini Code Assist for VS Code and the current Kimi Code CLI. The new values add coverage for npm and PyPI distributions, VS Code extension IDs, CLI command names, and OCI image references. Earlier records already cover hosted service/API hostnames, URL paths, and local ports.

Newly sourced examples include the Claude Code npm package and extension, Gemini CLI, Codex CLI, Aider, Qwen Code, Kimi Code, Mistral Vibe, Copilot/Copilot Chat, Cline, Continue, Gemini Code Assist, Amazon Q, llama.cpp, vLLM, LocalAI, Open WebUI, and LiteLLM. Every added indicator references an entry-local source with a checked date and an assessment explaining the evidence and scope. Product-specific source URLs are listed in [research sources](sources.md#registry-indicator-expansion-checked-19-october-2026).

## Classification snapshot

The v3 indicator fields have descriptive, evidence-based meanings; they are not probability estimates or runtime policies.

| Dimension | Current counts | Interpretation |
| --- | --- | --- |
| Confidence | 80 high, 4 medium, 0 low | Confidence in the exact identifier and stated scope, based on first-party or upstream documentation. It says nothing about whether the participant used the product. |
| Actionability | 65 advisory, 15 detection candidate, 3 unsafe to block, 1 research only | A candidate's potential future review use. No item is a block rule. |
| False-positive risk | 19 low, 47 medium, 18 high, 0 unknown | Risk that unrelated or allowed activity matches. Exact package or extension identity can have low match risk while still not proving use. |
| Indicator kinds | 30 web hostnames, 22 API hostnames, 10 package names, 8 VS Code extension IDs, 6 executable names, 3 container images, 3 local ports, 2 URL paths | Values are typed so later validation can apply kind-specific normalization and safety rules. |

The three local listener ports (`8080`, `11434`, and `1337`) are explicitly `unsafe_to_block` with high false-positive risk. They are configurable defaults that unrelated local programs can use. Broad/shared hosts and aggregator endpoints are advisory with elevated risk. The dataset currently contains no `blocking_candidate` indicator.

## Lifecycle and cautionary findings

- Roo Code's official Marketplace page states that the extension shut down on 15 May, without stating a year in the notice. The registry retains its extension identifier as `archived` historical research rather than implying a currently supported product.
- AWS's Amazon Q Marketplace page announces that IDE plugin support ends on 30 April 2027. The IDE extension remains current on this review date; the date is kept in the source assessment and limitation.
- The Kimi Code CLI is a separate current product record from hosted Kimi and the Kimi API. Its official docs support Moonshot-managed service and configurable compatible providers, so the CLI identifier does not identify its backend.
- Cline, Continue, Cursor, Qwen Code, Kimi Code, and LiteLLM can route to local, third-party, or configurable services. Installation identity cannot reveal the selected model.
- Open WebUI and LocalAI container images identify published software artifacts, not running containers, loaded models, user activity, or network destinations.
- `vibe`, `kimi`, and other command names are mutable and can collide with unrelated executables. They remain advisory with high false-positive risk.

## Coverage gaps

- Browser extension IDs and JetBrains plugin IDs were not added: this pass did not obtain sufficiently direct, stable upstream evidence for a representative set. The product category remains in the catalogue; lack of a structured ID is not evidence of absence.
- Desktop package IDs, signed publisher identities, and OS-specific application identifiers remain sparsely represented. Marketplace presence is not a substitute for local package metadata.
- Security/pentesting assistants remain represented as products, not as suspicious tools. A security product's intended use is not evidence of maliciousness or prohibited model use, and no stable product-specific indicator was established in this expansion.
- Custom endpoints, OpenAI-compatible self-hosted servers, user proxies, container remapping, source builds, renamed binaries, remote development, VMs, WSL, and second devices are not exhaustively enumerable from a static registry.
- Hostnames and package identifiers establish a product surface or distribution only. They do not establish user intent, a request's content, or that a product was used during a competition.

## Maintenance decision

V2 remains an immutable historical migration input. V3 is now independently curated because useful indicator classifications and newly researched identifiers cannot be losslessly regenerated from v2. The v2-to-v3 importer remains covered by deterministic migration tests, but CI no longer compares the current v3 dataset byte-for-byte with a fresh migration. Future releases must derive from the curated v3 source and a reviewed deterministic target-generation step.

The v3 validator checks Draft 2020-12 structure, semantic normalization, provenance resolution, duplicate values and sources, dates, and actionability/false-positive consistency. It also reports visible warnings separately from errors. Six repeated source URL/check-date pairs were merged into one source record per entry during this validation stage, preserving their distinct assessment text and redirecting the affected indicators. Neither validator can prove a source page's truth or that an indicator is safe to enforce.
