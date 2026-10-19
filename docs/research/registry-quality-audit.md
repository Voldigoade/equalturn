# Registry v2 quality audit

**Reviewed:** 19 October 2026
**Scope:** `registry/registry-v2.json`, its provenance records, and its 58 structured indicators. This is a research-data review; it does not authorize enforcement.

## Method and limits

The audit parsed every entry, source, relationship, and indicator in the v2 JSON. It checked identifier uniqueness, provider/product pair duplication, duplicate source URLs, entry-local source references, HTTP(S) URL shape, source check dates, status and category distributions, and duplicate `(kind, normalized value)` indicators. Every indicator and its source-specific assessment were enumerated for review. First-party pages were rechecked for the changed or weak claims listed below, including Mistral's Vibe rename, the separate Vibe Code surface, MiniMax's Agent/Code split, PentestGPT's current repository state, the CAI archive status, Cursor's CLI name, Kilo's npm package, and TGI's example port.

The static URL check does not prove that all 132 pages were reachable or that each page's content remains unchanged. This was not a whole-catalog uptime crawl. The date on each source is a research check date, not an automatic freshness guarantee. A registry review must revisit links and claims before publication; a future validator can check syntax and provenance linkage but cannot prove factual relevance.

## Snapshot after audit

| Measure | Result |
| --- | ---: |
| Registry entries | 129 |
| Entry-local provenance records | 132 |
| Structured indicators | 58 |
| Entries without a structured indicator | 72 |
| Source records not referenced by an indicator | 75 |
| Statuses | 127 observed, 1 unknown, 1 archived |
| Duplicate entry IDs, provider/product pairs, source URLs, or `(kind, value)` indicators | 0 |
| Indicators with unresolved entry-local source IDs | 0 |
| Non-HTTP(S) or malformed absolute source URLs in the structural scan | 0 |

The two distinct source labels are `source-1` and `source-2`; they remain local to each entry. Reuse across entries is intentional and does not make them global identifiers. Sources without indicator references are often the only evidence for a product or surface whose endpoint, package, or process identity is not stable enough to assert. They are not silently treated as unused or as evidence for an indicator.

Indicator kinds after removing the generic TGI port are 30 exact hostnames, 21 API hostnames, 2 URL paths, 3 local listeners, 1 process name, and 1 package name. The snapshot is not a blacklist and these counts do not imply equivalent enforcement value.

## Corrections made

| Record or value | Audit finding | Disposition |
| --- | --- | --- |
| Mistral / Le Chat | Mistral announced that Le Chat is now Vibe; the product family also includes a distinct Vibe Code CLI/IDE/web surface. | Kept the established `mistral-le-chat` key for Vibe Chat/Work, clarified its name and host limitations, and added a separate `mistral-vibe-code` research record with no unverified process or package indicator. The official [rename notice](https://help.mistral.ai/en/articles/682992-le-chat-is-now-vibe) says the chat surface retains `chat.mistral.ai`; [Vibe Code documentation](https://docs.mistral.ai/vibe/code/overview) documents CLI and IDE surfaces. |
| MiniMax Agent / MiniMax Code | A single record combined a hosted agent/chat surface with a separate desktop/CLI coding product. | Split the record into `minimax-agent` and `minimax-code`. The Agent record retains the evidenced `agent.minimax.io` host; the Code record has no structured indicator because no stable package or process identity was established. |
| PentestGPT | `unknown` reflected a stale “needs re-check” note. The current [official repository](https://github.com/GreyDGL/PentestGPT) describes an AI-powered autonomous pentesting/CTF agent and current CLI/provider integrations. | Changed status to `observed`, which in schema v2 means the product is documented, not that future availability or maintenance is guaranteed. No endpoint or process indicator was added. |
| Text Generation Inference (TGI), port `8080` | The cited docs use 8080 in launch/consumer examples; it is configurable and is a ubiquitous port, not a TGI identity. | Removed the `tcp_listener=8080` indicator. Kept the source only as product/API evidence and stated why the example port is not an indicator. |
| Gamma and Canva Magic Studio | Both were tagged `hosted_chat`, although the [Gamma presentation product](https://gamma.app/products/presentations) and [Canva Magic Studio](https://www.canva.com/newsroom/news/magic-studio/) describe broader creation/design surfaces and the domains do not isolate an AI chat feature. | Removed the `hosted_chat` category. Kept their host values only as research observations with explicit broad-scope/high false-positive limitations; the future actionability model must classify those hosts `unsafe_to_block`. |
| Cursor Agent CLI | The old source page concerned API-key settings, not the executable name. | Changed provenance to the official [Cursor CLI overview](https://docs.cursor.com/en/cli/overview), which documents the `cursor-agent` command. The process name remains a weak, renameable detection clue. |
| Kilo Code CLI package | The package claim requires a product-specific install source. | Retained `@kilocode/cli` and its direct [Kilo CLI documentation](https://kilo.ai/docs/code-with-ai/platforms/cli), which documents that package. Package presence alone does not prove model use. |
| Alias Robotics CAI | The [official repository](https://github.com/aliasrobotics/cai) says it was archived on 28 August 2026 and is read-only. | Retained `archived`; no indicator was added. The product's security-testing use is not itself a cheating or maliciousness signal. |

The original MiniMax v2 key was changed to `minimax-agent` while the data is still research-only and has no application consumer. This key correction and the two new records are recorded here for any later v2-to-v3 migration. The dataset semantic version advanced from `0.2.0` to `0.2.1`; schema version remains 2.

## Findings retained for schema v3

| Pattern | Current examples | Required v3 treatment |
| --- | --- | --- |
| Shared product host | `canva.com`, `gamma.app`, `you.com`; the Hugging Face host also serves model and repository content. | Preserve as research evidence; mark broad/shared host indicators `unsafe_to_block` and encode the false-positive risk. |
| Shared API host or task surface | Fireworks and Replicate expose multiple APIs/tasks; gateway hosts route across providers. | Keep exact source scope and limitations; do not infer a particular model, prompt, or prohibited use from destination alone. |
| Configurable local listeners | Ollama `localhost:11434`, llama.cpp `127.0.0.1:8080`, Jan `127.0.0.1:1337`. | Detection-only candidates at most. They are configurable, can be occupied by unrelated processes, and can be hidden by containers or namespaces; never promote a port-only match to a block rule. |
| Renameable local identity | `cursor-agent`; package `@kilocode/cli`. | Advisory/detection evidence only; process names can be renamed and package names do not prove execution or model use. |
| Configurable/cloud deployments | Vertex AI, Azure AI Foundry/Azure OpenAI, Bedrock, self-hosted gateways and OpenAI-compatible endpoints. | Keep records without invented global hosts. Preserve region, account, deployment, and operator-defined endpoint variability. |
| Research/security products | PentestGPT, Burp AI/AT, garak, PyRIT, Promptfoo and CAI. | Keep use-case context. Presence of a red-team or AI-security tool is not a judgment about intent or rule compliance. |
| Bundled service families | Azure AI Foundry/Azure OpenAI and Bedrock Runtime/Marketplace are broader platform records, not single exclusive client products. | Keep them explicitly as platform families until independently sourced records can separate their services without losing deployment context. |

The v2 source review found no duplicate exact source URLs or unresolved source references. It did not find evidence that the remaining structured values are exclusive signals of AI activity. Several are only useful as research or candidate observations; v3 must make that distinction typed and machine-checkable.

## Research references checked

- [Mistral Help: Le Chat is now Vibe](https://help.mistral.ai/en/articles/682992-le-chat-is-now-vibe)
- [Mistral Vibe Code overview](https://docs.mistral.ai/vibe/code/overview)
- [PentestGPT repository](https://github.com/GreyDGL/PentestGPT)
- [Alias Robotics CAI repository/archive](https://github.com/aliasrobotics/cai)
- [Cursor CLI overview](https://docs.cursor.com/en/cli/overview)
- [Kilo Code CLI documentation](https://kilo.ai/docs/code-with-ai/platforms/cli)
- [Hugging Face TGI consuming guide](https://huggingface.co/docs/text-generation-inference/basic_tutorials/consuming_tgi)
- [Gamma AI presentation product](https://gamma.app/products/presentations)
- [Canva Magic Studio overview](https://www.canva.com/newsroom/news/magic-studio/)
