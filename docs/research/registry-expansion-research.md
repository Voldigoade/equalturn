# AI ecosystem registry expansion research

**Checked:** 19 October 2026  
**Scope:** research artifact only. The original Phase 0 v1 sample and schema are unchanged; this expansion is published as `registry/registry-v2.json` with its own v2 schema. Neither registry version is an enforcement policy.  
**Initial expansion count:** 127 distinct provider/product/surface records across the requested categories. The subsequent [Stage 6 quality audit](registry-quality-audit.md) split Mistral Vibe Code and MiniMax Code into separate records; the current v2 registry has 129 entries, 132 provenance records, and 58 indicators. This document preserves the initial 127-row research ledger rather than silently rewriting its historical grouping.
**Evidence standard:** each row links to a first-party product page, documentation page, or upstream project repository checked on the date above. A URL is provenance for the product/surface claim, not an endorsement or proof that every deployment uses that route.

## Findings

The expanded sample supports the Phase 0 conclusion: a giant brand blacklist is not a sufficient control model. Many coding agents can select providers, use BYOK, call OpenAI-compatible endpoints, or point to a user-controlled base URL. Local runners and interfaces can bind to configurable ports, while cloud marketplaces and aggregators can expose many providers through a shared endpoint. The same vendor also appears as a consumer chat service, developer API, IDE feature, CLI, browser surface, or marketplace deployment. A match at one layer does not identify which model was used or what the user did.

The table therefore keeps product/surface facts separate from indicators. `Exact web host` means the first-party product hostname can be a **block candidate** under an agreed policy; it does not prove AI intent and may cover help, auth, static assets, or non-AI features. `API host` is a higher-confidence destination candidate only where first-party API documentation gives a concrete host; HTTPS still hides request semantics and gateways/proxies can reroute traffic. `Default local port` is a **detection candidate**, never a product identity: it is configurable, may collide with unrelated software, and may be invisible across namespaces/VMs. `None` means no stable, exclusive indicator was verified; it does not mean the product is safe or inactive.

For schema compatibility, every category tag below is from the existing v1 category enum. The Surface column is descriptive research metadata, not a v1-ready field: v1 has no native mobile surface and does not distinguish a browser app from a browser extension, while this ledger also uses descriptive terms such as `mobile`, `browser`, `local server`, `Python library`, and `self-hosted`. Normalize only when the meaning is preserved; otherwise omit the unsupported surface or propose a reviewed schema change. This ledger adds no fields to the canonical schema. A future data revision should preserve source provenance, check dates, indicator confidence/actionability, false-positive notes, status, and routing relationships. Model names are intentionally not enumerated: catalogs churn quickly and a model name alone is rarely an endpoint indicator.

## Coverage

| Research group | Records |
| --- | ---: |
| Hosted chat, research/search, multimodal and creative services | 33 |
| Model-provider APIs and cloud model services | 26 |
| Gateways and routers | 10 |
| Coding, IDE, desktop and CLI assistants | 26 |
| Browser assistants and browser surfaces | 7 |
| AI-assisted cybersecurity and AI-security tools | 7 |
| Local runtimes, inference servers and local interfaces | 18 |
| **Total** | **127** |

## Researched product and surface ledger

All source links were checked 2026-10-19. Indicators are deliberately conservative. Exact hostnames below refer to the canonical first-party surface shown by the source; they are not a complete network allow/deny list.

### Hosted chat, search/research, multimodal and creative services (33)

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 1 | OpenAI — ChatGPT | `hosted_chat`, `multimodal` | web, desktop | `chatgpt.com` exact host; identifies service surface, not user intent. | [ChatGPT](https://chatgpt.com/) |
| 2 | Anthropic — Claude | `hosted_chat`, `multimodal` | web, desktop | `claude.ai` exact host; API and other clients are separate. | [Claude](https://claude.ai/) |
| 3 | Google — Gemini | `hosted_chat`, `multimodal`, `search_research` | web, mobile | `gemini.google.com` exact host; shared Google services remain outside this match. | [Gemini](https://gemini.google.com/) |
| 4 | xAI — Grok | `hosted_chat`, `multimodal`, `search_research` | web, mobile | `grok.com` exact host for this consumer surface; API is separate. | [Grok](https://grok.com/) |
| 5 | DeepSeek — Chat | `hosted_chat`, `multimodal` | web, mobile | `chat.deepseek.com` exact host; does not cover API/gateway use. | [DeepSeek Chat](https://chat.deepseek.com/) |
| 6 | Moonshot AI — Kimi | `hosted_chat`, `search_research`, `multimodal` | web, mobile | `kimi.com` exact host for the product; regional and API surfaces may differ. | [Kimi product](https://www.kimi.com/en/) |
| 7 | Alibaba — Qwen Chat | `hosted_chat`, `multimodal` | web, mobile | `chat.qwen.ai` exact host; Alibaba also offers regional cloud APIs. | [Qwen Chat](https://chat.qwen.ai/) |
| 8 | Mistral AI — Vibe (formerly Le Chat) | `hosted_chat`, `multimodal`, `search_research` | web, mobile | `chat.mistral.ai` is the product host; the product page now brands the experience Vibe and says it was formerly Le Chat. This does not identify API use or intent. | [Mistral Vibe product](https://mistral.ai/products/vibe/) |
| 9 | Meta — Meta AI | `hosted_chat`, `multimodal` | web, mobile | `meta.ai` exact host; AI features embedded in other Meta apps are separate. | [Meta AI](https://www.meta.ai/) |
| 10 | Microsoft — Copilot | `hosted_chat`, `multimodal`, `search_research` | web, desktop, mobile | `copilot.microsoft.com` exact host; Copilot is also embedded across Microsoft products. | [Microsoft Copilot](https://copilot.microsoft.com/) |
| 11 | Perplexity — Perplexity | `hosted_chat`, `search_research`, `multimodal` | web, mobile | `perplexity.ai` exact host for the consumer product; Comet is separate below. | [Perplexity](https://www.perplexity.ai/) |
| 12 | Quora — Poe | `hosted_chat`, `router`, `multimodal` | web, desktop, mobile | `poe.com` exact host; bots may route to different underlying providers. | [Poe](https://poe.com/) |
| 13 | Venice — Venice Chat / Agent | `hosted_chat`, `router`, `multimodal` | web, API | `venice.ai` exact host; product documentation describes model choice and agent routing. | [Venice Chat](https://venice.ai/lp/ai-chat) |
| 14 | Hugging Face — HuggingChat | `hosted_chat`, `gateway` | web | `huggingface.co` is shared with model hosting and other services; host-only match is broad. | [HuggingChat](https://huggingface.co/chat/) |
| 15 | Z.ai — Chat | `hosted_chat`, `regional_provider` | web, mobile | `chat.z.ai` product host; no claim that it identifies all Z.ai API use. | [Z.ai Chat](https://chat.z.ai/) |
| 16 | MiniMax — Agent / MiniMax Code suite | `hosted_chat`, `multimodal`, `regional_provider`, `coding_assistant`, `desktop_app`, `cli_agent` | web, desktop, mobile, CLI | `agent.minimax.io` is a current web product host; `chat.minimax.io` redirects to a regional MiniMax Agent host. Official product material also documents MiniMax Code desktop and CLI surfaces, but supplies no stable package/process identifier. API and enterprise endpoints are separate. | [MiniMax Agent](https://agent.minimax.io/); [MiniMax Code desktop and CLI](https://agent.minimax.io/download) |
| 17 | ByteDance — Doubao | `hosted_chat`, `multimodal`, `regional_provider` | web, mobile | First-party product host is a candidate; localized app surfaces and API hosts differ. | [Doubao](https://www.doubao.com/chat/) |
| 18 | Baidu — ERNIE Bot / Wenxin Yiyan | `hosted_chat`, `multimodal`, `regional_provider` | web, mobile | The older `yiyan.baidu.com` URL redirects to `wenxin.baidu.com`, whose page was inaccessible to this checker; Baidu identifies ERNIE Bot as a conversational assistant. Treat the current app hostname as weak/unstable evidence and re-verify before use. | [Baidu product description](https://ir.baidu.com/baidu-general-business); [legacy app URL](https://yiyan.baidu.com/) |
| 19 | Tencent — Yuanbao | `hosted_chat`, `multimodal`, `regional_provider` | web, mobile | `yuanbao.tencent.com` product host; Tencent Cloud APIs are a separate surface. | [Yuanbao](https://yuanbao.tencent.com/) |
| 20 | Character Technologies — Character.AI | `hosted_chat` | web, mobile | `character.ai` exact host; character roleplay and general assistant use share the service. | [Character.AI](https://character.ai/) |
| 21 | You.com — You.com AI | `hosted_chat`, `search_research` | web | `you.com` exact host; multiple search and agent features share the domain. | [You.com](https://you.com/) |
| 22 | Scite — AI Assistant | `hosted_chat`, `search_research` | web | `scite.ai/assistant` is the product surface; the broader domain also serves search and citation tools, so a domain match does not establish assistant use. | [Scite AI Assistant](https://scite.ai/assistant) |
| 23 | Google — NotebookLM | `search_research`, `multimodal` | web | `notebooklm.google.com` exact app host; Google account infrastructure is shared. | [NotebookLM](https://notebooklm.google.com/) |
| 24 | Google — AI Studio | `provider_api`, `multimodal` | web, API | `aistudio.google.com` is a developer playground surface, not the only Gemini API route. | [Google AI Studio](https://aistudio.google.com/) |
| 25 | Elicit — Elicit | `search_research` | web | `elicit.com` exact host; a product hostname does not identify exported or API-mediated use. | [Elicit](https://elicit.com/) |
| 26 | Consensus — Consensus | `search_research` | web | `consensus.app` exact host; search/research features share the service. | [Consensus](https://consensus.app/) |
| 27 | Gamma — Gamma AI | `multimodal`, `hosted_chat` | web | `gamma.app` exact product host; presentation and document generation are not separable by hostname. | [Gamma](https://gamma.app/) |
| 28 | Canva — Magic Studio | `multimodal`, `hosted_chat` | web, desktop | `canva.com` is shared with non-AI design tools; broad host blocking has high false-positive risk. | [Magic Studio](https://www.canva.com/magic-studio/) |
| 29 | Adobe — Firefly | `multimodal` | web, desktop | `firefly.adobe.com` product host; Firefly features are also embedded in Creative Cloud. | [Adobe Firefly](https://firefly.adobe.com/) |
| 30 | Midjourney — Midjourney | `multimodal` | web, mobile | `midjourney.com` product surface; third-party clients are not covered. | [Midjourney](https://www.midjourney.com/) |
| 31 | Runway — Runway | `multimodal` | web, desktop | `runwayml.com` product host; exact service host(s) can change. | [Runway](https://runwayml.com/) |
| 32 | ElevenLabs — ElevenLabs | `multimodal`, `provider_api` | web, API | Product/API documentation exists; API hostname should be taken from current docs, not guessed from brand domain. | [ElevenLabs API docs](https://elevenlabs.io/docs/overview) |
| 33 | Suno — Suno | `multimodal` | web, mobile | `suno.com` product host; identifies the service, not whether a session-relevant feature was used. | [Suno](https://suno.com/) |

### Model-provider APIs and cloud model services (26)

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 34 | OpenAI — API | `provider_api`, `multimodal` | API, cloud | Exact API host `api.openai.com` documented; host does not identify operation/model. | [API overview](https://platform.openai.com/docs/api-reference/introduction) |
| 35 | Anthropic — Claude API | `provider_api`, `multimodal` | API, cloud | Exact host `api.anthropic.com` documented. | [API overview](https://platform.claude.com/docs/en/api/overview) |
| 36 | Google — Gemini Developer API | `provider_api`, `multimodal` | API, cloud | `generativelanguage.googleapis.com` documented; this does not cover Vertex or proxies. | [Gemini API key and endpoint docs](https://ai.google.dev/gemini-api/docs/api-key) |
| 37 | Google Cloud — Vertex AI generative models | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | Regional/project endpoints are parameterized; no single exact host is safe as a global indicator. | [Vertex AI generative quickstart](https://cloud.google.com/vertex-ai/generative-ai/docs/start/quickstarts/quickstart-multimodal) |
| 38 | Microsoft — Azure AI Foundry / Azure OpenAI | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | Customer deployment hostnames vary; avoid broad `azure.com`/`openai.azure.com` assumptions. | [Azure OpenAI overview](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/overview) |
| 39 | AWS — Bedrock Runtime / Marketplace models | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | Region/account endpoints vary and AWS service domains are shared. | [Bedrock Marketplace](https://docs.aws.amazon.com/bedrock/latest/userguide/amazon-bedrock-marketplace.html) |
| 40 | xAI — API | `provider_api`, `multimodal` | API, cloud | `api.x.ai` is documented; endpoint is not exclusive to a product feature. | [xAI API docs](https://docs.x.ai/) |
| 41 | DeepSeek — API | `provider_api`, `multimodal` | API, cloud | `api.deepseek.com` documented; proxies and compatible gateways bypass this destination. | [DeepSeek API guide](https://api-docs.deepseek.com/guides/responses_api/) |
| 42 | Moonshot AI — Kimi API | `provider_api`, `regional_provider` | API, cloud | Provider-specific regional endpoints; use current first-party endpoint table, not a guessed single host. | [Moonshot API documentation](https://platform.moonshot.cn/docs) |
| 43 | Alibaba Cloud — Qwen / Model Studio (DashScope) | `provider_api`, `regional_provider`, `multimodal` | API, cloud | Region- and workspace-specific hosts; wildcard suffixes would be broad and account-dependent. | [Model Studio endpoint matrix](https://help.aliyun.com/en/model-studio/base-url) |
| 44 | Mistral AI — API | `provider_api`, `multimodal` | API, cloud | Exact host `api.mistral.ai` documented in quickstart. | [First API request](https://docs.mistral.ai/getting-started/quickstarts/developer/first-api-request) |
| 45 | Cohere — API | `provider_api`, `multimodal` | API, cloud | `api.cohere.ai` documented by compatibility API; private deployments can differ. | [Cohere compatibility API](https://docs.cohere.com/v2/docs/compatibility-api) |
| 46 | Groq — GroqCloud API | `provider_api`, `multimodal` | API, cloud | Exact API host `api.groq.com` and base path documented. | [Groq API reference](https://console.groq.com/docs/api-reference) |
| 47 | Together AI — Inference API | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | API endpoint is documented; customer or compatible proxies may replace it. | [Together API docs](https://docs.together.ai/docs/introduction) |
| 48 | Fireworks AI — inference API | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | `api.fireworks.ai` documented; multiple product APIs share the host. | [Fireworks API docs](https://docs.fireworks.ai/) |
| 49 | Cerebras — Inference API | `provider_api`, `multimodal` | API, cloud | `api.cerebras.ai` documented; API requests can be routed through gateways. | [Cerebras API docs](https://inference-docs.cerebras.ai/) |
| 50 | DeepInfra — inference API | `provider_api`, `cloud_marketplace` | API, cloud | `api.deepinfra.com` documented by API references; model endpoint path varies. | [DeepInfra API reference](https://docs.deepinfra.com/api-reference) |
| 51 | Replicate — model API | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | `api.replicate.com` documented; API is general-purpose and supports many models/tasks. | [Replicate HTTP API](https://replicate.com/docs/reference/http) |
| 52 | Novita AI — inference API | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud | `api.novita.ai` documented; product endpoints are versioned and task-specific. | [Novita API reference](https://novita.ai/docs/api-reference/introduction) |
| 53 | SiliconFlow — OpenAI-compatible API | `provider_api`, `cloud_marketplace`, `regional_provider` | API, cloud | Regional hosts `api.siliconflow.cn` / `api.ap.siliconflow.com`; exact region matters. | [SiliconFlow quickstart](https://api-docs.siliconflow.cn/docs/userguide/quickstart) |
| 54 | SambaNova — Cloud API | `provider_api`, `multimodal` | API, cloud | `api.sambanova.ai` documented for cloud; self-hosted SambaStack URL is operator-defined. | [SambaNova model list endpoint](https://docs-prod.sambanova.ai/docs/api-reference/endpoints/model-list) |
| 55 | AI21 Labs — Jamba API | `provider_api`, `multimodal` | API, cloud | `api.ai21.com` documented; account/private deployment variants need separate evidence. | [AI21 API reference](https://docs.ai21.com/reference) |
| 56 | NVIDIA — NIM inference microservices | `provider_api`, `cloud_marketplace`, `multimodal` | API, cloud, self-hosted | Hosted and self-hosted deployment endpoints vary; no exclusive hostname asserted. | [NIM LLM documentation](https://docs.nvidia.com/nim/large-language-models/latest/introduction.html) |
| 57 | Baidu — Qianfan model platform | `provider_api`, `cloud_marketplace`, `regional_provider` | API, cloud | Account/region-specific cloud endpoints; broad Baidu domains create unrelated-service matches. | [Qianfan API docs](https://cloud.baidu.com/doc/WENXINWORKSHOP/index.html) |
| 58 | Z.ai — GLM API | `provider_api`, `regional_provider` | API, cloud | `api.z.ai` documented in API references; compatibility routes can vary. | [Z.ai API reference](https://docs.z.ai/api-reference/introduction) |
| 59 | MiniMax — API | `provider_api`, `multimodal`, `regional_provider` | API, cloud | The current docs describe API bases `api.minimax.io/anthropic` and `api.minimax.io/v1`; account, region, and compatible gateways can still change the route. | [MiniMax text-generation API docs](https://platform.minimax.io/docs/guides/text-generation) |

### Gateways and routers (10)

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 60 | OpenRouter — model gateway | `gateway`, `router`, `provider_api`, `cloud_marketplace` | web, API | `openrouter.ai` exact host; shared gateway path can route to many underlying providers. | [OpenRouter models API](https://openrouter.ai/docs/api/api-reference/models/get-models) |
| 61 | Hugging Face — Inference Providers / router | `gateway`, `router`, `cloud_marketplace` | API, cloud | Router endpoint `router.huggingface.co` is documented; Hugging Face hosts many unrelated services. | [Inference Providers docs](https://huggingface.co/docs/inference-providers/index) |
| 62 | BerriAI — LiteLLM Proxy / AI Gateway | `gateway`, `router`, `self_hosted_stack` | local server, API | Deployment hostname and port are operator-configured; no static global indicator. | [LiteLLM AI Gateway](https://docs.litellm.ai/docs/simple_proxy) |
| 63 | Vercel — AI Gateway | `gateway`, `router` | cloud, API | Account/project-specific gateway configuration; no unique client-side host can safely identify all use. | [Vercel AI Gateway](https://vercel.com/docs/ai-gateway) |
| 64 | Cloudflare — AI Gateway | `gateway`, `router` | cloud, API | Account/gateway IDs appear in URLs under a shared Cloudflare host; host-only rule is broad. | [Cloudflare AI Gateway](https://developers.cloudflare.com/ai-gateway/) |
| 65 | Portkey — AI Gateway | `gateway`, `router` | cloud, API | Gateway region/virtual key and self-hosted modes vary; no exclusive network indicator asserted. | [Portkey docs](https://docs.portkey.ai/docs/introduction) |
| 66 | Helicone — AI Gateway | `gateway`, `router` | cloud, API | Gateway endpoint is configurable and may proxy provider traffic; observability and gateway products share a vendor. | [Helicone AI Gateway docs](https://docs.helicone.ai/gateway/overview) |
| 67 | Kong — AI Gateway | `gateway`, `router`, `self_hosted_stack` | cloud, server | Customer-managed gateway host is arbitrary. | [Kong AI Gateway](https://docs.konghq.com/gateway/latest/ai-gateway/) |
| 68 | Eden AI — Unified AI API | `gateway`, `router`, `cloud_marketplace` | API, cloud | `api.edenai.run` service host; multiple AI tasks share the API and model selection is per request. | [Eden AI API docs](https://docs.edenai.co/v3) |
| 69 | TrueFoundry — LLM Gateway | `gateway`, `router`, `self_hosted_stack` | cloud, server, API | Tenant deployment endpoints are configurable; no global exclusive hostname. | [TrueFoundry LLM Gateway](https://docs.truefoundry.com/docs/llm-gateway) |

### Coding, IDE, desktop and CLI assistants (26)

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 70 | GitHub — Copilot IDE integrations | `coding_assistant`, `ide_assistant` | IDE | No exclusive process/domain indicator asserted; editor and account infrastructure are shared. | [Where Copilot is available](https://docs.github.com/en/copilot/get-started/where-to-use-github-copilot) |
| 71 | GitHub — Copilot CLI | `coding_assistant`, `cli_agent` | CLI | No stable executable signature asserted; install method and binary name can change. | [Copilot CLI documentation](https://docs.github.com/en/copilot/how-tos/copilot-cli) |
| 72 | Anysphere — Cursor IDE | `coding_assistant`, `ide_assistant`, `desktop_app`, `router` | desktop, IDE | BYOK/provider routing documented; no unique endpoint/process identifies model use. | [Cursor API keys](https://docs.cursor.com/settings/api-keys) |
| 73 | Anysphere — Cursor Agent CLI | `coding_assistant`, `cli_agent`, `router` | CLI | `cursor-agent` is documented but rename/rebuild/custom endpoint bypasses process matching. | [Cursor CLI authentication](https://docs.cursor.com/en/cli/reference/authentication) |
| 74 | Cognition / Windsurf — Windsurf editor and Cascade | `coding_assistant`, `ide_assistant`, `desktop_app` | desktop, IDE | No verified exclusive process/domain indicator; product uses hosted services and integrations. | [Windsurf documentation](https://docs.windsurf.com/) |
| 75 | Continue — Continue IDE assistant | `coding_assistant`, `ide_assistant`, `router` | IDE | User-configurable providers, local models, and `apiBase` make network indicator non-exclusive. | [Continue customization](https://docs.continue.dev/customize/overview) |
| 76 | Cline — VS Code agent | `coding_assistant`, `ide_assistant`, `router` | IDE, browser extension | Provider and base URL are configurable; no fixed provider destination. | [Cline model provider documentation](https://github.com/cline/cline/blob/main/docs/sdk/model-providers.mdx) |
| 77 | Roo Code — VS Code agent | `coding_assistant`, `ide_assistant`, `router` | IDE, browser extension | Supports multiple API providers and Roo Router; no fixed model host. | [Roo Code provider setup](https://docs.roocode.com/getting-started/connecting-api-provider) |
| 78 | Kilo Code — IDE extension | `coding_assistant`, `ide_assistant`, `router` | IDE | Multi-provider/model selection means underlying provider is not implied by Kilo identity. | [Kilo Code repository](https://github.com/kilo-org/kilocode) |
| 79 | Kilo Code — CLI | `coding_assistant`, `cli_agent`, `router` | CLI | Same configurable provider issue; package is `@kilocode/cli`, but package identity is only a weak local clue. | [Kilo CLI docs](https://kilo.ai/docs/code-with-ai/platforms/cli) |
| 80 | Anthropic — Claude Code | `coding_assistant`, `cli_agent`, `router` | CLI | Gateway and custom endpoint configuration documented; no single mandatory network route. | [Claude Code overview](https://docs.anthropic.com/en/docs/claude-code/overview) |
| 81 | Google — Gemini CLI | `coding_assistant`, `cli_agent` | CLI | Official source confirms CLI surface; process metadata is mutable and no exclusive signature asserted. | [Gemini CLI repository](https://github.com/google-gemini/gemini-cli) |
| 82 | OpenAI — Codex CLI | `coding_assistant`, `cli_agent` | CLI | Official source confirms CLI; process name is bypassable and provider endpoints are shared. | [Codex CLI repository](https://github.com/openai/codex) |
| 83 | AWS — Amazon Q Developer | `coding_assistant`, `ide_assistant`, `cli_agent` | IDE, CLI | AWS account/region and IDE surfaces vary; no single endpoint safely isolates Q usage. | [Amazon Q Developer docs](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/what-is.html) |
| 84 | JetBrains — AI Assistant | `coding_assistant`, `ide_assistant`, `router` | IDE | Supports third-party/local models and custom OpenAI-compatible endpoints. | [Use third-party and local models](https://www.jetbrains.com/help/ai-assistant/use-custom-models.html) |
| 85 | JetBrains — Junie | `coding_assistant`, `ide_assistant` | IDE | Integrated in JetBrains IDEs; no stable standalone process/network indicator. | [JetBrains AI service products](https://www.jetbrains.com/help/ai/jetbrains-ai.html) |
| 86 | Tabnine — AI code assistant | `coding_assistant`, `ide_assistant`, `desktop_app` | IDE, desktop, cloud | Supports third-party model choices and private installations; no exclusive host/process. | [Tabnine AI models](https://docs.tabnine.com/main/welcome/readme/ai-models) |
| 87 | Sourcegraph — Amp | `coding_assistant`, `cli_agent`, `ide_assistant` | CLI, IDE | Product spans CLI/editor and uses multiple models; no unique model-provider indicator. | [Amp documentation](https://ampcode.com/manual) |
| 88 | Sourcegraph — Cody | `coding_assistant`, `ide_assistant`, `search_research` | IDE, web | Providers and deployment options vary; Sourcegraph also provides non-AI code search. | [Cody documentation](https://sourcegraph.com/docs/cody) |
| 89 | Aider — Aider | `coding_assistant`, `cli_agent`, `router` | CLI | Supports many providers and local endpoints; no fixed provider/domain indicator. | [Aider documentation](https://aider.chat/docs/) |
| 90 | OpenHands — coding agent | `coding_assistant`, `cli_agent`, `self_hosted_stack`, `router` | server, CLI | Self-hosted or cloud deployment and configurable model providers prevent a universal indicator. | [OpenHands repository](https://github.com/OpenHands/openhands) |
| 91 | Block — Goose | `coding_assistant`, `cli_agent`, `desktop_app`, `router` | CLI, desktop | Multi-provider and local deployment support; no fixed endpoint or process signature. | [Goose documentation](https://block.github.io/goose/docs/) |
| 92 | Anomaly / OpenCode — OpenCode | `coding_assistant`, `cli_agent`, `router` | CLI, desktop, IDE | Provider selection and compatible endpoints are user-configurable. | [OpenCode docs](https://opencode.ai/docs/) |
| 93 | Alibaba — Qwen Code | `coding_assistant`, `cli_agent` | CLI | Official project confirms CLI; provider behavior can depend on account/configuration. | [Qwen Code](https://qwen.ai/qwencode) |
| 94 | Replit — Replit Agent | `coding_assistant`, `desktop_app`, `router` | web, cloud | Cloud-hosted development environment; local process detection does not cover remote execution. | [Replit Agent](https://replit.com/ai) |
| 95 | ByteDance — Trae | `coding_assistant`, `ide_assistant`, `desktop_app`, `router` | desktop, IDE | Product identity can be observable as an app, but no signed-package/process indicator is asserted here. | [Trae](https://www.trae.ai/) |

### Browser assistants and browser surfaces (7)

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 96 | Microsoft — Copilot in Edge | `browser_extension`, `hosted_chat`, `search_research` | browser, desktop | Built-in browser feature; no standalone extension ID or exclusive network host asserted. | [Copilot in Edge](https://support.microsoft.com/en-us/microsoft-copilot/getting-started-with-copilot-in-microsoft-edge) |
| 97 | Brave — Leo | `browser_extension`, `hosted_chat` | browser, desktop | Built-in browser assistant; browser build/version and model selection vary. | [Brave Leo](https://brave.com/leo/) |
| 98 | Opera — Aria | `browser_extension`, `hosted_chat`, `multimodal` | browser, desktop | Built into Opera surfaces; browser application control is the relevant local signal, not a unique process. | [Opera Aria](https://www.opera.com/features/aria) |
| 99 | The Browser Company — Arc Max | `browser_extension`, `search_research` | browser, desktop | Browser-integrated features; Arc browser identity does not prove Max feature use. | [Arc Max](https://arc.net/max) |
| 100 | Perplexity — Comet browser | `browser_extension`, `search_research`, `hosted_chat` | browser, desktop | Browser app surface; using browser presence as an AI-use verdict would be a false positive. | [Comet browser](https://www.perplexity.ai/comet) |
| 101 | Monica — AI assistant extension | `browser_extension`, `hosted_chat`, `router` | browser extension, web | Extension can route to multiple providers; exact store/package ID should be collected from official store metadata before registry use. | [Monica product](https://monica.im/) |
| 102 | Sider — AI sidebar / browser extension | `browser_extension`, `hosted_chat`, `router` | browser extension, web | Multi-model product; do not infer model from extension/vendor identity. | [Sider product](https://sider.ai/) |

### AI-assisted cybersecurity and AI-security tools (7)

These records have different rule implications: an AI red-team testing tool can be legitimate security work, and a product that tests AI systems is not itself evidence of prohibited assistant use. Session rules must classify use case, not just product name.

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 103 | PentestGPT | `security_tool`, `cli_agent`, `router` | CLI, web | Repository/product entry exists; project maintenance/status requires re-check. No stable network indicator asserted. | [PentestGPT repository](https://github.com/GreyDGL/PentestGPT) |
| 104 | PortSwigger — Burp AI / Burp AT | `security_tool`, `desktop_app` | desktop | Burp AT is agentic AI for human-led pentesting; no standalone endpoint signature asserted. | [Burp AT docs](https://portswigger.net/burp/documentation/desktop/burp-at) |
| 105 | XBOW — autonomous offensive security | `security_tool`, `cloud_marketplace` | cloud | Cloud service; product source establishes category only, not a workstation indicator. | [XBOW product announcement](https://xbow.com/blog/introducing-xbow) |
| 106 | Alias Robotics — CAI framework (archived) | `security_tool`, `self_hosted_stack`, `router` | CLI, local runtime | Archived project; do not treat as active. It accepts configurable model APIs so no unique provider indicator. | [CAI repository/status](https://github.com/aliasrobotics/cai) |
| 107 | NVIDIA — garak LLM vulnerability scanner | `security_tool`, `cli_agent` | CLI | Local tool may make requests to selected targets; no default vendor endpoint identifies its use. | [garak repository](https://github.com/NVIDIA/garak) |
| 108 | Microsoft — PyRIT | `security_tool`, `cli_agent`, `self_hosted_stack` | Python library, CLI | Red-team framework is provider-configurable; no universal endpoint or process signature. | [PyRIT repository](https://github.com/Azure/PyRIT) |
| 109 | Promptfoo — red-team / eval tooling | `security_tool`, `cli_agent`, `router` | CLI, web | Supports many model providers and target URLs; configured endpoint is not globally identifiable. | [Promptfoo red-team docs](https://www.promptfoo.dev/docs/red-team/quickstart/) |

### Local runtimes, inference servers and local interfaces (18)

| # | Provider / product surface | Registry category | Surface | Indicator assessment | First-party provenance |
| ---: | --- | --- | --- | --- | --- |
| 110 | Ollama — local runtime and API | `local_runner`, `inference_server`, `self_hosted_stack` | local runtime, API | Default `localhost:11434` documented; configurable; detection candidate only. | [Ollama API introduction](https://github.com/ollama/ollama/blob/main/docs/api/introduction.mdx) |
| 111 | ggml-org — llama.cpp / llama-server | `local_runner`, `inference_server` | local runtime, server | `llama-server` default `127.0.0.1:8080` documented; configurable and shared by unrelated apps. | [llama.cpp server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md) |
| 112 | LM Studio — local inference server | `desktop_app`, `local_runner`, `inference_server` | desktop, local runtime, API | Server port is configurable and last-used value may persist; do not assume a fixed port. | [LM Studio server CLI](https://lmstudio.ai/docs/cli/serve/server-start) |
| 113 | LocalAI — local inference server | `local_runner`, `inference_server`, `self_hosted_stack` | server, API | Bind address/port configurable; no fixed process signature is a reliable identity. | [LocalAI docs](https://localai.io/docs/index.html) |
| 114 | vLLM — OpenAI-compatible inference server | `inference_server`, `local_runner`, `self_hosted_stack` | server, API | Deployment port and host are operator-defined; no static default used here. | [vLLM online serving](https://docs.vllm.ai/en/latest/serving/online_serving/openai_compatible_server/) |
| 115 | Hugging Face — Text Generation Inference (TGI) | `inference_server`, `self_hosted_stack` | server, API | Docs show serving on configured port (example 8080); detection candidate, not identity. | [TGI consuming guide](https://huggingface.co/docs/text-generation-inference/basic_tutorials/consuming_tgi) |
| 116 | NVIDIA — TensorRT-LLM | `inference_server`, `local_runner`, `self_hosted_stack` | server, local runtime | Service deployment is operator-configured; no universal process/port indicator. | [TensorRT-LLM documentation](https://nvidia.github.io/TensorRT-LLM/) |
| 117 | LMSYS — SGLang | `inference_server`, `local_runner`, `self_hosted_stack` | server, local runtime | Host/port are deployment settings; no exclusive indicator asserted. | [SGLang documentation](https://docs.sglang.ai/) |
| 118 | Jan — Desktop, Agent and local API | `desktop_app`, `local_runner`, `inference_server`, `router` | desktop, CLI, API | Local server default `127.0.0.1:1337` documented; can route to configured remote providers too. | [Jan local API server](https://docs.jan.ai/desktop/api-server) |
| 119 | Nomic — GPT4All | `desktop_app`, `local_runner`, `inference_server` | desktop, local runtime | Product can use local and remote models; no stable endpoint or process signature asserted. | [GPT4All docs](https://docs.gpt4all.io/) |
| 120 | Mozilla — llamafile | `local_runner`, `inference_server` | local runtime, server | Single-file distribution can be renamed; no reliable executable basename. | [llamafile repository](https://github.com/Mozilla-Ocho/llamafile) |
| 121 | oobabooga — text-generation-webui | `local_runner`, `inference_server`, `local_web_ui` | local runtime, server, local web UI | API/UI ports and bind settings vary; no fixed default asserted in this ledger. | [text-generation-webui repository](https://github.com/oobabooga/text-generation-webui) |
| 122 | LostRuins — KoboldCpp | `local_runner`, `inference_server`, `local_web_ui` | local runtime, server | Portable binary and configurable ports make names/ports weak, bypassable hints. | [KoboldCpp repository](https://github.com/LostRuins/koboldcpp) |
| 123 | Open WebUI — self-hosted model interface | `local_web_ui`, `self_hosted_stack`, `router` | local web UI, server | Container publish port can differ from app port; configurable provider connections. | [Open WebUI quick start](https://docs.openwebui.com/getting-started/quick-start/) |
| 124 | Mintplex Labs — AnythingLLM | `local_web_ui`, `self_hosted_stack`, `router` | desktop, local web UI, server | Supports local/remote workspaces and providers; deployment endpoint is user-selected. | [AnythingLLM documentation](https://docs.anythingllm.com/) |
| 125 | LibreChat — self-hosted assistant UI | `local_web_ui`, `self_hosted_stack`, `router` | web, server | Multi-provider setup and self-hosting mean hostname/port are installation-specific. | [LibreChat docs](https://www.librechat.ai/docs) |
| 126 | LobeHub — LobeChat | `local_web_ui`, `self_hosted_stack`, `router` | web, desktop, server | Supports configurable provider APIs and deployments; no single indicator. | [LobeChat documentation](https://lobehub.com/docs) |
| 127 | Apple MLX — mlx-lm | `local_runner`, `inference_server` | local runtime, Python | Library can be embedded in arbitrary processes; process/domain/port indicators are not reliable. | [mlx-lm repository](https://github.com/ml-explore/mlx-lm) |

## Category and architecture implications

1. **Hosted service names are discovery metadata, not proof of use.** A hostname can justify a clearly disclosed block rule, but server-side feature paths are encrypted, and the same domain may host account, help, static, or non-AI functionality. Product host indicators should be exact-host by default; suffix matching requires a DNS-label boundary and a false-positive review.
2. **API catalogs do not enumerate where an API can be called from.** A source SDK is only one route. Any HTTP client can call the provider; OpenAI-compatible services can share client libraries, and gateways/proxies can replace the API base URL.
3. **Provider and product are separate entities.** Cursor, Cline, Continue, Roo Code, Kilo, Venice, Poe, Open WebUI, and other surfaces can route to one or many providers. Model selection can be dynamic; a `routes_to` relationship should be explicitly configurable, non-exhaustive, and never treated as proof of the selected backend.
4. **Package IDs and executable names are only local clues.** A process can be renamed, copied, rebuilt, or run remotely. Do not add a package/process indicator without signed-publisher or official package metadata and a false-positive analysis. This research artifact intentionally omits most such indicators.
5. **Local inference evades network-only registries.** Loopback ports, model files, containers, WSL/VMs, and embedded libraries are separate detection surfaces. Model formats such as GGUF do not prove a runtime is active or that a particular model was used.
6. **AI security tools need use-case classification.** PentestGPT, Burp AT, XBOW, garak, PyRIT, and promptfoo are included because they demonstrate AI-assisted security workflows or AI-system testing. Whether they violate rules depends on the agreed competition policy; blanket domain/app blocking can incorrectly prohibit normal security tooling.

## Important blind spots

- Unknown/new providers, resellers, regional product names, and short-lived launches.
- User-defined API base URLs, API-compatible servers, self-hosted gateways, reverse proxies, DNS aliases, CDN/shared cloud hosts, VPNs, tunnels, Tor, raw IPs, DNS-over-HTTPS/TLS, and encrypted traffic.
- Local models embedded in desktop/IDE apps, scripts, notebooks, browser WASM, GPU runtimes, containers, WSL, VMs, or remote machines.
- Browser extension sideloading, multiple profiles, mobile apps, remote browsers, SSH/RDP, and second physical devices.
- Product pages and sources may rename or disappear. A stale record must become `stale`/review-required in a later schema; removing a page does not prove a product is retired.
- The 127 count is a researched representative sample, not market coverage, completeness, or a detection-rate claim.

## Recommended follow-up to this artifact

Keep both registry snapshots as research data, separate from enforcement code. Before promoting any row into enforcement data, a reviewer should confirm the current official source, validate the indicator directly, assess false positives, check DNS ownership/redirects, and record an explicit scope. A future schema revision may add `evidence_type`, `indicator_scope`, `last_reviewed_by`, `review_due`, and `retired_reason`; do not silently extend either version or make registry contents drive Milestone 1 protocol behavior.
