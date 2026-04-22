#!/usr/bin/env python3
"""
Compliance Corpora → Qdrant ingestion pipeline.

Ingestion priority (§7.3 compliance-corpora-knowledge.md):
  1 [Critical] controls-mapping.json           — cross-framework Rosetta Stone
  2 [Critical] eu-ai-act-layer-v3.6.0-lite.json
  3 [High]     GDPR-Developer-Guide/*.md        — section-level chunks
  4 [High]     compl-ai/                        — benchmark × principle
  5 [High]     techops/template/                — section-level chunks
  6 [Medium]   rego-cns/**/*.rego               — one chunk per policy
  7 [Medium]   security-policy-templates/standards/*.json
  OSCAL        catalog / component-definition JSON anywhere in corpus tree

Canonical rule_id scheme (never changes across re-ingestion):
  sec-pol:{procedure_id}                 e.g. sec-pol:cp-access-standards
  eu-ai-act:{section}:{slug}             e.g. eu-ai-act:prohibited-practices:pp-01
  gdpr:{sheet:02d}:{section_slug}        e.g. gdpr:01:anonymisation-of-personal-data
  nist-800-53:{prov}:{control}:{policy}  e.g. nist-800-53:aws-cf:sc-28:cloudtrail-encryption
  compl-ai:{benchmark_name}              e.g. compl-ai:mmlu-pro
  techops:{level}:{section_slug}         e.g. techops:application:human-oversight
  oscal:{model_id}:{control_id}          e.g. oscal:nist800-53:ac-3

IDs are hashed → deterministic UUIDs so upsert is idempotent.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import uuid
from pathlib import Path
from typing import Any

from qdrant_client import QdrantClient
from qdrant_client.models import Distance, PointStruct, VectorParams
from tqdm import tqdm
import voyageai

# ── Config ────────────────────────────────────────────────────────────────────
CORPORA_ROOT = Path(os.environ.get("CORPORA_ROOT", "/home/jeremy/git/compliance_corpora"))
QDRANT_URL   = os.environ.get("QDRANT_URL", "http://localhost:6333")
QDRANT_KEY   = os.environ.get("QDRANT_API_KEY", "")
VOYAGE_KEY   = os.environ.get("VOYAGE_API_KEY", "")
VOYAGE_MODEL = os.environ.get("VOYAGE_MODEL", "voyage-large-2")
COLLECTION   = os.environ.get("QDRANT_COLLECTION", "compliance_rules")
VECTOR_DIM   = int(os.environ.get("VECTOR_DIM", "1536"))
BATCH        = int(os.environ.get("EMBED_BATCH", "8"))

# ── NIST 800-53 control-family canonical metadata ─────────────────────────────
_NIST_CF: dict[str, dict] = {
    "AC":  {"name": "Access Control",                      "category": "access-control",  "severity": "high"},
    "AU":  {"name": "Audit and Accountability",             "category": "governance",       "severity": "medium"},
    "CM":  {"name": "Configuration Management",             "category": "security",         "severity": "medium"},
    "CP":  {"name": "Contingency Planning",                 "category": "governance",       "severity": "medium"},
    "IA":  {"name": "Identification and Authentication",    "category": "access-control",  "severity": "high"},
    "IR":  {"name": "Incident Response",                    "category": "governance",       "severity": "high"},
    "MA":  {"name": "Maintenance",                          "category": "security",         "severity": "medium"},
    "MP":  {"name": "Media Protection",                     "category": "security",         "severity": "medium"},
    "PE":  {"name": "Physical and Environmental Protection","category": "security",         "severity": "medium"},
    "PL":  {"name": "Planning",                             "category": "governance",       "severity": "low"},
    "PS":  {"name": "Personnel Security",                   "category": "governance",       "severity": "medium"},
    "RA":  {"name": "Risk Assessment",                      "category": "risk-management",  "severity": "high"},
    "SA":  {"name": "System and Services Acquisition",      "category": "governance",       "severity": "medium"},
    "SC":  {"name": "System and Communications Protection", "category": "security",         "severity": "high"},
    "SI":  {"name": "System and Information Integrity",     "category": "security",         "severity": "high"},
}

# Rego filename stem → canonical NIST 800-53 control (§4 of knowledge doc)
_REGO_CTRL: dict[str, str] = {
    # AWS CloudFormation
    "api_gwcache_encrypted":                                "SC",
    "cloudtrail_encryption_enabled":                        "SC-28",
    "cloudwatch_loggroup_encrypted":                        "SC-28",
    "dynamodb_table_encrypted_using_kms":                   "SC-28",
    "ebs_volume_delete_on_termination":                     "MP",
    "ec2_ebs_encryption_bydefault":                         "SC-28",
    "ec2_instance_nopublic_ip":                             "SC-7",
    "efs_encrypted":                                        "SC-28",
    "elasticsearch_encrypted_atrest":                       "SC-28",
    "elb_deletion_protection_enabled":                      "CP",
    "iam_passwordpolicy_maxpassword_age":                   "IA-5",
    "iam_passwordpolicy_minimum_passwordlength":            "IA-5",
    "iam_passwordpolicy_password_reuse_prevention":         "IA-5",
    "iam_passwordpolicy_require_lowercasecharacters":       "IA-5",
    "iam_passwordpolicy_require_numbers":                   "IA-5",
    "iam_passwordpolicy_require_symbols":                   "IA-5",
    "iam_passwordpolicy_require_uppercasecharacters":       "IA-5",
    "maxaccesskeyage":                                      "IA-5",
    "mfa_enabled_iamconsole_access":                        "IA-2",
    "rds_instance_public_accesscheck":                      "SC-7",
    "redshift_cluster_public_accesscheck":                  "SC-7",
    "rotation_customer_created_cmks_enabled":               "SC-12",
    "s3bucket_level_public_access_prohibited":              "AC-3",
    "s3bucket_level_public_access_prohibited_singlebucket": "AC-3",
    "sagemaker_notebook_instance_kmskeyconfigured":         "SC-28",
    "security_group_restricted_ssh":                        "AC-17",
    # AWS Terraform
    "api_gw_cache_enabled_and_encrypted":                   "SC",
    "api_gw_execution_logging_enabled":                     "AU",
    "api_gw_require_authentication":                        "IA",
    "api_gw_tls":                                           "SC",
    "api_gw_xray_enabled":                                  "AU",
    "ebs_default_encrypt":                                  "SC-28",
    "ec2_imdsv2":                                           "IA-2",
    "ec2_user_data_no_secrets":                             "IA-5",
    "eks_encrypt_secrets":                                  "SC-28",
    "eks_no_public_cluster":                                "SC-7",
    "iam_no_wildcards_policies":                            "AC-6",
    "no_static_credentials_in_providers":                   "IA-5",
    "s3_bucket_level_public_access_prohibited":             "AC-3",
    "s3_bucket_logging_enabled":                            "AU-2",
    "s3_bucket_public_read_and_write_prohibited":           "AC-3",
    "s3_bucket_server_side_encryption_rule":                "SC-28",
    "s3_bucket_versioning_enabled":                         "CP-9",
    # GCP Terraform
    "bigquery_no_public_access":                            "AC-3",
    "check_google_container_cluster_node_config":           "CM-6",
    "compute_disk_encryption_customer_key":                 "SC-28",
    "compute_disk_encryption_required":                     "SC-28",
    "compute_enable_shielded_vm":                           "SI-7",
    "compute_enable_vpc_flow_logs":                         "AU-2",
    "compute_no_default_service_account":                   "AC-6",
    "compute_no_ip_forwarding":                             "SC-7",
    "compute_no_plaintext_vm_disk_keys":                    "SC-28",
    "compute_no_public_ip":                                 "SC-7",
    "dns_enable_dnssec":                                    "SC-20",
    "dns_key_check":                                        "SC-12",
    "gke_enable_auto_repair":                               "SI-2",
    "gke_enable_auto_upgrade":                              "SI-2",
    "gke_enable_ip_aliasing":                               "SC-7",
    "gke_enable_master_networks":                           "SC-7",
    "gke_enable_network_policy":                            "SC-7",
    "gke_enable_private_cluster":                           "SC-7",
    "gke_enable_stackdriver_logging":                       "AU-2",
    "gke_enable_stackdriver_monitoring":                    "AU-6",
    "gke_metadata_endpoints_disabled":                      "CM-6",
    "gke_no_basic_authentication":                          "IA-2",
    "gke_no_client_cert_authentication":                    "IA-2",
    "gke_node_metadata_security":                           "CM-6",
    "gke_node_pool_uses_cos":                               "CM-6",
    "gke_node_shielding_enabled":                           "SI-7",
    "gke_no_public_control_plane":                          "SC-7",
    "gke_use_cluster_labels":                               "CM-8",
    "gke_use_rbac_permissions":                             "AC-3",
    "iam_no_folder_level_default_service_account_assignment": "AC-6",
    "iam_no_folder_level_service_account_impersonation":    "AC-6",
    "iam_no_privileged_service_accounts":                   "AC-6",
    "storage_enable_ubla":                                  "AC-3",
    "storage_no_public_access":                             "AC-3",
}

# compl-ai benchmark → EU AI Act technical requirement + principle (§2.3)
_COMPL_AI_BENCH: dict[str, dict] = {
    "aime_2025":             {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "arc_challenge":         {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "gpqa_diamond":          {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "hle":                   {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "ifbench":               {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "include":               {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "livebench_coding":      {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "mmlu_pro":              {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "swe_bench_verified":    {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "mmmu_pro":              {"req": "Capabilities, Performance, and Limitations", "principle": "Human Agency and Oversight"},
    "bbq":                   {"req": "Representation — Absence of Bias",           "principle": "Diversity, Non-Discrimination, Fairness"},
    "bold":                  {"req": "Representation — Absence of Bias",           "principle": "Diversity, Non-Discrimination, Fairness"},
    "cab":                   {"req": "Representation — Absence of Bias",           "principle": "Diversity, Non-Discrimination, Fairness"},
    "bigbench_calibration":  {"req": "Interpretability",                           "principle": "Transparency"},
    "triviaqa_calibration":  {"req": "Interpretability",                           "principle": "Transparency"},
    "boolq_contrast":        {"req": "Robustness and Predictability",              "principle": "Technical Robustness and Safety"},
    "forecast_consistency":  {"req": "Robustness and Predictability",              "principle": "Technical Robustness and Safety"},
    "imdb_contrast":         {"req": "Robustness and Predictability",              "principle": "Technical Robustness and Safety"},
    "mmlu_pro_robustness":   {"req": "Robustness and Predictability",              "principle": "Technical Robustness and Safety"},
    "self_check_consistency":{"req": "Robustness and Predictability",              "principle": "Technical Robustness and Safety"},
    "decoding_trust":        {"req": "Fairness — Absence of Discrimination",       "principle": "Diversity, Non-Discrimination, Fairness"},
    "fairllm":               {"req": "Fairness — Absence of Discrimination",       "principle": "Diversity, Non-Discrimination, Fairness"},
    "human_deception":       {"req": "Disclosure of AI",                           "principle": "Transparency"},
    "instruction_goal_hijacking": {"req": "Cyberattack Resilience",               "principle": "Technical Robustness and Safety"},
    "llm_rules":             {"req": "Cyberattack Resilience",                     "principle": "Technical Robustness and Safety"},
    "strong_reject":         {"req": "Cyberattack Resilience",                     "principle": "Technical Robustness and Safety"},
    "mask":                  {"req": "Societal Alignment",                         "principle": "Societal and Environmental Well-being"},
    "simpleqa_verified":     {"req": "Societal Alignment",                         "principle": "Societal and Environmental Well-being"},
    "truthfulqa":            {"req": "Societal Alignment",                         "principle": "Societal and Environmental Well-being"},
    "realtoxicityprompts":   {"req": "Harmful Content and Toxicity",               "principle": "Societal and Environmental Well-being"},
}

# ── Helpers ───────────────────────────────────────────────────────────────────

def _slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")[:60]

def _uid(rule_id: str) -> str:
    return str(uuid.UUID(hashlib.md5(rule_id.encode()).hexdigest()))

def _cf_from_control(ctrl: str) -> str:
    return ctrl.split("-")[0].upper()

def _nist_meta(ctrl: str) -> dict:
    cf = _cf_from_control(ctrl)
    return _NIST_CF.get(cf, {"name": cf, "category": "security", "severity": "medium"})

def _framework_refs(implements: list[dict]) -> list[dict]:
    return [{"name": impl["standard"], "references": impl["requirements"]} for impl in implements]

# ── Priority 1: controls-mapping.json (Rosetta Stone) ─────────────────────────

def load_controls_index() -> dict[str, list[dict]]:
    """Returns {procedure_id: [framework_ref, ...]} for cross-ref enrichment."""
    path = CORPORA_ROOT / "security-policy-templates/templates/standards/controls-mapping.json"
    data = json.loads(path.read_text())
    return {p["id"]: _framework_refs(p.get("implements", [])) for p in data["procedures"]}

def extract_sec_pol(controls_index: dict) -> list[dict]:
    """Priority 1: vectorise each procedure as a canonical cross-framework chunk."""
    mapping_path = CORPORA_ROOT / "security-policy-templates/templates/standards/controls-mapping.json"
    proc_dir = CORPORA_ROOT / "security-policy-templates/templates/procedures"
    chunks = []
    for proc_id, fw_refs in controls_index.items():
        frameworks_text = "; ".join(
            f"{r['name']}: {', '.join(r['references'])}" for r in fw_refs
        )
        text = (
            f"Security procedure: {proc_id}. "
            f"Cross-framework mappings — {frameworks_text}. "
            f"This procedure implements requirements across: "
            f"{', '.join(r['name'] for r in fw_refs)}."
        )
        # Enrich with template body if file exists
        candidates = list(proc_dir.glob(f"{proc_id}.md.tmpl")) if proc_dir.exists() else []
        if candidates:
            body = candidates[0].read_text(errors="replace")[:3000]
            text += f"\n\n{body}"

        rule_id = f"sec-pol:{proc_id}"
        chunks.append({
            "rule_id":            rule_id,
            "text":               text,
            "source_corpus":      "security-policy-templates",
            "frameworks":         fw_refs,
            "category":           _infer_category_from_proc(proc_id),
            "subcategory":        proc_id,
            "description":        f"Security procedure {proc_id}",
            "validation_type":    "documentation-review",
            "validation_method":  {"type": "checklist", "specification": {}},
            "severity":           "medium",
            "applicability":      {"system_types": ["any"], "cloud_providers": ["any"], "data_types": ["any"]},
            "owner_role":         "CISO",
            "source_file":        str(mapping_path),
            "ingest_priority":    1,
        })
    return chunks

def _infer_category_from_proc(proc_id: str) -> str:
    p = proc_id.lower()
    if "data" in p:     return "data-protection"
    if "access" in p:   return "access-control"
    if "risk" in p:     return "risk-management"
    if "ir" in p or "breach" in p: return "governance"
    if "sdlc" in p or "vuln" in p: return "security"
    if "privacy" in p:  return "data-protection"
    return "governance"

# ── Priority 2: EU AI Act JSON ────────────────────────────────────────────────

def extract_eu_ai_act() -> list[dict]:
    path = CORPORA_ROOT / "eu-ai-act-layer-lite/eu-ai-act-layer-v3.6.0-lite.json"
    data = json.loads(path.read_text())
    chunks: list[dict] = []

    base_fw = [{"name": "EU AI Act", "references": ["Art. 5", "Art. 6", "Art. 9", "Art. 13", "Art. 14", "Annex III"]}]

    # Core principles
    for i, principle in enumerate(data.get("core_principles", []), 1):
        rule_id = f"eu-ai-act:core-principles:cp-{i:02d}"
        chunks.append(_eu_chunk(rule_id, f"EU AI Act Core Principle {i}: {principle}",
            principle, base_fw, "governance", "core-principles", "high",
            ["documentation-review"], path))

    # Prohibited practices (Article 5) — canonical PP-01 through PP-05
    pp_checks = {
        "PP-01": "No subliminal or manipulative techniques",
        "PP-02": "No exploitation of vulnerable groups",
        "PP-03": "No social scoring by public authorities",
        "PP-04": "No emotion recognition in workplace or educational institutions",
        "PP-05": "No behavior distortion causing harm",
    }
    for pp_id, statement in pp_checks.items():
        rule_id = f"eu-ai-act:prohibited-practices:{pp_id.lower()}"
        fw = [{"name": "EU AI Act", "references": ["Art. 5", f"Art. 5 {pp_id}"]}]
        chunks.append(_eu_chunk(rule_id,
            f"EU AI Act Prohibited Practice {pp_id}: {statement}",
            statement, fw, "governance", "prohibited-practices", "critical",
            ["manual-audit"], path))

    # Annex III risk categories
    annex_cats = data.get("normative_scope", {}).get("annex_iii_categories", [])
    for cat in annex_cats:
        rule_id = f"eu-ai-act:annex-iii:{_slug(cat)}"
        fw = [{"name": "EU AI Act", "references": ["Annex III", f"Annex III {cat}"]}]
        chunks.append(_eu_chunk(rule_id,
            f"EU AI Act Annex III High-Risk Category: {cat.replace('_', ' ').title()}",
            f"High-risk AI system category: {cat}", fw,
            "risk-management", "annex-iii", "high", ["documentation-review"], path))

    # Required artifacts
    for artifact in data.get("artifacts", {}).get("required", []):
        rule_id = f"eu-ai-act:required-artifact:{_slug(artifact)}"
        fw = [{"name": "EU AI Act", "references": ["Art. 11", "Annex IV"]}]
        desc = f"Required EU AI Act governance artifact: {artifact}"
        chunks.append(_eu_chunk(rule_id, desc, desc, fw,
            "documentation", "required-artifact", "high",
            ["documentation-review"], path))

    # Capabilities / included features
    for cap in data.get("capabilities", {}).get("included", []):
        rule_id = f"eu-ai-act:capability:{_slug(cap)}"
        fw = [{"name": "EU AI Act", "references": ["Art. 11"]}]
        desc = f"EU AI Act Layer Lite capability: {cap.replace('_', ' ')}"
        chunks.append(_eu_chunk(rule_id, desc, desc, fw,
            "governance", "capability", "medium", ["documentation-review"], path))

    return chunks

def _eu_chunk(rule_id, text, desc, fw, category, subcategory, severity, vtypes, path) -> dict:
    return {
        "rule_id":           rule_id,
        "text":              text,
        "source_corpus":     "eu-ai-act-layer",
        "frameworks":        fw,
        "category":          category,
        "subcategory":       subcategory,
        "description":       desc,
        "validation_type":   vtypes[0],
        "validation_method": {"type": "checklist", "specification": {}},
        "severity":          severity,
        "applicability":     {
            "system_types":    ["high-risk-ai", "general-purpose-ai"],
            "cloud_providers": ["any"],
            "data_types":      ["any"],
        },
        "owner_role":        "Legal",
        "source_file":       str(path),
        "ingest_priority":   2,
    }

# ── Priority 3: GDPR Developer Guide ─────────────────────────────────────────

def extract_gdpr() -> list[dict]:
    gdpr_dir = CORPORA_ROOT / "GDPR-Developer-Guide"
    chunks: list[dict] = []

    for md_file in sorted(gdpr_dir.glob("[0-9][0-9]*.md")):
        sheet_num = int(md_file.name[:2])
        text = md_file.read_text(errors="replace")

        # Split by ## headings into sections
        sections = re.split(r"\n(?=#{1,3} )", text)
        for sec in sections:
            lines = sec.strip().splitlines()
            if not lines:
                continue
            heading_line = lines[0].lstrip("#").strip()
            body = "\n".join(lines[1:]).strip()
            if not body or len(body) < 40:
                continue

            section_slug = _slug(heading_line)
            rule_id = f"gdpr:{sheet_num:02d}:{section_slug}"

            # Infer GDPR articles from content
            articles = _gdpr_articles_from_sheet(sheet_num)
            fw = [{"name": "GDPR", "references": articles}]

            embed_text = (
                f"GDPR Developer Guide — Sheet {sheet_num:02d}: {heading_line}\n\n{body[:1200]}"
            )
            chunks.append({
                "rule_id":           rule_id,
                "text":              embed_text,
                "source_corpus":     "gdpr-guide",
                "frameworks":        fw,
                "category":          _gdpr_category(sheet_num),
                "subcategory":       section_slug,
                "description":       f"GDPR Sheet {sheet_num:02d} — {heading_line}",
                "technical_requirement": body[:500],
                "validation_type":   "documentation-review",
                "validation_method": {"type": "checklist", "specification": {"sheet": sheet_num}},
                "severity":          "high" if sheet_num in {0,1,6,7,8,12,13} else "medium",
                "applicability":     {
                    "system_types":    ["web-application", "cloud-infrastructure", "high-risk-ai"],
                    "cloud_providers": ["any"],
                    "data_types":      ["personal-data"],
                },
                "owner_role":        "DPO",
                "source_file":       str(md_file),
                "ingest_priority":   3,
            })
    return chunks

def _gdpr_articles_from_sheet(n: int) -> list[str]:
    mapping = {
        0:  ["Art. 5", "Art. 24", "Art. 37"],
        1:  ["Art. 4", "Art. 9"],
        2:  ["Art. 25", "Art. 35"],
        3:  ["Art. 32"],
        4:  ["Art. 32", "Art. 25"],
        5:  ["Art. 44", "Art. 46"],
        6:  ["Art. 32"],
        7:  ["Art. 5(1)(c)", "Art. 25"],
        8:  ["Art. 5(1)(f)", "Art. 32"],
        9:  ["Art. 28", "Art. 32"],
        10: ["Art. 5(1)(f)", "Art. 32"],
        11: ["Art. 32", "Art. 35"],
        12: ["Art. 13", "Art. 14", "Art. 33", "Art. 34"],
        13: ["Art. 15", "Art. 16", "Art. 17", "Art. 18", "Art. 20", "Art. 21"],
        14: ["Art. 5(1)(e)", "Art. 17"],
        15: ["Art. 6", "Art. 7"],
        16: ["Art. 5(1)(c)", "Art. 6", "Art. 82"],
    }
    return mapping.get(n, ["Art. 5", "Art. 32"])

def _gdpr_category(n: int) -> str:
    cats = {
        0: "governance", 1: "data-protection", 2: "data-protection",
        3: "security", 4: "security", 5: "governance",
        6: "security", 7: "data-protection", 8: "access-control",
        9: "security", 10: "governance", 11: "security",
        12: "transparency", 13: "data-protection", 14: "data-protection",
        15: "governance", 16: "data-protection",
    }
    return cats.get(n, "governance")

# ── Priority 4: compl-ai benchmarks ──────────────────────────────────────────

def extract_compl_ai() -> list[dict]:
    compl_dir = CORPORA_ROOT / "compl-ai"
    chunks: list[dict] = []

    # Try to load config YAML for descriptions
    bench_descs: dict[str, str] = {}
    for yaml_file in compl_dir.rglob("*.yaml"):
        try:
            text = yaml_file.read_text(errors="replace")
            for line in text.splitlines():
                m = re.match(r"^(\w+):\s*#?\s*(.+)", line)
                if m:
                    bench_descs.setdefault(m.group(1), m.group(2))
        except Exception:
            pass

    for bench, meta in _COMPL_AI_BENCH.items():
        rule_id = f"compl-ai:{bench}"
        fw = [{"name": "EU AI Act", "references": [meta["principle"], meta["req"]]}]
        desc = bench_descs.get(bench, bench.replace("_", " ").title())
        text = (
            f"COMPL-AI Benchmark: {bench}. "
            f"EU AI Act Technical Requirement: {meta['req']}. "
            f"EU AI Act Principle: {meta['principle']}. "
            f"Description: {desc}. "
            f"Use this benchmark to evaluate LLM compliance with the EU AI Act principle "
            f"of {meta['principle']}."
        )
        chunks.append({
            "rule_id":           rule_id,
            "text":              text,
            "source_corpus":     "compl-ai",
            "frameworks":        fw,
            "category":          _compl_ai_category(meta["principle"]),
            "subcategory":       meta["req"],
            "description":       f"LLM benchmark for {meta['req']} ({meta['principle']})",
            "validation_type":   "benchmark-evaluation",
            "validation_method": {"type": "llm-benchmark", "specification": {"benchmark": bench}},
            "severity":          "high",
            "applicability":     {
                "system_types":    ["general-purpose-ai", "high-risk-ai"],
                "cloud_providers": ["any"],
                "data_types":      ["any"],
            },
            "owner_role":        "ML Engineer",
            "source_file":       str(compl_dir),
            "ingest_priority":   4,
        })
    return chunks

def _compl_ai_category(principle: str) -> str:
    p = principle.lower()
    if "oversight" in p:    return "governance"
    if "robustness" in p:   return "security"
    if "privacy" in p:      return "data-protection"
    if "transparency" in p: return "transparency"
    if "fairness" in p or "discrimination" in p: return "fairness"
    if "societal" in p:     return "governance"
    return "governance"

# ── Priority 5: TechOps documentation templates ───────────────────────────────

def extract_techops() -> list[dict]:
    tmpl_dir = CORPORA_ROOT / "techops/template"
    chunks: list[dict] = []

    level_map = {
        "application documentation.md": ("application", "AI System Provider", ["Art. 5", "Art. 6", "Art. 9", "Art. 13", "Art. 14", "Annex IV"]),
        "model documentation.md":       ("model",       "Model Developer",    ["Art. 11", "Art. 13", "Annex IV"]),
        "data documentation.md":        ("data",        "Data Owner",         ["Art. 10", "Art. 11", "Art. 13", "Annex IV"]),
    }

    for filename, (level, owner, articles) in level_map.items():
        path = tmpl_dir / filename
        if not path.exists():
            continue
        text = path.read_text(errors="replace")
        # Split by ## headings
        sections = re.split(r"\n(?=## )", text)
        for sec in sections:
            lines = sec.strip().splitlines()
            if not lines:
                continue
            heading = lines[0].lstrip("#").strip()
            body = "\n".join(lines[1:]).strip()
            if not body or len(body) < 40:
                continue

            section_slug = _slug(heading)
            rule_id = f"techops:{level}:{section_slug}"

            # Map section heading to specific EU AI Act article
            specific_arts = _techops_article_map(level, heading)
            sec_fw = [{"name": "EU AI Act", "references": specific_arts or articles}]

            embed_text = (
                f"TechOps EU AI Act {level.title()} Documentation — {heading}\n\n"
                f"{body[:1200]}"
            )
            chunks.append({
                "rule_id":           rule_id,
                "text":              embed_text,
                "source_corpus":     "techops",
                "frameworks":        sec_fw,
                "category":          "documentation",
                "subcategory":       section_slug,
                "description":       f"TechOps {level} template section: {heading}",
                "validation_type":   "documentation-review",
                "validation_method": {"type": "checklist", "specification": {"level": level}},
                "severity":          "high",
                "applicability":     {
                    "system_types":    ["high-risk-ai"],
                    "cloud_providers": ["any"],
                    "data_types":      ["any"],
                },
                "owner_role":        owner,
                "source_file":       str(path),
                "ingest_priority":   5,
            })
    return chunks

def _techops_article_map(_level: str, heading: str) -> list[str]:
    h = heading.lower()
    if "risk" in h and "classification" in h:     return ["Art. 5", "Art. 6", "Art. 7"]
    if "human oversight" in h:                    return ["Art. 14"]
    if "transparency" in h:                       return ["Art. 13"]
    if "data governance" in h or "data quality" in h: return ["Art. 10"]
    if "logging" in h or "traceability" in h:     return ["Art. 12"]
    if "conformity" in h:                         return ["Art. 43"]
    if "risk management" in h:                    return ["Art. 9"]
    if "accuracy" in h or "robustness" in h:      return ["Art. 15"]
    if "lifecycle" in h or "monitoring" in h:     return ["Art. 11", "Annex IV §6"]
    if "fairness" in h or "bias" in h:            return ["Art. 10"]
    return []

# ── Priority 6: NIST 800-53 Rego Policies ────────────────────────────────────

def extract_rego() -> list[dict]:
    rego_root = CORPORA_ROOT / "rego-cns"
    chunks: list[dict] = []

    provider_map = {
        "aws_cloudformation_nist_800_53": ("aws", "aws-cf",  "AWS CloudFormation"),
        "aws_terraform_nist_800_53":      ("aws", "aws-tf",  "AWS Terraform"),
        "gcp_terraform_nist_800_53":      ("gcp", "gcp-tf",  "GCP Terraform"),
    }

    for subdir, (cloud, prov_slug, prov_name) in provider_map.items():
        policy_dir = rego_root / subdir
        if not policy_dir.exists():
            continue
        for rego_file in sorted(policy_dir.glob("*.rego")):
            stem = rego_file.stem
            ctrl = _REGO_CTRL.get(stem, "")
            if not ctrl:
                ctrl = "NIST-UNKNOWN"

            cf   = _cf_from_control(ctrl)
            meta = _nist_meta(ctrl)

            rule_id = f"nist-800-53:{prov_slug}:{ctrl.lower().replace('-', '')}:{_slug(stem)}"
            content = rego_file.read_text(errors="replace")

            # Extract doc URL from deny_message
            doc_url_m = re.search(r'msg\s*:=\s*"(https?://[^"]+)"', content)
            doc_url = doc_url_m.group(1) if doc_url_m else ""

            # Extract resource type check
            resource_type_m = re.search(r'(?:Type|resource_type)\s*==\s*"([^"]+)"', content)
            resource_type = resource_type_m.group(1) if resource_type_m else ""

            fw = [{"name": "NIST 800-53", "references": [ctrl, cf]}]

            embed_text = (
                f"NIST 800-53 Rego Policy [{ctrl}] — {prov_name}: {stem}. "
                f"Control family: {meta['name']}. "
                f"Resource type: {resource_type}. "
                f"Policy logic:\n{content}"
            )
            chunks.append({
                "rule_id":           rule_id,
                "text":              embed_text,
                "source_corpus":     "rego-nist",
                "frameworks":        fw,
                "category":          meta["category"],
                "subcategory":       ctrl,
                "description":       f"NIST 800-53 {ctrl} — {prov_name} — {stem}",
                "technical_requirement": f"IaC policy validation: {stem}",
                "validation_type":   "static-analysis",
                "validation_method": {
                    "type": "rego-policy",
                    "specification": {
                        "provider":      prov_name,
                        "resource_type": resource_type,
                        "control":       ctrl,
                        "doc_url":       doc_url,
                        "policy_file":   stem + ".rego",
                    },
                },
                "severity":          meta["severity"],
                "applicability":     {
                    "system_types":    ["cloud-infrastructure"],
                    "cloud_providers": [cloud],
                    "data_types":      ["any"],
                },
                "owner_role":        "DevOps",
                "source_file":       str(rego_file),
                "ingest_priority":   6,
            })
    return chunks

# ── Priority 7: Framework JSON files ─────────────────────────────────────────

def extract_framework_jsons() -> list[dict]:
    standards_dir = CORPORA_ROOT / "security-policy-templates/templates/standards"
    chunks: list[dict] = []
    skip = {"controls-mapping.json", "scf.json", "sunstone-merged-moderate.json"}  # too large / handled elsewhere

    for jf in sorted(standards_dir.glob("*.json")):
        if jf.name in skip:
            continue
        try:
            data = json.loads(jf.read_text())
        except Exception:
            continue

        framework_name = jf.stem.replace("-", " ").replace("_", " ").title()
        controls = _flatten_framework_controls(data)

        for ctrl in controls[:200]:  # cap per file
            rule_id = f"framework:{_slug(jf.stem)}:{_slug(ctrl['id'])}"
            fw_ref = _infer_framework_from_filename(jf.stem)
            fw = [{"name": fw_ref, "references": [ctrl["id"]]}]
            text = f"{framework_name} Control {ctrl['id']}: {ctrl['title']}. {ctrl.get('description', '')}"
            chunks.append({
                "rule_id":           rule_id,
                "text":              text[:2000],
                "source_corpus":     "security-policy-templates",
                "frameworks":        fw,
                "category":          "security",
                "subcategory":       ctrl["id"],
                "description":       f"{framework_name} — {ctrl['id']}: {ctrl['title']}",
                "validation_type":   "documentation-review",
                "validation_method": {"type": "checklist", "specification": {}},
                "severity":          "medium",
                "applicability":     {"system_types": ["any"], "cloud_providers": ["any"], "data_types": ["any"]},
                "owner_role":        "CISO",
                "source_file":       str(jf),
                "ingest_priority":   7,
            })
    return chunks

def _flatten_framework_controls(data: Any, depth: int = 0) -> list[dict]:
    """Walk arbitrary JSON structures to find control-like objects."""
    results = []
    if depth > 6:
        return results
    if isinstance(data, list):
        for item in data:
            results.extend(_flatten_framework_controls(item, depth + 1))
    elif isinstance(data, dict):
        # Support multiple key conventions across frameworks
        cid   = (data.get("id") or data.get("ref") or data.get("control_id")
                 or data.get("identifier") or data.get("control-id") or "")
        title = (data.get("title") or data.get("name") or data.get("statement") or "")
        desc  = (data.get("description") or data.get("summary") or data.get("guidance")
                 or data.get("discussion") or data.get("prose") or "")
        if cid and title:
            results.append({"id": str(cid), "title": str(title), "description": str(desc)[:400]})
        for v in data.values():
            results.extend(_flatten_framework_controls(v, depth + 1))
    return results

def _infer_framework_from_filename(stem: str) -> str:
    s = stem.lower()
    if "hipaa" in s:       return "HIPAA"
    if "iso-iec-27001" in s or "iso_iec_27001" in s: return "ISO/IEC 27001"
    if "iso-27002" in s or "iso_27002" in s:          return "ISO/IEC 27002"
    if "csa" in s:         return "CSA CCM"
    if "pci" in s:         return "PCI DSS"
    if "cmmc" in s:        return "CMMC"
    if "gdpr" in s:        return "GDPR"
    return stem.replace("-", " ").title()

# ── OSCAL: catalog / component-definition chunking ───────────────────────────

def extract_oscal() -> list[dict]:
    """
    Scans compliance_corpora tree (and ../OSCAL/src if present) for OSCAL JSON.
    Chunks by control for catalogs, by implemented-requirement for components.
    """
    chunks: list[dict] = []
    search_roots = [
        CORPORA_ROOT,
        CORPORA_ROOT.parent / "OSCAL" / "src",
        CORPORA_ROOT.parent / "oscal-content",
        CORPORA_ROOT.parent / "fedramp-automation",
    ]

    for root in search_roots:
        if not root.exists():
            continue
        for jf in root.rglob("*.json"):
            if jf.stem.endswith("-min"):
                continue
            try:
                raw = jf.read_text(errors="replace")
                if "oscal_version" not in raw and "oscal-version" not in raw:
                    continue
                data = json.loads(raw)
            except Exception:
                continue

            if "catalog" in data:
                chunks.extend(_oscal_catalog(data["catalog"], jf))
            elif "component-definition" in data:
                chunks.extend(_oscal_component_def(data["component-definition"], jf))

    return chunks

def _oscal_catalog(cat: dict, path: Path) -> list[dict]:
    model_id = _slug(cat.get("metadata", {}).get("title", path.stem))
    chunks = []
    for group in cat.get("groups", []):
        for ctrl in group.get("controls", []):
            chunks.extend(_oscal_control_chunk(ctrl, model_id, path))
    for ctrl in cat.get("controls", []):
        chunks.extend(_oscal_control_chunk(ctrl, model_id, path))
    return chunks

def _oscal_control_chunk(ctrl: dict, model_id: str, path: Path) -> list[dict]:
    ctrl_id = ctrl.get("id", "unknown")
    title   = ctrl.get("title", "")
    # Flatten prose parts
    parts_text = " ".join(
        p.get("prose", "") for p in ctrl.get("parts", []) if p.get("prose")
    )
    rule_id = f"oscal:{model_id}:{ctrl_id}"
    fw_name = "NIST 800-53" if "nist" in model_id.lower() else model_id.upper()
    fw = [{"name": fw_name, "references": [ctrl_id.upper()]}]

    text = f"OSCAL Catalog Control [{ctrl_id.upper()}]: {title}. {parts_text}"
    ctrl_cf = ctrl_id.split("-")[0].upper() if "-" in ctrl_id else ctrl_id[:2].upper()
    meta = _NIST_CF.get(ctrl_cf, {"category": "security", "severity": "medium"})

    return [{
        "rule_id":           rule_id,
        "text":              text[:2000],
        "source_corpus":     "oscal",
        "frameworks":        fw,
        "category":          meta["category"],
        "subcategory":       ctrl_id,
        "description":       f"OSCAL {model_id} — {ctrl_id.upper()}: {title}",
        "validation_type":   "static-analysis",
        "validation_method": {"type": "rego-policy", "specification": {"oscal_model": model_id}},
        "severity":          meta["severity"],
        "applicability":     {"system_types": ["cloud-infrastructure"], "cloud_providers": ["any"], "data_types": ["any"]},
        "owner_role":        "DevOps",
        "source_file":       str(path),
        "ingest_priority":   6,
    }]

def _oscal_component_def(comp_def: dict, path: Path) -> list[dict]:
    chunks = []
    model_id = _slug(comp_def.get("metadata", {}).get("title", path.stem))
    for comp in comp_def.get("components", []):
        comp_title = comp.get("title", "")
        for ctrl_impl in comp.get("control-implementations", []):
            source = ctrl_impl.get("source", "")
            for req in ctrl_impl.get("implemented-requirements", []):
                ctrl_id = req.get("control-id", "unknown")
                prose = " ".join(
                    s.get("prose", "") for s in req.get("statements", []) if s.get("prose")
                )
                rule_id = f"oscal:{model_id}:{_slug(comp_title)}:{ctrl_id}"
                fw = [{"name": "NIST 800-53", "references": [ctrl_id.upper()]}]
                text = (
                    f"OSCAL Component Implementation [{ctrl_id.upper()}] "
                    f"in component '{comp_title}': {prose}"
                )
                ctrl_cf = ctrl_id.split("-")[0].upper() if "-" in ctrl_id else ctrl_id[:2].upper()
                meta = _NIST_CF.get(ctrl_cf, {"category": "security", "severity": "medium"})
                chunks.append({
                    "rule_id":           rule_id,
                    "text":              text[:2000],
                    "source_corpus":     "oscal",
                    "frameworks":        fw,
                    "category":          meta["category"],
                    "subcategory":       ctrl_id,
                    "description":       f"OSCAL {comp_title} implements {ctrl_id.upper()}",
                    "validation_type":   "static-analysis",
                    "validation_method": {"type": "rego-policy", "specification": {"oscal_source": source}},
                    "severity":          meta["severity"],
                    "applicability":     {"system_types": ["cloud-infrastructure"], "cloud_providers": ["any"], "data_types": ["any"]},
                    "owner_role":        "DevOps",
                    "source_file":       str(path),
                    "ingest_priority":   6,
                })
    return chunks

# ── Embedding + Qdrant upsert ─────────────────────────────────────────────────

def embed_and_upsert(chunks: list[dict], vc: voyageai.Client, qc: QdrantClient) -> None:
    texts = [c["text"] for c in chunks]
    points: list[PointStruct] = []

    for i in tqdm(range(0, len(texts), BATCH), desc="embedding"):
        batch_texts = texts[i:i+BATCH]
        result = vc.embed(batch_texts, model=VOYAGE_MODEL, input_type="document")
        for j, vec in enumerate(result.embeddings):
            chunk = chunks[i+j]
            payload = {k: v for k, v in chunk.items() if k != "text"}
            points.append(PointStruct(
                id=_uid(chunk["rule_id"]),
                vector=vec,
                payload=payload,
            ))

    for i in tqdm(range(0, len(points), 100), desc="upserting"):
        qc.upsert(collection_name=COLLECTION, points=points[i:i+100])

# ── Main ─────────────────────────────────────────────────────────────────────

def main() -> None:
    if not VOYAGE_KEY:
        print("ERROR: set VOYAGE_API_KEY", flush=True)
        raise SystemExit(1)

    vc = voyageai.Client(api_key=VOYAGE_KEY)
    qc = QdrantClient(url=QDRANT_URL, api_key=QDRANT_KEY or None)

    # Ensure collection exists
    existing = {c.name for c in qc.get_collections().collections}
    if COLLECTION not in existing:
        qc.create_collection(
            collection_name=COLLECTION,
            vectors_config=VectorParams(size=VECTOR_DIM, distance=Distance.COSINE),
        )
        print(f"Created collection '{COLLECTION}' (dim={VECTOR_DIM})")
    else:
        print(f"Collection '{COLLECTION}' exists — upserting")

    # Load cross-framework index first (Priority 1 enriches all other corpora)
    print("\n[1/7] Loading controls-mapping Rosetta Stone …")
    controls_index = load_controls_index()
    print(f"  {len(controls_index)} procedures indexed")

    extractors = [
        ("1-Critical  controls-mapping",   lambda: extract_sec_pol(controls_index)),
        ("2-Critical  eu-ai-act",          extract_eu_ai_act),
        ("3-High      gdpr-guide",         extract_gdpr),
        ("4-High      compl-ai",           extract_compl_ai),
        ("5-High      techops",            extract_techops),
        ("6-Medium    rego-nist-800-53",   extract_rego),
        ("7-Medium    framework-jsons",    extract_framework_jsons),
        ("OSCAL       oscal-catalogs",     extract_oscal),
    ]

    total = 0
    for label, fn in extractors:
        print(f"\n[{label}] extracting …")
        try:
            chunks = fn()
        except Exception as e:
            print(f"  SKIP — {e}")
            continue
        if not chunks:
            print("  0 chunks — skipping")
            continue
        print(f"  {len(chunks)} chunks → embedding + upsert")
        embed_and_upsert(chunks, vc, qc)
        total += len(chunks)

    print(f"\nDone. {total} chunks upserted to '{COLLECTION}' @ {QDRANT_URL}")


if __name__ == "__main__":
    main()
