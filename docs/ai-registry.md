# AI ecosystem and registry design

**Research date:** 19 October 2026. Registry v2 is a researched catalogue for ecosystem discovery. After the Stage 6 quality audit, it contains 129 provider/product/surface records, 132 dated source records, and 58 structured indicators. It is incomplete, sparse in structured indicators, and is not an enforcement policy or an implementation of AI blocking. The application does not load it. See the [registry v2 quality audit](research/registry-quality-audit.md) for corrections and retained limitations.

## Method and coverage

Research started from the 18 requested capability categories rather than a provider blacklist. The expanded snapshot in `registry/registry-v2.json` contains 129 provider/product/surface records across all 18 categories, 132 dated source records, and 58 structured indicator instances. A local audit confirmed that every indicator's `source_id` resolves to a source in its own entry. This is a broad research sample, not 129 items with actionable controls: even a documented indicator may be ambiguous, configurable, shared with unrelated functions, or unsuitable for blocking. Each source record carries a URL, check date, and source-specific assessment. First-party product/API documentation and upstream project repositories provide provenance; where a value or scope was not explicit, the record preserves a written research assessment without inventing a structured indicator.

**Source ID scope:** `source-1`, `source-2`, and similar labels are local to a registry entry. They intentionally repeat across different entries; the current snapshot has 132 source records but only two distinct ID strings globally. Resolve an indicator's `source_id` only against the `sources` array in that same entry. These labels are not stable, globally addressable source IDs and must not be used as cross-entry keys. A future registry revision may adopt globally unique IDs only with an explicit migration.

The original 30-record sample remains in `registry/registry-v1.json`, described by `registry/schema-v1.schema.json`. Those v1 artifacts are unchanged. V2 is the expanded research snapshot described by `registry/schema-v2.schema.json`. The quality audit split Mistral Vibe Code and MiniMax Code from broader chat/agent records, corrected two hosted-chat tags, removed an unsupported generic TGI port, and refreshed product status/provenance where evidence supported it. Neither version estimates the full market or provides a completeness score. Product names, endpoints, ownership, and routing capabilities change quickly; this snapshot was checked on 19 October 2026.

| Category | Examples investigated | What a registry can use | Main blind spot |
| --- | --- | --- | --- |
| Hosted conversational AI | ChatGPT, Claude, Gemini, Grok, DeepSeek, Mistral Vibe Chat/Work, Perplexity, Poe, Venice, Meta AI | Web domains, documented product APIs, package/app identity where stable | New brands, regional domains, web proxies, shared hosting, custom gateways. |
| Model-provider APIs | OpenAI, Anthropic, Google, xAI, DeepSeek, Mistral, Cohere | Documented API hostnames, SDK/package identifiers where stable | Direct HTTP clients and custom DNS/IP; API traffic may be embedded in another app. |
| Inference aggregators/gateways | OpenRouter, Hugging Face Inference Providers, LiteLLM, Vercel AI Gateway | Gateway endpoints, known SDK/package IDs | Gateway routes across many providers; one endpoint can also host non-AI functionality. |
| Coding assistants | GitHub Copilot, Cursor, Windsurf, Continue, Cline, Roo Code, OpenHands, Mistral Vibe Code, MiniMax Code | Package IDs, signed app metadata, known domains | BYOK, custom provider routing, updates, portable/recompiled clients. |
| IDE-native assistants | VS Code, JetBrains, Visual Studio, Neovim integrations and plugins | Extension/plugin IDs when stable and locally observable | Forked IDEs, profile isolation, custom plugins, remote development. |
| Browser extensions | Cline-like agent extensions, provider extensions, search assistants | Browser extension store IDs if exposed; managed browser policy | User profiles, sideloading, renamed/repacked extensions, browser differences. |
| Desktop applications | ChatGPT, Claude, LM Studio, Poe and vendor desktop clients | Package/bundle IDs, signed publisher metadata, process path as weak signal | Portable builds, renamed executables, embedded browser, remote desktop. |
| CLI coding agents | Claude Code, Gemini CLI, Codex, OpenHands, Aider, Continue CLI | Package IDs, publisher signatures, known network endpoints | Scripts, package managers, self-built binaries, SSH/remote execution. |
| Cybersecurity/pentesting AI tools | PentestGPT, Burp AI-assisted DAST features, agentic security workflows | Product/package ID and configured provider integration | Tool function may be permitted while model assistance is not; custom APIs. |
| Local model runners | Ollama, llama.cpp, LM Studio, LocalAI, Jan | Known process/package identifiers, default localhost ports | Custom builds, ports, embedded runtimes, model format alone is not a runtime signal. |
| Inference servers | vLLM, TGI, llama.cpp server, TensorRT-LLM | Process/package metadata, documented listener ports | Containers, private ports, source builds, model serving embedded in another service. |
| Local web interfaces | Open WebUI, text-generation-webui, AnythingLLM | Default ports, package/container metadata | Custom port, reverse proxy, private container network, alternative interface. |
| Self-hosted AI stacks | Ollama + Open WebUI, LocalAI, LiteLLM, vLLM/TGI stacks | Provider relationships and endpoint patterns | Arbitrary compositions and locally compiled clients. |
| Cloud model marketplaces | AWS Bedrock Marketplace, Azure AI Foundry model catalog, Hugging Face providers | Cloud endpoints, account policy where managed | Shared cloud service domains, custom deployments, gateway routing. |
| Multimodal AI services | Gemini, OpenAI, Claude, Grok and hosted speech/image/video APIs | Same provider/product indicators plus documented specialized endpoints | New modalities may use existing shared API domains; content is not inspected. |
| Search/research assistants | Perplexity, Scite, You.com, Poe, browser search assistants | Product domains/app identifiers | Ordinary search traffic, plugins, provider changes, and embedded widgets. |
| Lesser-known/regional providers | Qwen, Kimi/Moonshot, DeepSeek, Venice, Cohere and regional model services | First-party docs, verified domains, registry contributions with provenance | Rapid launches, multilingual naming, reseller domains, weak documentation. |
| Tools capable of routing to other providers | Cursor, Cline, Roo Code, LiteLLM, OpenRouter, OpenHands, Continue | `routes_to`/`integrates_with` relationships, configurable endpoint indicator | A brand’s activity does not reveal which model/provider was chosen; routes are user-configurable. |

## Why a manually curated blacklist is insufficient

An endpoint can be an official provider API, an aggregator, a user-configured proxy, a local model, an arbitrary server, or a shared cloud domain. A brand/domain list cannot identify semantic model usage across encrypted HTTPS, locally embedded runtimes, self-hosted services, custom ports, containers, or remote systems. Conversely, blocking a broad cloud hostname can break unrelated development work. V2 is a research catalogue, not an oracle or deny list; its records do not establish which product or model a participant used.

V2 structured indicators contain an evidence kind, value, scope, source reference, and limitations. They intentionally have no confidence or actionability fields: a documented host, path, package name, process name, or listener is not automatically a block rule. Source-specific `indicator_assessment` text preserves caveats, including shared hosts, configurable routes, and weak clues. Any future enforcement policy must separately review false positives, intended action, confidence, and platform-specific feasibility. The v2 quality audit removed the TGI example port `8080` because it is generic and not a product identity. Remaining local listeners are configurable observations, not block rules. Unknown applications and unlisted providers must remain unknown rather than being silently treated as clean.

## Schema and update lifecycle

`registry/schema-v1.schema.json` continues to validate the unchanged v1 sample. `registry/schema-v2.schema.json` defines the expanded research format in `registry/registry-v2.json`. V2 separates:

- stable IDs for provider and product records;
- the 18 descriptive capability categories and client surfaces, which do not guarantee that a surface is detectable or blockable;
- source records with URL, `checked_on`, and source-specific `indicator_assessment`;
- structured evidence indicators with kind, value, scope, source reference, and limitations, but no confidence or intended action;
- optional product relationships such as `routes_to`, `provided_by`, or `integrates_with` (the current expansion leaves them empty where a specific relationship was not established);
- research status (`observed`, `unknown`, `deprecated`, `archived`, or `disputed`).

V2 retains the ten v1 surface values and adds four values needed to preserve researched facts: `mobile`, `browser` (browser-integrated features), `local_server`, and `python_library`. `self-hosted` is a deployment mode, not a surface; the data keeps that distinction in category/source assessment text rather than pretending it is an access surface.

Keep the dataset separate from enforcement code. Add a record only with a first-party or independently credible source and a check date. Add a structured indicator only when its specific value and scope are supported; record shared/region-specific hosts, configurable endpoints, and false-positive or coverage limits in the source assessment. Retain archived or deprecated entries with status and migration notes rather than silently deleting history.

The v2 JSON Schema describes structural shape and enumerated values. The dependency-free `tools/validate_registry.py` check also enforces entry and entry-local source-ID uniqueness, indicator-to-source references, relationship targets, dates, and absolute HTTP(S) provenance URLs; CI runs it on every change. Neither schema nor checker can prove that a cited page supports a claim, assess false positives, or apply a published freshness policy. Human review remains required for provenance and indicator scope.

The current application does not consume or update this registry. A future independent-update design may publish versioned research artifacts with signed metadata. TUF is a suitable design family for repository roles, expiration, rollback, and freeze protection; use a maintained implementation and a documented offline root-key ceremony rather than inventing signatures. Any future client should pin trusted root metadata, report expired/invalid data clearly, and never silently change an active session’s registry. No live registry update path, signature verification, or remotely downloaded enforcement policy is implemented here.

## Sources

The primary ecosystem sources, API documentation, project repositories, and TUF/RFC references are listed in [research sources](research/sources.md#ai-ecosystem-and-registry).
