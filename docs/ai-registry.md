# AI ecosystem and registry design

**Research date:** 19 October 2026. The registry sample is illustrative, incomplete, and not suitable as an enforcement policy without review.

## Method and coverage

Research started from the 18 requested capability categories rather than a provider blacklist. Representative provider/product claims were checked against official product/API documentation and selected project repositories. Directory/catalogue sources such as Hugging Face Inference Providers, OpenRouter’s model catalogue, and cloud model catalogs were used to discover breadth; important entries were checked against the provider or project’s own docs. The sample records links and check dates so future maintainers can repeat that review.

The investigation mapped all 18 categories below and places 30 representative provider/product records in `registry/registry-v1.json`. That count is a demonstrator only. It is not an estimate of the full market or a completeness score. Product names, endpoints, ownership, and routing capabilities change quickly; the sources were last checked on 19 October 2026.

| Category | Examples investigated | What a registry can use | Main blind spot |
| --- | --- | --- | --- |
| Hosted conversational AI | ChatGPT, Claude, Gemini, Grok, DeepSeek, Le Chat, Perplexity, Poe, Venice, Meta AI | Web domains, documented product APIs, package/app identity where stable | New brands, regional domains, web proxies, shared hosting, custom gateways. |
| Model-provider APIs | OpenAI, Anthropic, Google, xAI, DeepSeek, Mistral, Cohere | Documented API hostnames, SDK/package identifiers where stable | Direct HTTP clients and custom DNS/IP; API traffic may be embedded in another app. |
| Inference aggregators/gateways | OpenRouter, Hugging Face Inference Providers, LiteLLM, Vercel AI Gateway | Gateway endpoints, known SDK/package IDs | Gateway routes across many providers; one endpoint can also host non-AI functionality. |
| Coding assistants | GitHub Copilot, Cursor, Windsurf, Continue, Cline, Roo Code, OpenHands | Package IDs, signed app metadata, known domains | BYOK, custom provider routing, updates, portable/recompiled clients. |
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
| Search/research assistants | Perplexity, Phind, You.com, Poe, browser search assistants | Product domains/app identifiers | Ordinary search traffic, plugins, provider changes, and embedded widgets. |
| Lesser-known/regional providers | Qwen, Kimi/Moonshot, DeepSeek, Venice, Cohere and regional model services | First-party docs, verified domains, registry contributions with provenance | Rapid launches, multilingual naming, reseller domains, weak documentation. |
| Tools capable of routing to other providers | Cursor, Cline, Roo Code, LiteLLM, OpenRouter, OpenHands, Continue | `routes_to`/`integrates_with` relationships, configurable endpoint indicator | A brand’s activity does not reveal which model/provider was chosen; routes are user-configurable. |

## Why a manually curated blacklist is insufficient

An endpoint can be an official provider API, an aggregator, a user-configured proxy, a local model, an arbitrary server, or a shared cloud domain. A brand/domain list cannot identify semantic model usage across encrypted HTTPS, locally embedded runtimes, self-hosted services, custom ports, containers, or remote systems. Conversely, blocking a broad cloud hostname can break unrelated development work. The registry is a set of explainable indicators and relationships, not an oracle.

Enforcement must combine independent layers: provider/API endpoint indicators, managed application/package identity where available, selected process/runtime observations, OS network controls, local endpoint/port checks, and explicit coverage/liveness reporting. Each indicator needs a confidence, actionability, source, verification date, and false-positive notes. Unknown applications and unlisted providers should remain explicitly unknown; they must not be silently treated as clean.

## Schema and update lifecycle

`registry/schema-v1.schema.json` validates `registry/registry-v1.json`. The v1 design separates:

- stable IDs for provider and product records;
- categories and client surfaces, which are descriptive rather than enforcement guarantees;
- indicators with a match type, value, confidence, intended action, and provenance;
- product relationships such as `routes_to`, `provided_by`, or `integrates_with`;
- current, deprecated, or disputed state and review notes.

Keep the dataset outside the enforcement code. Add a change only with a first-party or independently credible source, direct indicator verification, a check date, a false-positive analysis, and a reviewer. Use exact domains by default; suffix matching must mean a true DNS label boundary. Never treat a substring as a domain match. Record shared/region-specific hosts and custom endpoint support. Retain deprecated entries with status and migration notes rather than silently deleting history.

The JSON Schema checks shape and allowed values; it cannot verify that a cited page supports an indicator. A future registry checker must additionally reject duplicate IDs, dangling relationship targets, malformed hostnames, and stale source records according to a published freshness policy. Human review remains required for provenance and false-positive assessment.

For independent updates, publish versioned registry artifacts with signed metadata. TUF is a suitable established design family for repository roles, expiration, rollback, and freeze protection; adopt a maintained implementation and a documented offline root-key ceremony rather than inventing signatures. The client must pin trusted root metadata and report an expired/invalid registry clearly. Do not fetch unsigned updates or silently change an active session’s registry.

The first development milestone should validate the schema, source requirement, deterministic serialization, and signature/update verification locally. It should not yet add a live auto-update path or remotely downloaded enforcement policy.

## Sources

The primary ecosystem sources, API documentation, project repositories, and TUF/RFC references are listed in [research sources](research/sources.md#ai-ecosystem-and-registry).
