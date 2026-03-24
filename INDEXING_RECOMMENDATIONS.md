# Recommended Repositories for Indexing

This document outlines the public repositories recommended for indexing to support the development, customization, and regulatory compliance of the **dbus-enterprise-2026** ecosystem.

## 1. Core System & Native Integration (The "How it Works" Layer)
*Indexing these ensures agents write idiomatic, high-performance, and "native-first" Rust code.*

| Repository | Focus Area | Why Index? |
| :--- | :--- | :--- |
| **[zbus-rs/zbus](https://github.com/zbus-rs/zbus)** | Native D-Bus | The backbone for all native D-Bus communication without CLI wrappers. |
| **[rust-netlink/rtnetlink](https://github.com/rust-netlink/rtnetlink)** | Network Ops | Critical for replacing NetworkManager with direct kernel netlink calls. |
| **[firecracker-microvm/firecracker](https://github.com/firecracker-microvm/firecracker)** | Rust Security | AWS's reference for secure, minimalist, native-first Rust system programming. |
| **[tokio-rs/tokio](https://github.com/tokio-rs/tokio)** | Async Runtime | Ensures the `op-workflows` and `op-gateway` handle async scheduling correctly. |

## 2. Schema-as-Code & JSON Mapping (The "Source of Truth" Layer)
*Indexing these supports the automation of Rust-type-to-JSON-Schema synchronization.*

| Repository | Focus Area | Why Index? |
| :--- | :--- | :--- |
| **[oxidecomputer/typify](https://github.com/oxidecomputer/typify)** | Rust Generation | Generates idiomatic Rust types from JSON schemas for your `tunable` configs. |
| **[GREATARENA/schemars](https://github.com/GREATARENA/schemars)** | Schema Export | Generates JSON schemas from your Rust structs to power the `web-ui` and API. |
| **[cloudflare/serde-json-path](https://github.com/cloudflare/serde-json-path)** | Data Selection | Essential for your `privacy_index` to mask PII and immutable paths. |
| **[jsontypedef/json-typedef-spec](https://github.com/jsontypedef/json-typedef-spec)** | Deterministic Schema | Reference for strict, unambiguous data contracts used in regulatory audits. |

## 3. Compliance & Regulatory Enforcement (The "Governance" Layer)
*Indexing these provides the "ground truth" for your ComplianceProfile and PolicyEngine.*

| Repository | Focus Area | Why Index? |
| :--- | :--- | :--- |
| **[ComplianceAsCode/content](https://github.com/ComplianceAsCode/content)** | Security Guides | Provides the industry-standard SCAP/CIS benchmarks to map to D-Bus state. |
| **[open-policy-agent/opa](https://github.com/open-policy-agent/opa)** | Policy as Code | Helps standardize the logic in your `PolicyEngine` against enterprise norms. |
| **[cloud-custodian/cloud-custodian](https://github.com/cloud-custodian/cloud-custodian)** | Governance | A reference for "Filter and Action" logic used in automated remediation. |
| **[microsoft/presidio](https://github.com/microsoft/presidio)** | Privacy/PII | Informs your `redaction` logic for HIPAA, GDPR, and SOC2 compliance. |

## 4. Enterprise Infrastructure & Identity (The "Replacement" Layer)
*Indexing these helps agents migrate legacy enterprise systems to OP-DBUS.*

| Repository | Focus Area | Why Index? |
| :--- | :--- | :--- |
| **[FreeIPA/freeipa](https://github.com/freeipa/freeipa)** | Identity Management | Reference for replacing LDAP/Active Directory with `org.opdbus.directory`. |
| **[systemd/systemd](https://github.com/systemd/systemd)** | D-Bus Surface | Allows agents to understand and migrate the 16,000+ D-Bus tools you've indexed. |
| **[linkedin/datahub](https://github.com/datahub-project/datahub)** | Metadata Catalog | Standard for managing large-scale metadata, useful for the `op-introspection` indexer. |
| **[confluentinc/schema-registry](https://github.com/confluentinc/schema-registry)** | Versioning | Patterns for managing schema evolution and compatibility in your `Event Chain`. |

## 5. Orchestration & Protocol (The "Intelligence" Layer)
*Indexing these ensures your agents and MCP servers remain compliant with standard protocols.*

| Repository | Focus Area | Why Index? |
| :--- | :--- | :--- |
| **[modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers)** | MCP Examples | Provides structural patterns for your `70+ specialized agents`. |
| **[modelcontextprotocol/specification](https://github.com/modelcontextprotocol/specification)** | Protocol Spec | Ensures your `op-mcp` bridge remains compliant with future Anthropics updates. |
| **[temporalio/temporal](https://github.com/temporalio/temporal)** | Auditable Workflows | Patterns for the "Event Chain" strategy used for regulatory proof-of-change. |

## Recommended First Steps for Indexing:
1.  **zbus-rs/zbus**: Immediate impact on core D-Bus discovery.
2.  **oxidecomputer/typify**: Automates the Schema-as-Code pipeline.
3.  **ComplianceAsCode/content**: Populates your ComplianceProfile with real-world regulatory controls.
4.  **systemd/systemd**: Provides the raw material for your D-Bus tool registry.
