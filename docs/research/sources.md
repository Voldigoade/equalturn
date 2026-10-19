# Research source index

**Research snapshot date:** 19 October 2026. Sources below were consulted during Phase 0 and the Milestone 1 dependency review; access dates do not imply that a page will remain unchanged. Product and library facts are volatile and must be rechecked before upgrades. Sources support the linked topic, not every inference made in the handoff.

The documents separate **verified fact** (a capability or product claim stated by a source), **inference** (synthesis across sources or observed product scope), **proposed design** (a project decision not yet implemented), and **unresolved risk** (a question the evidence does not settle). Search absence is never treated as proof that a product or collision does not exist.

## Operating-system and network controls

### Windows

- [Windows Filtering Platform overview](https://learn.microsoft.com/en-us/windows/win32/fwp/about-windows-filtering-platform), [ALE](https://learn.microsoft.com/en-us/windows/win32/fwp/application-layer-enforcement--ale-), and [WFP access control](https://learn.microsoft.com/en-us/windows/win32/fwp/access-control)
- [Windows Firewall configuration](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/configure) and [New-NetFirewallRule reference](https://learn.microsoft.com/en-us/powershell/module/netsecurity/new-netfirewallrule?view=windowsserver2025-ps)
- [App Control for Business and AppLocker overview](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/appcontrol-and-applocker-overview), [signed policies and tampering](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/deployment/use-signed-policies-to-protect-appcontrol-against-tampering), [feature availability](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/feature-availability), and [AppLocker Application Identity service](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/applocker/configure-the-application-identity-service)
- [Process enumeration](https://learn.microsoft.com/en-us/windows/win32/procthread/process-enumeration), [ETW overview](https://learn.microsoft.com/en-us/windows/win32/etw/about-event-tracing), and [ETW session control](https://learn.microsoft.com/en-us/windows/win32/etw/controlling-event-tracing-sessions)
- [Windows service security/access rights](https://learn.microsoft.com/en-us/windows/win32/services/service-security-and-access-rights)
- [Windows DNS-over-HTTPS client support](https://learn.microsoft.com/en-us/windows-server/networking/dns/doh-client-support), [Edge DNS-over-HTTPS policy](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/dnsoverhttpsmode), and [Edge built-in DNS client policy](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/builtinDNSClientEnabled)
- [Zero Trust DNS](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/zero-trust-dns/) and [application control for packaged apps](https://learn.microsoft.com/en-us/windows/security/threat-protection/windows-defender-application-control/manage-packaged-apps-with-windows-defender-application-control)
- [WSL networking](https://learn.microsoft.com/en-us/windows/wsl/networking) and [Hyper-V Firewall](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/hyper-v-firewall)

### Linux and protocol standards

- [nftables manual](https://www.netfilter.org/projects/nftables/manpage.html) and [`CAP_NET_ADMIN` capability](https://www.man7.org/linux/man-pages/man7/capabilities.7.html)
- [Linux kernel self-protection threat model](https://cdn.kernel.org/doc/html/latest/process/threat-model.html), [cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html), and [proc filesystem documentation](https://docs.kernel.org/filesystems/proc.html)
- [systemd service units](https://www.freedesktop.org/software/systemd/man/256/systemd.service.html), [systemd kill behavior](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html), and [systemd execution environment](https://www.freedesktop.org/software/systemd/man/260/systemd.exec.html)
- [BPF program types](https://docs.kernel.org/bpf/programs.html), [BPF verifier](https://kernel.org/doc/html/latest/bpf/verifier.html), and [BPF kernel functions](https://docs.kernel.org/bpf/kfuncs.html)
- [DNS over HTTPS, RFC 8484](https://www.rfc-editor.org/info/rfc8484/) and [DNS over TLS, RFC 7858](https://www.rfc-editor.org/info/rfc7858/)
- [Wayland protocol documentation](https://wayland.freedesktop.org/docs/book/Protocol.html), [X Window System communication overview](https://www.x.org/guide/communication/), [X security discussion](https://www.x.org/wiki/Development/Documentation/Security/), and [Linux network namespace reference (man-pages 6.16)](https://www.kernel.org/pub/linux/docs/man-pages/book/man-pages-6.16.pdf)

## Stack and protocol

- Rust [ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html), [Send and Sync](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html), and [target support](https://doc.rust-lang.org/rustc/targets/index.html)
- Microsoft [windows-rs crate guidance](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows.md) and [binding metadata issue history](https://github.com/microsoft/windows-rs/issues/4867); [`windows-service` crate](https://docs.rs/windows-service/latest/windows_service/)
- Go [GC guide](https://go.dev/doc/gc-guide), [Effective Go](https://go.dev/doc/effective_go), [cgo](https://go.dev/cmd/cgo/), and [Windows service package](https://github.com/golang/sys/blob/master/windows/svc/service.go)
- C++ [resource lifetime guidance](https://learn.microsoft.com/en-us/cpp/cpp/object-lifetime-and-resource-management-modern-cpp) and [draft object model](https://eel.is/c%2B%2Bdraft/basic); [libnftnl](https://netfilter.org/projects/libnftnl/index.html); [Qt system tray icon](https://doc.qt.io/qt-6.11/qsystemtrayicon.html)
- Zig [0.17.0 language documentation](https://ziglang.org/documentation/0.17.0/), [platform support](https://ziglang.org/learn/platform-support/), and [0.17.0 release](https://ziglang.org/news/0.17.0-released/)
- Windows [WFP architecture](https://learn.microsoft.com/en-us/windows/win32/fwp/windows-filtering-platform-architecture-overview) and [Windows services](https://learn.microsoft.com/en-us/windows/win32/services); systemd [service manager documentation](https://www.freedesktop.org/software/systemd/man/257/systemd.service.html)
- [TUF specification v1.0.36](https://theupdateframework.github.io/specification/v1.0.36/) and [TUF overview](https://theupdateframework.org/spec/), consulted for established signed-metadata and rollback/freeze-protection design
- [Ed25519, RFC 8032](https://www.rfc-editor.org/info/rfc8032/), [JSON Canonicalization Scheme, RFC 8785](https://www.rfc-editor.org/info/rfc8785/), and [WebSocket, RFC 6455](https://www.rfc-editor.org/info/rfc6455/)
- Current implementation API references: [Axum 0.8.9 WebSocket upgrade and limits](https://docs.rs/axum/0.8.9/axum/extract/struct.WebSocketUpgrade.html), [SQLx 0.9.0 transactions](https://docs.rs/sqlx/0.9.0/sqlx/struct.Transaction.html) and [migrations](https://docs.rs/sqlx/0.9.0/sqlx/migrate/index.html), and [ed25519-dalek 3.0.0 strict verification](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.VerifyingKey.html)
- [RustSec cargo-audit documentation](https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md), checked 19 October 2026. `cargo-audit` 0.22.2 was run against the committed lockfile and the current RustSec advisory database on that date.
- Context7 queries on 19 October 2026 checked Axum WebSocket frame/message limits, SQLx transactions/migrations, and `ed25519-dalek` strict verification API semantics. They corroborated API use; they did not replace repository implementation tests or the upstream sources linked above.
- Context7 queries on 19 October 2026: Tauri v2 tray-icon feature guidance using `/tauri-apps/plugins-workspace` and `/tauri-apps/tauri-docs`; Microsoft's Windows bindings guidance using `/microsoft/windows-rs`. These confirmed documentation direction only; no integration was built.

## AI ecosystem and registry

- [python-jsonschema 4.26.0 validation documentation](https://python-jsonschema.readthedocs.io/en/v4.26.0/validate/) and its [Draft 2020-12 validator API](https://python-jsonschema.readthedocs.io/en/v4.26.0/api/jsonschema/validators/); the format checker is enabled because JSON Schema `format` is otherwise an annotation. Release/maintenance status checked against the [upstream changelog](https://github.com/python-jsonschema/jsonschema/blob/main/CHANGELOG.rst) on 19 October 2026.

### Provider/API and gateway documentation

- [OpenAI API reference](https://platform.openai.com/docs/api-reference/introduction)
- [Anthropic API overview](https://platform.claude.com/docs/en/api/overview) and [Claude Code CLI usage](https://docs.anthropic.com/en/docs/claude-code/cli-usage)
- [Gemini API key documentation](https://ai.google.dev/gemini-api/docs/api-key)
- [xAI API documentation](https://docs.x.ai/)
- [DeepSeek API documentation](https://api-docs.deepseek.com/guides/responses_api/)
- [Mistral API first request](https://docs.mistral.ai/getting-started/quickstarts/developer/first-api-request) and [Le Chat product](https://mistral.ai/products/le-chat)
- [Hugging Face Inference Providers](https://huggingface.co/docs/inference-providers/en/index) and [provider routing/function-calling guide](https://huggingface.co/docs/inference-providers/en/guides/function-calling)
- [OpenRouter model API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [LiteLLM documentation](https://docs.litellm.ai/docs/)
- [Amazon Bedrock Marketplace](https://docs.aws.amazon.com/bedrock/latest/userguide/amazon-bedrock-marketplace.html) and [Azure AI Foundry model catalog](https://learn.microsoft.com/en-us/azure/machine-learning/concept-model-catalog?view=azureml-api-2)
- [Perplexity](https://www.perplexity.ai/) and [Poe](https://poe.com/) product sites

### Developer tools and local/self-hosted inference

- [GitHub Copilot product surfaces](https://docs.github.com/en/copilot/get-started/where-to-use-github-copilot) and [Copilot coding agents](https://docs.github.com/en/copilot/how-tos/copilot-in-your-ide/use-copilot-agents)
- [Cursor BYOK/provider configuration](https://prod.cursor.com/help/models-and-usage/api-keys)
- [Cline repository](https://github.com/Cline/Cline) and [provider authorization documentation](https://github.com/cline/cline/blob/main/docs/getting-started/authorizing-with-cline.mdx)
- [Gemini CLI repository and documentation](https://github.com/google-gemini/gemini-cli)
- [OpenHands repository](https://github.com/OpenHands/openhands)
- [Ollama API](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [LM Studio local server](https://lmstudio.ai/docs/developer/core/server)
- [vLLM OpenAI-compatible server](https://docs.vllm.ai/en/latest/serving/online_serving/openai_compatible_server/)
- [LocalAI documentation](https://localai.io/docs/index.html)
- [Open WebUI provider connection](https://docs.openwebui.com/getting-started/quick-start/connect-a-provider/)
- [PentestGPT repository](https://github.com/GreyDGL/PentestGPT) and [Burp DAST AI documentation](https://portswigger.net/burp/documentation/dast/user-guide/using-ai)

## Prior art and ecosystem directories

- [Safe Exam Browser overview](https://safeexambrowser.org/about_overview_en.html) and [Windows user manual](https://safeexambrowser.org/windows/win_usermanual_v2.4.1_en.html)
- [Respondus LockDown Browser and extensions](https://web.respondus.com/ldb-vs-extensions/) and [application conflict support note](https://support.respondus.com/hc/en-us/articles/4409607172507-When-I-start-Respondus-LockDown-Browser-a-warning-message-states-You-must-close-the-following-program-before-starting-the-browser-Why-does-this-occur)
- [CTFd](https://ctfd.io/v1/) and [CTFd scoring overview](https://docs.ctfd.io/docs/scoring/overview/)
- [CTFd AI Cheater Detector repository](https://github.com/7h30th3r0n3/ctfd-ai-cheater-detector)
- [OpenLock Linux exam-browser research repository](https://github.com/oneKn8/openlock_browser)
- Microsoft [generative AI app governance](https://github.com/MicrosoftDocs/microsoft-365-docs/blob/public/copilot/manage-generative-ai-apps.md)
- [Netskope generative AI security](https://www.netskope.com/products/securing-generative-ai) and [access policies](https://docs.netskope.com/en/access-policies)
- [DNSFilter content categories](https://help.dnsfilter.com/hc/en-us/articles/29593839153171-Filtering-Policy-content-categories) and [content filtering](https://www.dnsfilter.com/content-filtering)
- [Freedom](https://freedom.to/) and [Cold Turkey guide](https://getcoldturkey.com/support/user-guide/)
- Microsoft [App Control for Business](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/) and [AppLocker overview](https://learn.microsoft.com/en-au/windows/security/application-security/application-control/windows-defender-application-control/applocker/applocker-overview)

## Naming screen

- Final Equalturn screen, checked 19 October 2026: GitHub repository connector queries `Equalturn` and `equalturn in:name` returned no repositories; see the public [GitHub repository search](https://github.com/search?q=Equalturn&type=repositories). Search absence is limited by indexing and repository visibility.
- Exact package pages: [crates.io: `equalturn`](https://crates.io/crates/equalturn) displayed “Crate not found”; [npm: `equalturn`](https://www.npmjs.com/package/equalturn) displayed “package not found”; [PyPI: `equalturn`](https://pypi.org/project/equalturn/) returned 404.
- [EUIPO eSearch](https://euipo.europa.eu/eSearch/#basic/1+1+1+1/100+100+100+100/Equalturn), exact query `Equalturn`: zero results in trade marks, designs, owners, and representatives. This is an exact-word query, not similarity analysis.
- [USPTO Trademark Search](https://tmsearch.uspto.gov/), Wordmark query `Equalturn`: “No results found,” zero live and dead results, checked 19 October 2026. The UI exposes no stable public permalink for a query. USPTO explains that clearance must also consider similar marks, related goods/services, and broader resources in its [clearance-search guidance](https://www.uspto.gov/trademarks/search/comprehensive-clearance-search-similar-trademarks).
- [WIPO Global Brand Database](https://branddb.wipo.int/branddb/en/) was opened but not searched: it presented an anti-bot challenge. No result or clearance inference is made from WIPO.
- General web queries: `"Equalturn" app OR software OR company`, `"Equalturn" cybersecurity OR security`, `"Equalturn" trademark OR brand`, and `"Equalturn" product`; Product Hunt- and SourceForge-scoped search-engine queries also surfaced no matching product. Results were noisy and are discovery-only evidence, not exhaustive index searches.
- Earlier Phase 0 exact GitHub and USPTO checks for the alternative names: [Rulepair](https://github.com/search?q=Rulepair&type=repositories) and [Equalterms](https://github.com/search?q=Equalterms&type=repositories); USPTO UI had returned no exact Wordmark results for those terms. These checks did not establish legal availability.
- Collision examples that removed candidates: [Scrutineer security scanner](https://github.com/alpha-omega-security/scrutineer), [Evenhand project](https://github.com/brod-dev/evenhand), [Goodfaith PyPI package](https://pypi.org/project/goodfaith/), [Groundrule](https://groundrule.dev/), [Fairmark](https://github.com/shfrlabs/Fairmark), [Pactline coaching](https://www.getpactline.com/coaching), [Scrimmage robotics project](https://github.com/gtri/scrimmage), [Fairterms project](https://github.com/isaaceng7/FairTerms), [Fairturn app](https://play.google.com/store/apps/details?id=com.netforza.fairturn)
- Additional visible same-name/type usages considered: [Lugha.Common RulePair package](https://www.nuget.org/packages/Lugha.Common/0.2.0) and [pkg.go.dev RulePair identifier](https://pkg.go.dev/github.com/openzro/openzro/client/internal/ingressgw)

## Scope and limits

Primary OS/vendor documentation was prioritized for capability claims. Project repositories and first-party docs were used to establish product surfaces. Product catalogs and search results were discovery aids, not independent proof. No patent search or exhaustive trademark clearance was performed. EUIPO and USPTO exact-word queries were run; WIPO was blocked by an anti-bot challenge, no separate TMview review was performed, and domain availability was not checked. Source availability and the open-source review cannot establish completeness or non-existence.
