This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-introspection/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-introspection/
              src/
                cache.rs
                cpu_features.rs
                hierarchical.rs
                indexer_manager.rs
                indexer.rs
                lib.rs
                mod.rs
                parser.rs
                projection.rs
                scanner.rs
              Cargo.toml
              compare-op-introspection.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/cache.rs">
//! Introspection caching

use op_core::{BusType, ObjectInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type CacheKey = (BusType, String, String);
type CacheMap = HashMap<CacheKey, ObjectInfo>;

pub struct IntrospectionCache {
    cache: Arc<RwLock<CacheMap>>,
}

impl IntrospectionCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, bus: BusType, service: &str, path: &str) -> Option<ObjectInfo> {
        let cache = self.cache.read().await;
        cache
            .get(&(bus, service.to_string(), path.to_string()))
            .cloned()
    }

    pub async fn set(&self, bus: BusType, service: &str, path: &str, info: ObjectInfo) {
        let mut cache = self.cache.write().await;
        cache.insert((bus, service.to_string(), path.to_string()), info);
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Default for IntrospectionCache {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/cpu_features.rs">
// CPU feature detection and BIOS lock analysis
// Detects hidden/disabled CPU features that could be unlocked

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

/// Type alias for complex feature check results
type FeatureCheckResult = Result<Option<(CpuFeature, Option<BiosLock>, Option<Recommendation>)>>;

/// CPU feature analysis report
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CpuFeatureAnalysis {
    /// CPU model information
    pub cpu_model: CpuModel,

    /// Features present in CPU but potentially disabled
    pub features: Vec<CpuFeature>,

    /// BIOS locks detected
    pub bios_locks: Vec<BiosLock>,

    /// Recommendations for enabling hidden features
    pub recommendations: Vec<Recommendation>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CpuModel {
    pub vendor: String,     // "Intel", "AMD"
    pub family: String,     // CPU family
    pub model_name: String, // Full model string
    pub stepping: u32,
    pub microcode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CpuFeature {
    pub name: String,           // "VT-x", "AMD-V", "VT-d", "SGX", etc.
    pub technical_name: String, // "vmx", "svm", "iommu", "sgx"
    pub category: FeatureCategory,
    pub status: FeatureStatus,
    pub bios_locked: bool, // True if BIOS prevents enabling
    pub unlock_method: Option<UnlockMethod>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FeatureCategory {
    Virtualization,  // VT-x, AMD-V, VT-d
    Security,        // SGX, TXT, SME, SEV
    Performance,     // Turbo Boost, SpeedStep
    PowerManagement, // C-states, P-states
    Debugging,       // Performance counters, debug registers
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FeatureStatus {
    /// Feature is enabled and working
    Enabled,

    /// Feature supported by CPU but disabled
    DisabledByBios,

    /// Feature supported but blocked by BIOS lock
    LockedByBios,

    /// Feature not supported by CPU
    NotSupported,

    /// Feature partially enabled (some aspects locked)
    PartiallyEnabled,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BiosLock {
    pub register: String, // "MSR 0x3A", "CPUID leaf 0x7"
    pub lock_bit: String, // "Bit 0 (Lock)"
    pub affected_features: Vec<String>,
    pub locked: bool,
    pub lock_method: String, // "MSR lock bit", "BIOS setting", "Vendor fuse"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnlockMethod {
    pub method: String,
    pub risk_level: RiskLevel,
    pub commands: Vec<String>,
    pub description: String,
    pub requires_reboot: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,         // No risk, reversible
    Low,          // Minimal risk, easily reversible
    Medium,       // Some risk, may cause instability
    High,         // Significant risk, may brick BIOS
    VendorLocked, // Cannot be unlocked (hardware fuse)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Recommendation {
    pub priority: Priority,
    pub feature: String,
    pub reason: String,
    pub benefit: String,
    pub action: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical, // Essential for operation (e.g., VT-x for virtualization)
    High,     // Significant benefit
    Medium,   // Nice to have
    Low,      // Minor improvement
}

/// Analyzer for CPU features and BIOS locks
pub struct CpuFeatureAnalyzer;

impl Default for CpuFeatureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuFeatureAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze CPU features and BIOS locks
    pub fn analyze(&self) -> Result<CpuFeatureAnalysis> {
        let cpu_model = self.detect_cpu_model()?;
        let cpuinfo_flags = self.read_cpuinfo_flags()?;
        let msr_available = self.check_msr_available();

        let mut features = Vec::new();
        let mut bios_locks = Vec::new();
        let mut recommendations = Vec::new();

        // Check virtualization features (VT-x/AMD-V)
        let virt_feature = self.check_virtualization(&cpu_model, &cpuinfo_flags, msr_available)?;
        if let Some((feature, lock, rec)) = virt_feature {
            features.push(feature.clone());
            if let Some(l) = lock {
                bios_locks.push(l);
            }
            if let Some(r) = rec {
                recommendations.push(r);
            }
        }

        // Check IOMMU (VT-d/AMD-Vi)
        let iommu_feature = self.check_iommu(&cpuinfo_flags)?;
        if let Some((feature, rec)) = iommu_feature {
            features.push(feature);
            if let Some(r) = rec {
                recommendations.push(r);
            }
        }

        // Check Intel SGX (Software Guard Extensions)
        if cpu_model.vendor == "Intel" {
            if let Some((feature, lock, rec)) = self.check_sgx(&cpuinfo_flags, msr_available)? {
                features.push(feature);
                if let Some(l) = lock {
                    bios_locks.push(l);
                }
                if let Some(r) = rec {
                    recommendations.push(r);
                }
            }
        }

        // Check Turbo Boost / Precision Boost
        let turbo_feature = self.check_turbo(&cpu_model, msr_available)?;
        if let Some((feature, rec)) = turbo_feature {
            features.push(feature);
            if let Some(r) = rec {
                recommendations.push(r);
            }
        }

        // Check AMD SME/SEV (Secure Memory Encryption)
        if cpu_model.vendor == "AMD" {
            if let Some((feature, rec)) = self.check_amd_encryption(&cpuinfo_flags)? {
                features.push(feature);
                if let Some(r) = rec {
                    recommendations.push(r);
                }
            }
        }

        // Sort recommendations by priority
        recommendations.sort_by_key(|r| r.priority.clone());

        Ok(CpuFeatureAnalysis {
            cpu_model,
            features,
            bios_locks,
            recommendations,
        })
    }

    /// Detect CPU model information
    fn detect_cpu_model(&self) -> Result<CpuModel> {
        let cpuinfo =
            fs::read_to_string("/proc/cpuinfo").context("Failed to read /proc/cpuinfo")?;

        let mut vendor = "Unknown".to_string();
        let mut model_name = "Unknown".to_string();
        let mut family = "Unknown".to_string();
        let mut stepping = 0u32;
        let mut microcode = "Unknown".to_string();

        for line in cpuinfo.lines() {
            if line.starts_with("vendor_id") {
                vendor = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string();
                // Normalize vendor names
                if vendor.contains("Intel") {
                    vendor = "Intel".to_string();
                } else if vendor.contains("AMD") {
                    vendor = "AMD".to_string();
                }
            } else if line.starts_with("model name") {
                model_name = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string();
            } else if line.starts_with("cpu family") {
                family = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string();
            } else if line.starts_with("stepping") {
                if let Ok(s) = line.split(':').nth(1).unwrap_or("0").trim().parse() {
                    stepping = s;
                }
            } else if line.starts_with("microcode") {
                microcode = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string();
            }
        }

        Ok(CpuModel {
            vendor,
            family,
            model_name,
            stepping,
            microcode,
        })
    }

    /// Read CPU flags from /proc/cpuinfo
    fn read_cpuinfo_flags(&self) -> Result<Vec<String>> {
        let cpuinfo =
            fs::read_to_string("/proc/cpuinfo").context("Failed to read /proc/cpuinfo")?;

        for line in cpuinfo.lines() {
            if line.starts_with("flags") || line.starts_with("Features") {
                let flags: Vec<String> = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("")
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                return Ok(flags);
            }
        }

        Ok(Vec::new())
    }

    /// Check if MSR (Model Specific Register) access is available
    fn check_msr_available(&self) -> bool {
        std::path::Path::new("/dev/cpu/0/msr").exists()
            || Command::new("modprobe").arg("msr").output().is_ok()
    }

    /// Check virtualization support (VT-x/AMD-V)
    fn check_virtualization(
        &self,
        cpu_model: &CpuModel,
        flags: &[String],
        msr_available: bool,
    ) -> FeatureCheckResult {
        let (feature_flag, feature_name, technical_name) = if cpu_model.vendor == "Intel" {
            ("vmx", "VT-x (Intel Virtualization)", "vmx")
        } else if cpu_model.vendor == "AMD" {
            ("svm", "AMD-V (AMD Virtualization)", "svm")
        } else {
            return Ok(None);
        };

        let cpu_supports = flags.contains(&feature_flag.to_string());

        if !cpu_supports {
            return Ok(Some((
                CpuFeature {
                    name: feature_name.to_string(),
                    technical_name: technical_name.to_string(),
                    category: FeatureCategory::Virtualization,
                    status: FeatureStatus::NotSupported,
                    bios_locked: false,
                    unlock_method: None,
                },
                None,
                None,
            )));
        }

        // Check if actually enabled (can we use KVM?)
        let kvm_enabled = std::path::Path::new("/dev/kvm").exists();

        let (status, bios_lock, recommendation) = if !kvm_enabled
            && cpu_model.vendor == "Intel"
            && msr_available
        {
            // CPU supports VT-x but /dev/kvm doesn't exist
            // Check MSR 0x3A (IA32_FEATURE_CONTROL) to see if BIOS locked it
            let msr_lock_status = self.check_intel_vmx_lock()?;

            match msr_lock_status {
                VmxLockStatus::Locked => {
                    (
                        FeatureStatus::LockedByBios,
                        Some(BiosLock {
                            register: "MSR 0x3A (IA32_FEATURE_CONTROL)".to_string(),
                            lock_bit: "Bit 0 (Lock), Bit 2 (VMX Enable)".to_string(),
                            affected_features: vec!["VT-x".to_string(), "KVM".to_string()],
                            locked: true,
                            lock_method: "BIOS MSR lock bit set before OS boot".to_string(),
                        }),
                        Some(Recommendation {
                            priority: Priority::Critical,
                            feature: "VT-x".to_string(),
                            reason: "CPU supports VT-x but BIOS has locked it via MSR".to_string(),
                            benefit: "Enable KVM virtualization, Docker, QEMU, VirtualBox with hardware acceleration".to_string(),
                            action: "BIOS Update: Check for BIOS update that exposes VT-x option, or use BIOS modification tools (advanced)".to_string(),
                        })
                    )
                },
                VmxLockStatus::DisabledUnlocked => {
                    (
                        FeatureStatus::DisabledByBios,
                        None,
                        Some(Recommendation {
                            priority: Priority::Critical,
                            feature: "VT-x".to_string(),
                            reason: "CPU supports VT-x but it is disabled (BIOS not locked)".to_string(),
                            benefit: "Enable KVM virtualization for Docker, QEMU, VirtualBox".to_string(),
                            action: "Can be enabled via MSR write: modprobe msr && wrmsr 0x3A 0x5".to_string(),
                        })
                    )
                },
                VmxLockStatus::EnabledLocked => {
                    // This shouldn't happen if /dev/kvm doesn't exist, but handle it
                    (FeatureStatus::Enabled, None, None)
                },
            }
        } else if !kvm_enabled {
            // AMD or MSR not available - just report disabled
            (
                FeatureStatus::DisabledByBios,
                None,
                Some(Recommendation {
                    priority: Priority::Critical,
                    feature: feature_name.to_string(),
                    reason: format!("CPU supports {} but /dev/kvm is not available", feature_name),
                    benefit: "Enable virtualization for KVM, Docker, QEMU".to_string(),
                    action: "Enter BIOS/UEFI setup and enable virtualization (usually under CPU or Advanced settings)".to_string(),
                })
            )
        } else {
            (FeatureStatus::Enabled, None, None)
        };

        Ok(Some((
            CpuFeature {
                name: feature_name.to_string(),
                technical_name: technical_name.to_string(),
                category: FeatureCategory::Virtualization,
                status,
                bios_locked: bios_lock.is_some(),
                unlock_method: self.create_vmx_unlock_method(&cpu_model.vendor),
            },
            bios_lock,
            recommendation,
        )))
    }

    /// Check Intel VT-x lock status via MSR
    fn check_intel_vmx_lock(&self) -> Result<VmxLockStatus> {
        // Try to read MSR 0x3A (IA32_FEATURE_CONTROL)
        let output = Command::new("rdmsr").arg("0x3A").output();

        if let Ok(out) = output {
            if out.status.success() {
                let value_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if let Ok(value) = u64::from_str_radix(&value_str, 16) {
                    let lock_bit = value & 0x1; // Bit 0: Lock
                    let vmx_enable = value & 0x4; // Bit 2: VMX Enable

                    if lock_bit == 1 {
                        if vmx_enable == 0 {
                            return Ok(VmxLockStatus::Locked);
                        } else {
                            return Ok(VmxLockStatus::EnabledLocked);
                        }
                    } else {
                        return Ok(VmxLockStatus::DisabledUnlocked);
                    }
                }
            }
        }

        // Can't read MSR, assume disabled
        Ok(VmxLockStatus::DisabledUnlocked)
    }

    fn create_vmx_unlock_method(&self, vendor: &str) -> Option<UnlockMethod> {
        if vendor == "Intel" {
            Some(UnlockMethod {
                method: "MSR Write".to_string(),
                risk_level: RiskLevel::Medium,
                commands: vec![
                    "# Load MSR kernel module".to_string(),
                    "modprobe msr".to_string(),
                    "# Enable VT-x (MSR 0x3A = 0x5: Lock=1, VMX=1)".to_string(),
                    "wrmsr 0x3A 0x5".to_string(),
                    "# Check /dev/kvm now exists".to_string(),
                    "ls -l /dev/kvm".to_string(),
                ],
                description: "Write to IA32_FEATURE_CONTROL MSR to enable VT-x. Only works if BIOS has not set lock bit. Requires reboot to persist.".to_string(),
                requires_reboot: true,
            })
        } else {
            Some(UnlockMethod {
                method: "BIOS Setting".to_string(),
                risk_level: RiskLevel::Safe,
                commands: vec![
                    "# Reboot and enter BIOS/UEFI setup".to_string(),
                    "# Navigate to CPU or Advanced settings".to_string(),
                    "# Enable SVM (AMD Virtualization)".to_string(),
                ],
                description: "Enable AMD-V in BIOS settings. Usually found under CPU Configuration or Advanced settings.".to_string(),
                requires_reboot: true,
            })
        }
    }

    /// Check IOMMU (VT-d/AMD-Vi) support
    fn check_iommu(
        &self,
        _flags: &[String],
    ) -> Result<Option<(CpuFeature, Option<Recommendation>)>> {
        // Check for IOMMU support in kernel
        let dmesg_output = Command::new("dmesg")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        let iommu_enabled = dmesg_output.contains("IOMMU enabled")
            || dmesg_output.contains("AMD-Vi")
            || dmesg_output.contains("DMAR");

        let iommu_groups_exist = std::path::Path::new("/sys/kernel/iommu_groups").exists();

        let status = if iommu_enabled || iommu_groups_exist {
            FeatureStatus::Enabled
        } else {
            FeatureStatus::DisabledByBios
        };

        let recommendation = if status == FeatureStatus::DisabledByBios {
            Some(Recommendation {
                priority: Priority::High,
                feature: "IOMMU (VT-d/AMD-Vi)".to_string(),
                reason: "IOMMU support not detected in kernel".to_string(),
                benefit: "Enable PCI passthrough for VMs, improved device isolation and security".to_string(),
                action: "Enable VT-d (Intel) or AMD-Vi (AMD) in BIOS, add intel_iommu=on or amd_iommu=on to kernel parameters".to_string(),
            })
        } else {
            None
        };

        Ok(Some((
            CpuFeature {
                name: "IOMMU (VT-d/AMD-Vi)".to_string(),
                technical_name: "iommu".to_string(),
                category: FeatureCategory::Virtualization,
                status,
                bios_locked: false,
                unlock_method: None,
            },
            recommendation,
        )))
    }

    /// Check Intel SGX (Software Guard Extensions)
    fn check_sgx(&self, flags: &[String], _msr_available: bool) -> FeatureCheckResult {
        let sgx_supported = flags.contains(&"sgx".to_string());

        if !sgx_supported {
            return Ok(None);
        }

        // Check if SGX is enabled
        let sgx_enabled = std::path::Path::new("/dev/sgx").exists()
            || std::path::Path::new("/dev/sgx_enclave").exists();

        let (status, lock, rec) = if !sgx_enabled {
            (
                FeatureStatus::DisabledByBios,
                None,
                Some(Recommendation {
                    priority: Priority::Medium,
                    feature: "Intel SGX".to_string(),
                    reason: "CPU supports SGX but it is disabled".to_string(),
                    benefit:
                        "Enable secure enclaves for confidential computing, secrets management"
                            .to_string(),
                    action: "Enable Intel SGX in BIOS (usually under Security or CPU settings)"
                        .to_string(),
                }),
            )
        } else {
            (FeatureStatus::Enabled, None, None)
        };

        Ok(Some((
            CpuFeature {
                name: "Intel SGX (Software Guard Extensions)".to_string(),
                technical_name: "sgx".to_string(),
                category: FeatureCategory::Security,
                status,
                bios_locked: lock.is_some(),
                unlock_method: None,
            },
            lock,
            rec,
        )))
    }

    /// Check Turbo Boost / Precision Boost
    fn check_turbo(
        &self,
        cpu_model: &CpuModel,
        _msr_available: bool,
    ) -> Result<Option<(CpuFeature, Option<Recommendation>)>> {
        // Check if turbo is currently enabled
        let turbo_enabled = if cpu_model.vendor == "Intel" {
            // Check /sys/devices/system/cpu/intel_pstate/no_turbo
            if let Ok(contents) =
                fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo")
            {
                contents.trim() == "0" // no_turbo=0 means turbo is enabled
            } else {
                // Assume enabled if can't check
                true
            }
        } else {
            // AMD: check cpufreq boost
            if let Ok(contents) = fs::read_to_string("/sys/devices/system/cpu/cpufreq/boost") {
                contents.trim() == "1"
            } else {
                true
            }
        };

        let feature_name = if cpu_model.vendor == "Intel" {
            "Intel Turbo Boost"
        } else {
            "AMD Precision Boost"
        };

        let status = if turbo_enabled {
            FeatureStatus::Enabled
        } else {
            FeatureStatus::DisabledByBios
        };

        let recommendation = if !turbo_enabled {
            Some(Recommendation {
                priority: Priority::Medium,
                feature: feature_name.to_string(),
                reason: "Turbo/Boost is disabled".to_string(),
                benefit: "Improve single-threaded performance by 20-30%".to_string(),
                action: format!("Enable {} in BIOS or via sysfs", feature_name),
            })
        } else {
            None
        };

        Ok(Some((
            CpuFeature {
                name: feature_name.to_string(),
                technical_name: "turbo".to_string(),
                category: FeatureCategory::Performance,
                status,
                bios_locked: false,
                unlock_method: None,
            },
            recommendation,
        )))
    }

    /// Check AMD SME/SEV (Secure Memory Encryption)
    fn check_amd_encryption(
        &self,
        flags: &[String],
    ) -> Result<Option<(CpuFeature, Option<Recommendation>)>> {
        let sme_supported = flags.contains(&"sme".to_string());
        let sev_supported = flags.contains(&"sev".to_string());

        if !sme_supported && !sev_supported {
            return Ok(None);
        }

        // Check if actually enabled
        let mem_encrypt = fs::read_to_string("/sys/kernel/mm/mem_encrypt/active")
            .unwrap_or_else(|_| "0".to_string());
        let enabled = mem_encrypt.trim() == "1";

        let status = if enabled {
            FeatureStatus::Enabled
        } else {
            FeatureStatus::DisabledByBios
        };

        let recommendation = if !enabled {
            Some(Recommendation {
                priority: Priority::High,
                feature: "AMD SME/SEV".to_string(),
                reason: "CPU supports memory encryption but it is disabled".to_string(),
                benefit: "Encrypt system memory, protect against physical attacks, enable confidential computing".to_string(),
                action: "Enable AMD Memory Guard/SME in BIOS, add mem_encrypt=on to kernel parameters".to_string(),
            })
        } else {
            None
        };

        Ok(Some((
            CpuFeature {
                name: "AMD SME/SEV (Secure Memory Encryption)".to_string(),
                technical_name: "sme/sev".to_string(),
                category: FeatureCategory::Security,
                status,
                bios_locked: false,
                unlock_method: None,
            },
            recommendation,
        )))
    }
}

#[derive(Debug, PartialEq)]
enum VmxLockStatus {
    Locked,           // BIOS locked VT-x disabled
    EnabledLocked,    // BIOS locked VT-x enabled
    DisabledUnlocked, // Not locked, can be enabled
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/hierarchical.rs">
// Hierarchical D-Bus Introspection with JSON Caching
// Implements comprehensive D-Bus discovery using all methods from the guide:
// - Recursive object path traversal with zbus_xml::Node
// - ObjectManager.GetManagedObjects for bulk discovery
// - Proper handling of non-introspectable objects
// - Full interface, method, signal, and property introspection
// - JSON caching to BTRFS @cache/introspection/ subvolume

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use zbus::zvariant;
use zbus::{Connection, Proxy};
use zbus_xml::Node;

/// Hierarchical D-Bus introspection snapshot
/// Stored as JSON in @cache/introspection/{timestamp}.json
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HierarchicalIntrospection {
    /// Timestamp of snapshot
    pub timestamp: String,

    /// System bus services
    pub system_bus: BusIntrospection,

    /// Session bus services
    pub session_bus: BusIntrospection,

    /// Summary statistics
    pub summary: IntrospectionSummary,
}

/// Introspection data for a single bus
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BusIntrospection {
    /// All services on this bus (indexed by service name)
    pub services: HashMap<String, ServiceIntrospection>,

    /// Total object count across all services
    pub total_objects: usize,

    /// Total interface count
    pub total_interfaces: usize,
}

/// Complete introspection data for a D-Bus service
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceIntrospection {
    /// Service name (e.g., "org.freedesktop.NetworkManager")
    pub name: String,

    /// Bus type ("system" or "session")
    pub bus_type: String,

    /// All object paths in this service
    pub objects: HashMap<String, ObjectIntrospection>,

    /// Whether ObjectManager was used for discovery
    pub used_object_manager: bool,

    /// Root object path (typically / or /org/freedesktop/ServiceName)
    pub root_path: String,
}

/// Complete introspection data for a D-Bus object
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ObjectIntrospection {
    /// Object path (e.g., "/org/freedesktop/NetworkManager")
    pub path: String,

    /// Interfaces implemented by this object
    pub interfaces: Vec<InterfaceIntrospection>,

    /// Child object paths (for tree traversal)
    pub children: Vec<String>,

    /// Whether this object is introspectable
    pub introspectable: bool,

    /// Error message if introspection failed
    pub error: Option<String>,
}

/// Complete interface introspection data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InterfaceIntrospection {
    /// Interface name (e.g., "org.freedesktop.NetworkManager")
    pub name: String,

    /// Methods on this interface
    pub methods: Vec<MethodIntrospection>,

    /// Properties on this interface
    pub properties: Vec<PropertyIntrospection>,

    /// Signals on this interface
    pub signals: Vec<SignalIntrospection>,
}

/// Method introspection data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MethodIntrospection {
    /// Method name (e.g., "GetDevices")
    pub name: String,

    /// Input arguments
    pub inputs: Vec<ArgumentIntrospection>,

    /// Output arguments
    pub outputs: Vec<ArgumentIntrospection>,
}

/// Property introspection data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PropertyIntrospection {
    /// Property name
    pub name: String,

    /// D-Bus type signature
    pub type_: String,

    /// Access mode ("read", "write", "readwrite")
    pub access: String,
}

/// Signal introspection data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalIntrospection {
    /// Signal name
    pub name: String,

    /// Signal arguments
    pub args: Vec<ArgumentIntrospection>,
}

/// Method/Signal argument
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ArgumentIntrospection {
    /// Argument name
    pub name: Option<String>,

    /// D-Bus type signature
    pub type_: String,

    /// Direction ("in" or "out")
    pub direction: Option<String>,
}

/// Summary statistics
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntrospectionSummary {
    pub total_services: usize,
    pub total_objects: usize,
    pub total_interfaces: usize,
    pub total_methods: usize,
    pub non_introspectable_objects: usize,
    pub services_with_object_manager: usize,
}

/// Hierarchical D-Bus introspector
pub struct HierarchicalIntrospector {
    cache_dir: PathBuf,
}

impl HierarchicalIntrospector {
    /// Create new introspector with cache directory
    pub async fn new(cache_dir: PathBuf) -> Result<Self> {
        // Create @cache/introspection subvolume if needed
        let introspection_cache = cache_dir.join("introspection");

        // Check if parent @cache exists
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        // Create introspection subdirectory
        tokio::fs::create_dir_all(&introspection_cache).await?;

        info!(
            "Hierarchical introspection cache: {}",
            introspection_cache.display()
        );

        Ok(Self { cache_dir })
    }

    /// Perform comprehensive introspection of both buses
    pub async fn introspect_all(&self) -> Result<HierarchicalIntrospection> {
        info!("Starting comprehensive D-Bus introspection");

        let timestamp = chrono::Utc::now().to_rfc3339();

        // Introspect system bus
        info!("Introspecting system bus...");
        let system_bus = self.introspect_bus("system").await?;

        // Introspect session bus (may not be available in all contexts)
        info!("Introspecting session bus...");
        let session_bus = match self.introspect_bus("session").await {
            Ok(bus) => bus,
            Err(e) => {
                warn!("Session bus not available: {}", e);
                BusIntrospection {
                    services: HashMap::new(),
                    total_objects: 0,
                    total_interfaces: 0,
                }
            }
        };

        // Calculate summary
        let summary = Self::calculate_summary(&system_bus, &session_bus);

        let introspection = HierarchicalIntrospection {
            timestamp,
            system_bus,
            session_bus,
            summary,
        };

        // Save to cache
        self.save_to_cache(&introspection).await?;

        info!(
            "Introspection complete: {} services, {} objects, {} interfaces",
            introspection.summary.total_services,
            introspection.summary.total_objects,
            introspection.summary.total_interfaces
        );

        Ok(introspection)
    }

    /// Introspect a single bus (system or session)
    async fn introspect_bus(&self, bus_type: &str) -> Result<BusIntrospection> {
        // Connect to bus
        let connection = match bus_type {
            "system" => Connection::system().await?,
            "session" => Connection::session().await?,
            _ => anyhow::bail!("Invalid bus type: {}", bus_type),
        };

        // Get list of all services on the bus
        let service_names = self.list_services(&connection).await?;

        info!("Found {} services on {} bus", service_names.len(), bus_type);

        let mut services = HashMap::new();
        let mut total_objects = 0;
        let mut total_interfaces = 0;

        for service_name in service_names {
            debug!("Introspecting service: {}", service_name);

            match self
                .introspect_service(&connection, &service_name, bus_type)
                .await
            {
                Ok(service_data) => {
                    total_objects += service_data.objects.len();
                    total_interfaces += service_data
                        .objects
                        .values()
                        .map(|obj| obj.interfaces.len())
                        .sum::<usize>();

                    services.insert(service_name.clone(), service_data);
                }
                Err(e) => {
                    warn!("Failed to introspect {}: {}", service_name, e);
                }
            }
        }

        Ok(BusIntrospection {
            services,
            total_objects,
            total_interfaces,
        })
    }

    /// List all service names on a bus
    async fn list_services(&self, conn: &Connection) -> Result<Vec<String>> {
        use zbus::fdo::DBusProxy;

        let proxy = DBusProxy::new(conn).await?;
        let names = proxy.list_names().await?;

        // Filter out unique names (starting with :) and org.freedesktop.DBus itself
        Ok(names
            .into_iter()
            .filter(|name| !name.starts_with(':'))
            .filter(|name| name.as_str() != "org.freedesktop.DBus")
            .map(|name| name.to_string())
            .collect())
    }

    /// Introspect a single service completely
    async fn introspect_service(
        &self,
        conn: &Connection,
        service_name: &str,
        bus_type: &str,
    ) -> Result<ServiceIntrospection> {
        let mut objects = HashMap::new();
        let mut used_object_manager = false;

        // Try ObjectManager first (most efficient)
        let root_path = Self::guess_root_path(service_name);

        if let Ok(managed_objects) = self
            .try_object_manager(conn, service_name, &root_path)
            .await
        {
            info!("Service {} provides ObjectManager", service_name);
            used_object_manager = true;

            // Parse managed objects into our format
            for (path, iface_data) in managed_objects {
                let obj_data = self
                    .introspect_object_by_path(conn, service_name, &path)
                    .await?;

                objects.insert(path.to_string(), obj_data);
            }
        } else {
            // Fall back to recursive introspection
            debug!(
                "ObjectManager not available for {}, using recursive introspection",
                service_name
            );

            self.introspect_recursively(conn, service_name, &root_path, &mut objects)
                .await?;
        }

        Ok(ServiceIntrospection {
            name: service_name.to_string(),
            bus_type: bus_type.to_string(),
            objects,
            used_object_manager,
            root_path,
        })
    }

    /// Try to use ObjectManager.GetManagedObjects for bulk discovery
    async fn try_object_manager(
        &self,
        conn: &Connection,
        service_name: &str,
        root_path: &str,
    ) -> Result<HashMap<String, HashMap<String, HashMap<String, zvariant::OwnedValue>>>> {
        let proxy = Proxy::new(
            conn,
            service_name,
            root_path,
            "org.freedesktop.DBus.ObjectManager",
        )
        .await?;

        // Call GetManagedObjects
        let result: HashMap<
            zbus::zvariant::OwnedObjectPath,
            HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>,
        > = proxy.call("GetManagedObjects", &()).await?;

        // Convert to string keys
        Ok(result
            .into_iter()
            .map(|(path, ifaces)| {
                (
                    path.to_string(),
                    ifaces
                        .into_iter()
                        .map(|(iface, props)| (iface.to_string(), props))
                        .collect(),
                )
            })
            .collect())
    }

    /// Recursively introspect object tree starting from a root path
    async fn introspect_recursively(
        &self,
        conn: &Connection,
        service_name: &str,
        path: &str,
        objects: &mut HashMap<String, ObjectIntrospection>,
    ) -> Result<()> {
        // Introspect this object
        let obj_data = self
            .introspect_object_by_path(conn, service_name, path)
            .await?;

        // Collect children before inserting (to avoid borrow issues)
        let children = obj_data.children.clone();
        objects.insert(path.to_string(), obj_data);

        // Recurse into children
        for child_name in children {
            let child_path = if path == "/" {
                format!("/{}", child_name)
            } else {
                format!("{}/{}", path, child_name)
            };

            // Recursive call (boxed to avoid infinite-sized future)
            Box::pin(self.introspect_recursively(conn, service_name, &child_path, objects)).await?;
        }

        Ok(())
    }

    /// Introspect a single object by path
    async fn introspect_object_by_path(
        &self,
        conn: &Connection,
        service_name: &str,
        path: &str,
    ) -> Result<ObjectIntrospection> {
        let proxy = Proxy::new(
            conn,
            service_name,
            path,
            "org.freedesktop.DBus.Introspectable",
        )
        .await?;

        // Try to introspect
        match proxy.introspect().await {
            Ok(xml) => {
                // Parse XML with zbus_xml
                let node = Node::from_reader(xml.as_bytes())
                    .context("Failed to parse introspection XML")?;

                // Extract interfaces
                let interfaces = node
                    .interfaces()
                    .iter()
                    .map(|iface| self.parse_interface(iface))
                    .collect();

                // Extract child node names
                let children = node
                    .nodes()
                    .iter()
                    .map(|child| child.name().unwrap_or("").to_string())
                    .filter(|name| !name.is_empty())
                    .collect();

                Ok(ObjectIntrospection {
                    path: path.to_string(),
                    interfaces,
                    children,
                    introspectable: true,
                    error: None,
                })
            }
            Err(e) => {
                // Object is not introspectable
                warn!("Cannot introspect {} on {}: {}", path, service_name, e);

                Ok(ObjectIntrospection {
                    path: path.to_string(),
                    interfaces: Vec::new(),
                    children: Vec::new(),
                    introspectable: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Parse interface from zbus_xml::Interface
    fn parse_interface(&self, iface: &zbus_xml::Interface) -> InterfaceIntrospection {
        let methods = iface
            .methods()
            .iter()
            .map(|method| {
                let inputs = method
                    .args()
                    .iter()
                    .filter(|arg| {
                        arg.direction()
                            .map(|d| matches!(d, zbus_xml::ArgDirection::In))
                            .unwrap_or(true)
                    })
                    .map(|arg| ArgumentIntrospection {
                        name: arg.name().map(String::from),
                        type_: arg.ty().to_string(),
                        direction: Some("in".to_string()),
                    })
                    .collect();

                let outputs = method
                    .args()
                    .iter()
                    .filter(|arg| {
                        arg.direction()
                            .map(|d| matches!(d, zbus_xml::ArgDirection::Out))
                            .unwrap_or(false)
                    })
                    .map(|arg| ArgumentIntrospection {
                        name: arg.name().map(String::from),
                        type_: arg.ty().to_string(),
                        direction: Some("out".to_string()),
                    })
                    .collect();

                MethodIntrospection {
                    name: method.name().to_string(),
                    inputs,
                    outputs,
                }
            })
            .collect();

        let properties = iface
            .properties()
            .iter()
            .map(|prop| PropertyIntrospection {
                name: prop.name().to_string(),
                type_: prop.ty().to_string(),
                access: {
                    // In zbus 4, Access enum may have moved
                    // Convert to string representation
                    format!("{:?}", prop.access()).to_lowercase()
                },
            })
            .collect();

        let signals = iface
            .signals()
            .iter()
            .map(|signal| {
                let args = signal
                    .args()
                    .iter()
                    .map(|arg| ArgumentIntrospection {
                        name: arg.name().map(String::from),
                        type_: arg.ty().to_string(),
                        direction: None,
                    })
                    .collect();

                SignalIntrospection {
                    name: signal.name().to_string(),
                    args,
                }
            })
            .collect();

        InterfaceIntrospection {
            name: iface.name().to_string(),
            methods,
            properties,
            signals,
        }
    }

    /// Guess root object path from service name
    fn guess_root_path(service_name: &str) -> String {
        // Common patterns:
        // org.freedesktop.NetworkManager -> /org/freedesktop/NetworkManager
        // org.bluez -> /

        if service_name == "org.bluez" {
            "/".to_string()
        } else {
            format!("/{}", service_name.replace('.', "/"))
        }
    }

    /// Calculate summary statistics
    fn calculate_summary(
        system: &BusIntrospection,
        session: &BusIntrospection,
    ) -> IntrospectionSummary {
        let total_services = system.services.len() + session.services.len();
        let total_objects = system.total_objects + session.total_objects;
        let total_interfaces = system.total_interfaces + session.total_interfaces;

        let total_methods = [system, session]
            .iter()
            .flat_map(|bus| bus.services.values())
            .flat_map(|svc| svc.objects.values())
            .flat_map(|obj| &obj.interfaces)
            .map(|iface| iface.methods.len())
            .sum();

        let non_introspectable_objects = [system, session]
            .iter()
            .flat_map(|bus| bus.services.values())
            .flat_map(|svc| svc.objects.values())
            .filter(|obj| !obj.introspectable)
            .count();

        let services_with_object_manager = [system, session]
            .iter()
            .flat_map(|bus| bus.services.values())
            .filter(|svc| svc.used_object_manager)
            .count();

        IntrospectionSummary {
            total_services,
            total_objects,
            total_interfaces,
            total_methods,
            non_introspectable_objects,
            services_with_object_manager,
        }
    }

    /// Save introspection to cache as JSON
    async fn save_to_cache(&self, data: &HierarchicalIntrospection) -> Result<()> {
        let cache_path = self.cache_dir.join("introspection");
        tokio::fs::create_dir_all(&cache_path).await?;

        // Save timestamped snapshot
        let filename = format!("{}.json", data.timestamp.replace(':', "-"));
        let snapshot_path = cache_path.join(&filename);

        let json = simd_json::to_string_pretty(data)?;
        tokio::fs::write(&snapshot_path, json).await?;

        info!("Saved introspection snapshot: {}", snapshot_path.display());

        // Also save as "latest.json" for easy access
        let latest_path = cache_path.join("latest.json");
        let json = simd_json::to_string_pretty(data)?;
        tokio::fs::write(&latest_path, json).await?;

        Ok(())
    }

    /// Load latest introspection from cache
    pub async fn load_latest(&self) -> Result<HierarchicalIntrospection> {
        let latest_path = self.cache_dir.join("introspection/latest.json");

        if !latest_path.exists() {
            anyhow::bail!("No cached introspection found, run introspect_all() first");
        }

        let json = tokio::fs::read_to_string(&latest_path).await?;
        let data: HierarchicalIntrospection = simd_json::from_str(&json)?;

        Ok(data)
    }

    /// Load introspection by timestamp
    pub async fn load_by_timestamp(&self, timestamp: &str) -> Result<HierarchicalIntrospection> {
        let filename = format!("{}.json", timestamp.replace(':', "-"));
        let path = self.cache_dir.join("introspection").join(&filename);

        let json = tokio::fs::read_to_string(&path).await?;
        let data: HierarchicalIntrospection = simd_json::from_str(&json)?;

        Ok(data)
    }

    /// List all cached introspection timestamps
    pub async fn list_snapshots(&self) -> Result<Vec<String>> {
        let cache_path = self.cache_dir.join("introspection");

        if !cache_path.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();
        let mut entries = tokio::fs::read_dir(&cache_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            if filename_str.ends_with(".json") && filename_str != "latest.json" {
                // Extract timestamp from filename
                let timestamp = filename_str.trim_end_matches(".json").replace('-', ":");
                snapshots.push(timestamp);
            }
        }

        snapshots.sort();
        Ok(snapshots)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/indexer_manager.rs">
//! Async-safe indexer manager
//!
//! Wraps the DbusIndexer to provide async-safe access using spawn_blocking

use crate::indexer::{DbusIndexer, IndexStatistics, SearchResult};
use anyhow::Result;
use op_core::types::BusType;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Async-safe wrapper around DbusIndexer
pub struct IndexerManager {
    db_path: PathBuf,
    // Mutex protects against concurrent database access
    // Inner Option allows taking ownership for spawn_blocking
    #[allow(clippy::arc_with_non_send_sync)]
    _indexer: Arc<Mutex<Option<DbusIndexer>>>,
}

impl IndexerManager {
    /// Create new indexer manager
    #[allow(clippy::arc_with_non_send_sync)]
    pub async fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let indexer = DbusIndexer::new(&db_path).await?;

        Ok(Self {
            db_path,
            _indexer: Arc::new(Mutex::new(Some(indexer))),
        })
    }

    /// Build or rebuild the index
    pub async fn build_index(&self, bus_type: BusType) -> Result<IndexStatistics> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            // Create a new indexer in the blocking task
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.build_index(bus_type).await
            })
        })
        .await?
    }

    /// Search methods
    pub async fn search_methods(&self, query: String, limit: usize) -> Result<Vec<SearchResult>> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.search_methods(&query, limit)
            })
        })
        .await?
    }

    /// Search properties
    pub async fn search_properties(
        &self,
        query: String,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.search_properties(&query, limit)
            })
        })
        .await?
    }

    /// Search all (methods, properties, signals)
    pub async fn search_all(&self, query: String, limit: usize) -> Result<Vec<SearchResult>> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.search_all(&query, limit)
            })
        })
        .await?
    }

    /// Get statistics
    pub async fn get_statistics(&self) -> Result<Option<IndexStatistics>> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.get_statistics()
            })
        })
        .await?
    }

    /// Clear index
    pub async fn clear_index(&self) -> Result<()> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.clear_index()
            })
        })
        .await?
    }
}

// IndexerManager is Send + Sync by virtue of using Arc<Mutex<...>>
unsafe impl Send for IndexerManager {}
unsafe impl Sync for IndexerManager {}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/indexer.rs">
//! DBus Hierarchical Indexer with FTS5 Full-Text Search
//!
//! Builds a persistent, searchable index of all DBus services, objects, methods, and properties.
//! Enables semantic queries like "find all network-related methods" without real-time DBus calls.

use anyhow::{Context, Result};
use chrono::Utc;
use op_core::types::{BusType, ObjectInfo};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

use crate::scanner::ServiceScanner;

/// Statistics about the indexed DBus system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    pub total_services: usize,
    pub total_objects: usize,
    pub total_interfaces: usize,
    pub total_methods: usize,
    pub total_properties: usize,
    pub total_signals: usize,
    pub scan_duration_seconds: f64,
    pub indexed_at: i64,
}

/// Search result for FTS queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub service: String,
    pub object_path: String,
    pub interface: String,
    pub item_type: String, // "method", "property", "signal", "interface"
    pub item_name: String,
    pub description: String,
    pub relevance_score: f64,
}

/// DBus FTS Indexer - builds searchable index of entire DBus system
pub struct DbusIndexer {
    #[allow(clippy::arc_with_non_send_sync)]
    conn: Arc<RwLock<Connection>>,
    scanner: Arc<ServiceScanner>,
}

impl DbusIndexer {
    /// Create new indexer with SQLite database
    #[allow(clippy::arc_with_non_send_sync)]
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path.as_ref()).context("Failed to open indexer database")?;

        // Create schema with FTS5 virtual tables
        conn.execute_batch(
            r#"
            -- Core index tables
            CREATE TABLE IF NOT EXISTS services (
                service_name TEXT PRIMARY KEY,
                indexed_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS objects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT NOT NULL,
                object_path TEXT NOT NULL,
                indexed_at INTEGER NOT NULL,
                UNIQUE(service_name, object_path)
            );

            CREATE TABLE IF NOT EXISTS interfaces (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                object_id INTEGER NOT NULL,
                interface_name TEXT NOT NULL,
                indexed_at INTEGER NOT NULL,
                FOREIGN KEY(object_id) REFERENCES objects(id),
                UNIQUE(object_id, interface_name)
            );

            CREATE TABLE IF NOT EXISTS methods (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                interface_id INTEGER NOT NULL,
                method_name TEXT NOT NULL,
                input_signature TEXT,
                output_signature TEXT,
                description TEXT,
                FOREIGN KEY(interface_id) REFERENCES interfaces(id),
                UNIQUE(interface_id, method_name)
            );

            CREATE TABLE IF NOT EXISTS properties (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                interface_id INTEGER NOT NULL,
                property_name TEXT NOT NULL,
                type_signature TEXT NOT NULL,
                access TEXT NOT NULL,
                description TEXT,
                FOREIGN KEY(interface_id) REFERENCES interfaces(id),
                UNIQUE(interface_id, property_name)
            );

            CREATE TABLE IF NOT EXISTS signals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                interface_id INTEGER NOT NULL,
                signal_name TEXT NOT NULL,
                signature TEXT,
                description TEXT,
                FOREIGN KEY(interface_id) REFERENCES interfaces(id),
                UNIQUE(interface_id, signal_name)
            );

            -- FTS5 virtual tables for full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS methods_fts USING fts5(
                service_name,
                object_path,
                interface_name,
                method_name,
                description,
                input_signature,
                output_signature,
                content=methods,
                content_rowid=id
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS properties_fts USING fts5(
                service_name,
                object_path,
                interface_name,
                property_name,
                description,
                type_signature,
                access,
                content=properties,
                content_rowid=id
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS signals_fts USING fts5(
                service_name,
                object_path,
                interface_name,
                signal_name,
                description,
                signature,
                content=signals,
                content_rowid=id
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS interfaces_fts USING fts5(
                service_name,
                object_path,
                interface_name,
                content=interfaces,
                content_rowid=id
            );

            -- Triggers to keep FTS tables in sync
            CREATE TRIGGER IF NOT EXISTS methods_ai AFTER INSERT ON methods BEGIN
                INSERT INTO methods_fts(rowid, service_name, object_path, interface_name,
                                       method_name, description, input_signature, output_signature)
                SELECT m.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name,
                       m.method_name,
                       m.description,
                       m.input_signature,
                       m.output_signature
                FROM methods m
                JOIN interfaces i ON m.interface_id = i.id
                JOIN objects o ON i.object_id = o.id
                WHERE m.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS methods_au AFTER UPDATE ON methods BEGIN
                INSERT INTO methods_fts(methods_fts, rowid) VALUES('delete', OLD.id);
                INSERT INTO methods_fts(rowid, service_name, object_path, interface_name,
                                       method_name, description, input_signature, output_signature)
                SELECT m.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name,
                       m.method_name,
                       m.description,
                       m.input_signature,
                       m.output_signature
                FROM methods m
                JOIN interfaces i ON m.interface_id = i.id
                JOIN objects o ON i.object_id = o.id
                WHERE m.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS properties_ai AFTER INSERT ON properties BEGIN
                INSERT INTO properties_fts(rowid, service_name, object_path, interface_name,
                                          property_name, description, type_signature, access)
                SELECT p.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name,
                       p.property_name,
                       p.description,
                       p.type_signature,
                       p.access
                FROM properties p
                JOIN interfaces i ON p.interface_id = i.id
                JOIN objects o ON i.object_id = o.id
                WHERE p.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS properties_au AFTER UPDATE ON properties BEGIN
                INSERT INTO properties_fts(properties_fts, rowid) VALUES('delete', OLD.id);
                INSERT INTO properties_fts(rowid, service_name, object_path, interface_name,
                                          property_name, description, type_signature, access)
                SELECT p.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name,
                       p.property_name,
                       p.description,
                       p.type_signature,
                       p.access
                FROM properties p
                JOIN interfaces i ON p.interface_id = i.id
                JOIN objects o ON i.object_id = o.id
                WHERE p.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS signals_ai AFTER INSERT ON signals BEGIN
                INSERT INTO signals_fts(rowid, service_name, object_path, interface_name,
                                       signal_name, description, signature)
                SELECT s.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name,
                       s.signal_name,
                       s.description,
                       s.signature
                FROM signals s
                JOIN interfaces i ON s.interface_id = i.id
                JOIN objects o ON i.object_id = o.id
                WHERE s.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS signals_au AFTER UPDATE ON signals BEGIN
                INSERT INTO signals_fts(signals_fts, rowid) VALUES('delete', OLD.id);
                INSERT INTO signals_fts(rowid, service_name, object_path, interface_name,
                                       signal_name, description, signature)
                SELECT s.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name,
                       s.signal_name,
                       s.description,
                       s.signature
                FROM signals s
                JOIN interfaces i ON s.interface_id = i.id
                JOIN objects o ON i.object_id = o.id
                WHERE s.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS interfaces_ai AFTER INSERT ON interfaces BEGIN
                INSERT INTO interfaces_fts(rowid, service_name, object_path, interface_name)
                SELECT i.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name
                FROM interfaces i
                JOIN objects o ON i.object_id = o.id
                WHERE i.id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS interfaces_au AFTER UPDATE ON interfaces BEGIN
                INSERT INTO interfaces_fts(interfaces_fts, rowid) VALUES('delete', OLD.id);
                INSERT INTO interfaces_fts(rowid, service_name, object_path, interface_name)
                SELECT i.id,
                       o.service_name,
                       o.object_path,
                       i.interface_name
                FROM interfaces i
                JOIN objects o ON i.object_id = o.id
                WHERE i.id = NEW.id;
            END;

            -- Index for performance
            CREATE INDEX IF NOT EXISTS idx_objects_service ON objects(service_name);
            CREATE INDEX IF NOT EXISTS idx_interfaces_object ON interfaces(object_id);
            CREATE INDEX IF NOT EXISTS idx_methods_interface ON methods(interface_id);
            CREATE INDEX IF NOT EXISTS idx_properties_interface ON properties(interface_id);
            CREATE INDEX IF NOT EXISTS idx_signals_interface ON signals(interface_id);

            -- Statistics table
            CREATE TABLE IF NOT EXISTS index_stats (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                total_services INTEGER NOT NULL,
                total_objects INTEGER NOT NULL,
                total_interfaces INTEGER NOT NULL,
                total_methods INTEGER NOT NULL,
                total_properties INTEGER NOT NULL,
                total_signals INTEGER NOT NULL,
                scan_duration_seconds REAL NOT NULL,
                indexed_at INTEGER NOT NULL
            );
            "#,
        )?;

        let scanner = Arc::new(ServiceScanner::new());

        Ok(Self {
            conn: Arc::new(RwLock::new(conn)),
            scanner,
        })
    }

    /// Build complete index of DBus system
    pub async fn build_index(&self, bus_type: BusType) -> Result<IndexStatistics> {
        info!("🔍 Starting DBus index build for {:?} bus", bus_type);
        let start = std::time::Instant::now();

        // Discover all services
        let services = self.scanner.list_services(bus_type).await?;
        info!("   Found {} services to index", services.len());

        let mut total_objects = 0;
        let mut total_interfaces = 0;
        let mut total_methods = 0;
        let mut total_properties = 0;
        let mut total_signals = 0;

        let timestamp = Utc::now().timestamp();

        for (idx, service_info) in services.iter().enumerate() {
            if (idx + 1) % 10 == 0 {
                info!("   Progress: {}/{} services", idx + 1, services.len());
            }

            match self
                .index_service(bus_type, &service_info.name, timestamp)
                .await
            {
                Ok(stats) => {
                    total_objects += stats.0;
                    total_interfaces += stats.1;
                    total_methods += stats.2;
                    total_properties += stats.3;
                    total_signals += stats.4;
                }
                Err(e) => {
                    warn!("Failed to index service {}: {}", service_info.name, e);
                }
            }
        }

        let duration = start.elapsed().as_secs_f64();

        let stats = IndexStatistics {
            total_services: services.len(),
            total_objects,
            total_interfaces,
            total_methods,
            total_properties,
            total_signals,
            scan_duration_seconds: duration,
            indexed_at: timestamp,
        };

        // Store statistics
        self.store_statistics(&stats)?;

        info!("✅ Index build complete in {:.2}s", duration);
        info!("   Services: {}", stats.total_services);
        info!("   Objects: {}", stats.total_objects);
        info!("   Methods: {}", stats.total_methods);
        info!("   Properties: {}", stats.total_properties);

        Ok(stats)
    }

    /// Index a single service
    async fn index_service(
        &self,
        bus_type: BusType,
        service_name: &str,
        timestamp: i64,
    ) -> Result<(usize, usize, usize, usize, usize)> {
        let mut total_objects = 0;
        let mut total_interfaces = 0;
        let mut total_methods = 0;
        let mut total_properties = 0;
        let mut total_signals = 0;

        // Store service
        {
            let conn = self.conn.write().map_err(|e| anyhow::anyhow!("{}", e))?;
            conn.execute(
                "INSERT OR REPLACE INTO services (service_name, indexed_at) VALUES (?1, ?2)",
                params![service_name, timestamp],
            )?;
        }

        // Try common root paths (most services expose objects at "/" or service-specific paths)
        let service_path = format!("/{}", service_name.replace('.', "/"));
        let common_paths = vec!["/".to_string(), service_path];

        for object_path in &common_paths {
            match self
                .scanner
                .introspect(bus_type, service_name, object_path)
                .await
            {
                Ok(object_info) => {
                    self.store_object(service_name, &object_info, timestamp)?;
                    total_objects += 1;
                    total_interfaces += object_info.interfaces.len();

                    for interface in &object_info.interfaces {
                        total_methods += interface.methods.len();
                        total_properties += interface.properties.len();
                        total_signals += interface.signals.len();
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to introspect {}:{}: {}",
                        service_name, object_path, e
                    );
                }
            }
        }

        Ok((
            total_objects,
            total_interfaces,
            total_methods,
            total_properties,
            total_signals,
        ))
    }

    /// Store object and its interfaces in the database
    fn store_object(
        &self,
        service_name: &str,
        object_info: &ObjectInfo,
        timestamp: i64,
    ) -> Result<()> {
        let conn = self.conn.write().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Insert object
        conn.execute(
            "INSERT INTO objects (service_name, object_path, indexed_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(service_name, object_path)
             DO UPDATE SET indexed_at = excluded.indexed_at",
            params![service_name, &object_info.path, timestamp],
        )?;

        let object_id: i64 = conn.query_row(
            "SELECT id FROM objects WHERE service_name = ?1 AND object_path = ?2",
            params![service_name, &object_info.path],
            |row| row.get(0),
        )?;

        // Insert interfaces and their members
        for interface in &object_info.interfaces {
            conn.execute(
                "INSERT INTO interfaces (object_id, interface_name, indexed_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(object_id, interface_name)
                 DO UPDATE SET indexed_at = excluded.indexed_at",
                params![object_id, &interface.name, timestamp],
            )?;

            let interface_id: i64 = conn.query_row(
                "SELECT id FROM interfaces WHERE object_id = ?1 AND interface_name = ?2",
                params![object_id, &interface.name],
                |row| row.get(0),
            )?;

            // Insert methods
            for method in &interface.methods {
                let input_sig = method
                    .in_args
                    .iter()
                    .map(|a| format!("{}:{}", a.name.as_deref().unwrap_or("arg"), a.signature))
                    .collect::<Vec<_>>()
                    .join(", ");
                let output_sig = method
                    .out_args
                    .iter()
                    .map(|a| format!("{}:{}", a.name.as_deref().unwrap_or("arg"), a.signature))
                    .collect::<Vec<_>>()
                    .join(", ");

                conn.execute(
                    "INSERT INTO methods
                     (interface_id, method_name, input_signature, output_signature, description)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(interface_id, method_name)
                     DO UPDATE SET
                        input_signature = excluded.input_signature,
                        output_signature = excluded.output_signature,
                        description = excluded.description",
                    params![
                        interface_id,
                        &method.name,
                        &input_sig,
                        &output_sig,
                        format!("{}.{}", interface.name, method.name)
                    ],
                )?;
            }

            // Insert properties
            for property in &interface.properties {
                let access_str = match property.access {
                    op_core::types::PropertyAccess::Read => "read",
                    op_core::types::PropertyAccess::Write => "write",
                    op_core::types::PropertyAccess::ReadWrite => "readwrite",
                };

                conn.execute(
                    "INSERT INTO properties
                     (interface_id, property_name, type_signature, access, description)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(interface_id, property_name)
                     DO UPDATE SET
                        type_signature = excluded.type_signature,
                        access = excluded.access,
                        description = excluded.description",
                    params![
                        interface_id,
                        &property.name,
                        &property.signature,
                        access_str,
                        format!("{}.{}", interface.name, property.name)
                    ],
                )?;
            }

            // Insert signals
            for signal in &interface.signals {
                let sig = signal
                    .args
                    .iter()
                    .map(|a| format!("{}:{}", a.name.as_deref().unwrap_or("arg"), a.signature))
                    .collect::<Vec<_>>()
                    .join(", ");

                conn.execute(
                    "INSERT INTO signals
                     (interface_id, signal_name, signature, description)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(interface_id, signal_name)
                     DO UPDATE SET
                        signature = excluded.signature,
                        description = excluded.description",
                    params![
                        interface_id,
                        &signal.name,
                        &sig,
                        format!("{}.{}", interface.name, signal.name)
                    ],
                )?;
            }
        }

        Ok(())
    }

    /// Store index statistics
    fn store_statistics(&self, stats: &IndexStatistics) -> Result<()> {
        let conn = self.conn.write().map_err(|e| anyhow::anyhow!("{}", e))?;

        conn.execute(
            "INSERT OR REPLACE INTO index_stats
             (id, total_services, total_objects, total_interfaces, total_methods,
              total_properties, total_signals, scan_duration_seconds, indexed_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stats.total_services,
                stats.total_objects,
                stats.total_interfaces,
                stats.total_methods,
                stats.total_properties,
                stats.total_signals,
                stats.scan_duration_seconds,
                stats.indexed_at,
            ],
        )?;

        Ok(())
    }

    /// Get index statistics
    pub fn get_statistics(&self) -> Result<Option<IndexStatistics>> {
        let conn = self.conn.read().map_err(|e| anyhow::anyhow!("{}", e))?;

        let result = conn.query_row(
            "SELECT total_services, total_objects, total_interfaces, total_methods,
                    total_properties, total_signals, scan_duration_seconds, indexed_at
             FROM index_stats WHERE id = 1",
            [],
            |row| {
                Ok(IndexStatistics {
                    total_services: row.get(0)?,
                    total_objects: row.get(1)?,
                    total_interfaces: row.get(2)?,
                    total_methods: row.get(3)?,
                    total_properties: row.get(4)?,
                    total_signals: row.get(5)?,
                    scan_duration_seconds: row.get(6)?,
                    indexed_at: row.get(7)?,
                })
            },
        );

        match result {
            Ok(stats) => Ok(Some(stats)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Search methods using FTS5
    pub fn search_methods(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.read().map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut stmt = conn.prepare(
            "SELECT service_name, object_path, interface_name, method_name,
                    description, rank
             FROM methods_fts
             WHERE methods_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![query, limit], |row| {
                Ok(SearchResult {
                    service: row.get(0)?,
                    object_path: row.get(1)?,
                    interface: row.get(2)?,
                    item_name: row.get(3)?,
                    item_type: "method".to_string(),
                    description: row.get(4)?,
                    relevance_score: row.get::<_, f64>(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Search properties using FTS5
    pub fn search_properties(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.read().map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut stmt = conn.prepare(
            "SELECT service_name, object_path, interface_name, property_name,
                    description, rank
             FROM properties_fts
             WHERE properties_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![query, limit], |row| {
                Ok(SearchResult {
                    service: row.get(0)?,
                    object_path: row.get(1)?,
                    interface: row.get(2)?,
                    item_name: row.get(3)?,
                    item_type: "property".to_string(),
                    description: row.get(4)?,
                    relevance_score: row.get::<_, f64>(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Search everything (methods, properties, signals, interfaces)
    pub fn search_all(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Search methods
        results.extend(self.search_methods(query, limit / 4)?);

        // Search properties
        results.extend(self.search_properties(query, limit / 4)?);

        // Search signals
        let conn = self.conn.read().map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut stmt = conn.prepare(
            "SELECT service_name, object_path, interface_name, signal_name,
                    description, rank
             FROM signals_fts
             WHERE signals_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let signals = stmt
            .query_map(params![query, limit / 4], |row| {
                Ok(SearchResult {
                    service: row.get(0)?,
                    object_path: row.get(1)?,
                    interface: row.get(2)?,
                    item_name: row.get(3)?,
                    item_type: "signal".to_string(),
                    description: row.get(4)?,
                    relevance_score: row.get::<_, f64>(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        results.extend(signals);

        // Sort by relevance
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Trim to limit
        results.truncate(limit);

        Ok(results)
    }

    /// Clear the entire index
    pub fn clear_index(&self) -> Result<()> {
        let conn = self.conn.write().map_err(|e| anyhow::anyhow!("{}", e))?;

        conn.execute_batch(
            "DELETE FROM methods_fts;
             DELETE FROM properties_fts;
             DELETE FROM signals_fts;
             DELETE FROM interfaces_fts;
             DELETE FROM signals;
             DELETE FROM properties;
             DELETE FROM methods;
             DELETE FROM interfaces;
             DELETE FROM objects;
             DELETE FROM services;
             DELETE FROM index_stats;",
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_indexer_creation() {
        let indexer = DbusIndexer::new(":memory:").await.unwrap();
        let stats = indexer.get_statistics().unwrap();
        assert!(stats.is_none());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/lib.rs">
//! op-introspection: DBus introspection capabilities
//!
//! This crate provides:
//! - Service discovery
//! - Interface introspection
//! - XML parsing to JSON-serializable structures
//! - Caching of introspection results
//! - FTS5 full-text search indexer for semantic DBus queries
//!
//! All introspection results are returned as structs that implement
//! Serialize/Deserialize for easy JSON conversion in the RPC layer.

pub mod cache;
pub mod indexer;
pub mod indexer_manager;
pub mod parser;
pub mod projection;
pub mod scanner;

pub use cache::IntrospectionCache;
pub use indexer::{DbusIndexer, IndexStatistics, SearchResult};
pub use indexer_manager::IndexerManager;
pub use parser::IntrospectionParser;
pub use projection::DbusProjection;
pub use scanner::ServiceScanner;

use op_core::error::Result;
use op_core::types::{BusType, ObjectInfo, ServiceInfo};
use simd_json::ValueBuilder;
use std::sync::Arc;

/// High-level introspection service
///
/// Provides DBus introspection with results as JSON-serializable structs.
pub struct IntrospectionService {
    scanner: ServiceScanner,
    cache: Arc<IntrospectionCache>,
}

impl IntrospectionService {
    /// Create a new introspection service
    pub fn new() -> Self {
        Self {
            scanner: ServiceScanner::new(),
            cache: Arc::new(IntrospectionCache::new()),
        }
    }

    /// List all services on a bus (returns JSON-serializable structs)
    pub async fn list_services(&self, bus_type: BusType) -> Result<Vec<ServiceInfo>> {
        self.scanner.list_services(bus_type).await
    }

    /// List all services as JSON
    pub async fn list_services_json(&self, bus_type: BusType) -> Result<simd_json::OwnedValue> {
        let services = self.list_services(bus_type).await?;
        Ok(simd_json::serde::to_owned_value(services).unwrap_or(simd_json::OwnedValue::null()))
    }

    /// Introspect a service (returns JSON-serializable struct)
    pub async fn introspect(
        &self,
        bus_type: BusType,
        service: &str,
        path: &str,
    ) -> Result<ObjectInfo> {
        // Check cache first
        if let Some(cached) = self.cache.get(bus_type, service, path).await {
            return Ok(cached);
        }

        // Perform introspection
        let info = self.scanner.introspect(bus_type, service, path).await?;

        // Cache the result
        self.cache.set(bus_type, service, path, info.clone()).await;

        Ok(info)
    }

    /// Introspect a service and return as JSON
    pub async fn introspect_json(
        &self,
        bus_type: BusType,
        service: &str,
        path: &str,
    ) -> Result<simd_json::OwnedValue> {
        let info = self.introspect(bus_type, service, path).await?;
        Ok(simd_json::serde::to_owned_value(info).unwrap_or(simd_json::OwnedValue::null()))
    }

    /// Get cache reference
    pub fn cache(&self) -> Arc<IntrospectionCache> {
        Arc::clone(&self.cache)
    }
}

impl Default for IntrospectionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Prelude for convenient imports
pub mod prelude {
    pub use super::{
        DbusIndexer, DbusProjection, IndexStatistics, IndexerManager, IntrospectionCache,
        IntrospectionParser, IntrospectionService, SearchResult, ServiceScanner,
    };
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/mod.rs">
// Comprehensive system introspection
// Discovers D-Bus services, non-D-Bus services, and conversion opportunities

mod cpu_features;
pub use cpu_features::*;

mod hierarchical;
pub use hierarchical::*;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use zbus::Connection;

/// System introspection report
#[derive(Debug, Serialize, Deserialize)]
pub struct IntrospectionReport {
    /// D-Bus services currently managed by op-dbus
    pub managed_dbus_services: Vec<DbusServiceInfo>,

    /// D-Bus services discovered but not yet managed
    pub unmanaged_dbus_services: Vec<DbusServiceInfo>,

    /// Non-D-Bus systemd services that could be converted
    pub conversion_candidates: Vec<ConversionCandidate>,

    /// Kernel and hardware configuration
    pub system_config: SystemConfiguration,

    /// Summary statistics
    pub summary: IntrospectionSummary,
}

/// System-level configuration (kernel parameters, CPU settings, etc.)
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemConfiguration {
    /// Kernel command line parameters
    pub kernel_cmdline: Vec<String>,

    /// CPU vulnerability mitigations status
    pub cpu_mitigations: Vec<CpuMitigation>,

    /// CPU features and BIOS locks analysis
    pub cpu_features: Option<CpuFeatureAnalysis>,

    /// Loaded kernel modules
    pub loaded_modules: Vec<String>,

    /// QEMU/KVM configuration (if applicable)
    pub virtualization: Option<VirtualizationConfig>,

    /// Hardware model info (for BIOS workarounds)
    pub hardware: HardwareInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuMitigation {
    pub vulnerability: String, // e.g., "spectre_v2", "meltdown"
    pub status: String,        // e.g., "Mitigation: ...", "Vulnerable", "Not affected"
    pub mitigation_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualizationConfig {
    pub hypervisor: String,    // "kvm", "qemu", "xen", etc.
    pub vm_count: usize,       // Number of VMs
    pub cpu_passthrough: bool, // Host CPU features passed to guests
    pub nested_virt: bool,     // Nested virtualization enabled
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub vendor: String,            // e.g., "Samsung", "Dell", "HP"
    pub model: String,             // e.g., "360 Pro", "XPS 13"
    pub bios_version: String,      // For tracking buggy BIOS
    pub known_issues: Vec<String>, // Known hardware issues
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbusServiceInfo {
    pub service_name: String,
    pub bus_type: String, // "system" or "session"
    pub object_path: String,
    pub interfaces: Vec<InterfaceInfo>,
    pub management_status: ManagementStatus,
    pub recommended_plugin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub methods: Vec<String>,
    pub properties: Vec<String>,
    pub signals: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ManagementStatus {
    /// Managed by built-in plugin
    ManagedBuiltIn { plugin_name: String },

    /// Managed by auto-generated plugin
    ManagedAuto,

    /// Discovered but not managed
    Unmanaged { reason: String },

    /// Could be managed with new plugin
    ConversionCandidate,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversionCandidate {
    pub service_name: String,
    pub service_type: String,      // systemd, docker, etc.
    pub current_interface: String, // how it's currently managed
    pub dbus_opportunity: String,  // why it could use D-Bus
    pub complexity: ConversionComplexity,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ConversionComplexity {
    Easy,   // Just needs wrapper
    Medium, // Requires some refactoring
    Hard,   // Significant work needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntrospectionSummary {
    pub total_dbus_services: usize,
    pub managed_services: usize,
    pub unmanaged_services: usize,
    pub conversion_candidates: usize,
    pub management_coverage: f32, // Percentage of services managed
}

pub struct SystemIntrospector;

impl Default for SystemIntrospector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemIntrospector {
    pub fn new() -> Self {
        Self {}
    }

    /// Generate comprehensive introspection report
    pub async fn introspect_system(&self) -> Result<IntrospectionReport> {
        println!("🔍 Introspecting system...\n");

        // Discover D-Bus services
        let (system_services, session_services) = self.discover_dbus_services().await?;

        // Categorize D-Bus services
        let (managed, unmanaged) = self.categorize_dbus_services(system_services, session_services);

        // Find non-D-Bus services that could be converted
        let candidates = self.find_conversion_candidates().await?;

        // Gather system configuration (kernel, CPU, hardware)
        let system_config = self.gather_system_config().await?;

        let summary = IntrospectionSummary {
            total_dbus_services: managed.len() + unmanaged.len(),
            managed_services: managed.len(),
            unmanaged_services: unmanaged.len(),
            conversion_candidates: candidates.len(),
            management_coverage: if managed.len() + unmanaged.len() > 0 {
                (managed.len() as f32 / (managed.len() + unmanaged.len()) as f32) * 100.0
            } else {
                0.0
            },
        };

        Ok(IntrospectionReport {
            managed_dbus_services: managed,
            unmanaged_dbus_services: unmanaged,
            conversion_candidates: candidates,
            system_config,
            summary,
        })
    }

    /// Gather system-level configuration (kernel, CPU, hardware)
    async fn gather_system_config(&self) -> Result<SystemConfiguration> {
        println!("  🖥️  Gathering system configuration...");

        // Kernel command line
        let kernel_cmdline = self.read_kernel_cmdline()?;
        println!(
            "    ✓ Read kernel command line ({} parameters)",
            kernel_cmdline.len()
        );

        // CPU mitigations
        let cpu_mitigations = self.read_cpu_mitigations()?;
        println!(
            "    ✓ Checked CPU mitigations ({} vulnerabilities)",
            cpu_mitigations.len()
        );

        // Loaded modules
        let loaded_modules = self.read_loaded_modules()?;
        println!(
            "    ✓ Read loaded modules ({} modules)",
            loaded_modules.len()
        );

        // Virtualization config
        let virtualization = self.detect_virtualization()?;
        if virtualization.is_some() {
            println!("    ✓ Detected virtualization support");
        }

        // Hardware info
        let hardware = self.read_hardware_info()?;
        println!(
            "    ✓ Read hardware info ({} {})",
            hardware.vendor, hardware.model
        );

        // CPU feature analysis (detect hidden/locked BIOS features)
        let cpu_features = match CpuFeatureAnalyzer::new().analyze() {
            Ok(analysis) => {
                let locked_count = analysis.bios_locks.len();
                let disabled_count = analysis
                    .features
                    .iter()
                    .filter(|f| {
                        matches!(
                            f.status,
                            FeatureStatus::DisabledByBios | FeatureStatus::LockedByBios
                        )
                    })
                    .count();

                if locked_count > 0 || disabled_count > 0 {
                    println!(
                        "    ⚠️  CPU feature analysis: {} disabled, {} BIOS-locked",
                        disabled_count, locked_count
                    );
                } else {
                    println!("    ✓ CPU feature analysis complete");
                }
                Some(analysis)
            }
            Err(e) => {
                println!("    ⚠️  CPU feature analysis failed: {}", e);
                None
            }
        };

        Ok(SystemConfiguration {
            kernel_cmdline,
            cpu_mitigations,
            cpu_features,
            loaded_modules,
            virtualization,
            hardware,
        })
    }

    /// Read kernel command line from /proc/cmdline
    fn read_kernel_cmdline(&self) -> Result<Vec<String>> {
        let cmdline =
            std::fs::read_to_string("/proc/cmdline").context("Failed to read /proc/cmdline")?;

        Ok(cmdline.split_whitespace().map(|s| s.to_string()).collect())
    }

    /// Read CPU vulnerability mitigations from /sys
    fn read_cpu_mitigations(&self) -> Result<Vec<CpuMitigation>> {
        let vulnerabilities_dir = std::path::Path::new("/sys/devices/system/cpu/vulnerabilities");

        if !vulnerabilities_dir.exists() {
            return Ok(Vec::new());
        }

        let mut mitigations = Vec::new();

        for entry in std::fs::read_dir(vulnerabilities_dir)? {
            let entry = entry?;
            let vulnerability = entry.file_name().to_string_lossy().to_string();
            let status = std::fs::read_to_string(entry.path())?.trim().to_string();

            let mitigation_active =
                status.contains("Mitigation:") || status.contains("Not affected");

            mitigations.push(CpuMitigation {
                vulnerability,
                status,
                mitigation_active,
            });
        }

        Ok(mitigations)
    }

    /// Read loaded kernel modules from /proc/modules
    fn read_loaded_modules(&self) -> Result<Vec<String>> {
        let modules =
            std::fs::read_to_string("/proc/modules").context("Failed to read /proc/modules")?;

        Ok(modules
            .lines()
            .filter_map(|line| {
                // Format: "module_name size used_by_count ..."
                line.split_whitespace().next().map(|s| s.to_string())
            })
            .collect())
    }

    /// Detect virtualization configuration
    fn detect_virtualization(&self) -> Result<Option<VirtualizationConfig>> {
        // Check if KVM module is loaded
        let modules = self.read_loaded_modules()?;
        let has_kvm = modules.iter().any(|m| m.contains("kvm"));

        if !has_kvm {
            return Ok(None);
        }

        // Check for running QEMU/KVM VMs
        let vm_count = self.count_running_vms()?;

        // Check if CPU virtualization features are available
        let cpuinfo =
            std::fs::read_to_string("/proc/cpuinfo").context("Failed to read /proc/cpuinfo")?;
        let cpu_passthrough = cpuinfo.contains("vmx") || cpuinfo.contains("svm");

        // Check for nested virtualization
        let nested_virt = self.check_nested_virt()?;

        Ok(Some(VirtualizationConfig {
            hypervisor: "kvm".to_string(),
            vm_count,
            cpu_passthrough,
            nested_virt,
        }))
    }

    fn count_running_vms(&self) -> Result<usize> {
        // Count QEMU processes
        let output = Command::new("pgrep").arg("-c").arg("qemu").output();

        match output {
            Ok(out) if out.status.success() => {
                let count_str = String::from_utf8_lossy(&out.stdout);
                Ok(count_str.trim().parse().unwrap_or(0))
            }
            _ => Ok(0),
        }
    }

    fn check_nested_virt(&self) -> Result<bool> {
        // Check Intel nested virtualization
        if let Ok(contents) = std::fs::read_to_string("/sys/module/kvm_intel/parameters/nested") {
            if contents.trim() == "Y" || contents.trim() == "1" {
                return Ok(true);
            }
        }

        // Check AMD nested virtualization
        if let Ok(contents) = std::fs::read_to_string("/sys/module/kvm_amd/parameters/nested") {
            if contents.trim() == "1" {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Read hardware information
    fn read_hardware_info(&self) -> Result<HardwareInfo> {
        // Read DMI info
        let vendor = self
            .read_dmi_field("sys_vendor")
            .unwrap_or_else(|_| "Unknown".to_string());
        let model = self
            .read_dmi_field("product_name")
            .unwrap_or_else(|_| "Unknown".to_string());
        let bios_version = self
            .read_dmi_field("bios_version")
            .unwrap_or_else(|_| "Unknown".to_string());

        // Check for known problematic hardware
        let known_issues = self.check_known_hardware_issues(&vendor, &model);

        Ok(HardwareInfo {
            vendor,
            model,
            bios_version,
            known_issues,
        })
    }

    fn read_dmi_field(&self, field: &str) -> Result<String> {
        let path = format!("/sys/devices/virtual/dmi/id/{}", field);
        std::fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .context(format!("Failed to read {}", path))
    }

    fn check_known_hardware_issues(&self, vendor: &str, model: &str) -> Vec<String> {
        let mut issues = Vec::new();

        // Samsung 360 Pro known issues
        if vendor.contains("SAMSUNG") && model.contains("360") {
            issues.push("Buggy BIOS: Requires acpi=off kernel parameter".to_string());
            issues.push("Power management: Use intel_idle.max_cstate=1".to_string());
            issues.push("PCIe ASPM: Use pcie_aspm=off".to_string());
            issues.push("Graphics: May need i915.enable_psr=0".to_string());
        }

        // Dell XPS 13 9370 (Killer WiFi issues)
        if vendor.contains("Dell") && model.contains("XPS") && model.contains("9370") {
            issues.push("Killer WiFi: May need pcie_port_pm=off".to_string());
            issues.push("Thunderbolt: Check BIOS settings".to_string());
        }

        // Lenovo ThinkPad X1 Carbon Gen 7 (S3 sleep issues)
        if vendor.contains("LENOVO") && model.contains("X1 Carbon") {
            issues.push("S3 sleep: May need mem_sleep_default=deep in BIOS".to_string());
        }

        issues
    }

    /// Discover all D-Bus services on system and session buses
    async fn discover_dbus_services(&self) -> Result<(Vec<String>, Vec<String>)> {
        println!("  📡 Discovering D-Bus services...");

        // Use existing auto_plugin discovery for system services (if MCP enabled)
        #[cfg(feature = "mcp")]
        let system_services =
            match crate::state::auto_plugin::PluginDiscovery::discover_services().await {
                Ok(services) => {
                    println!("    ✓ Found {} services on system bus", services.len());
                    services
                }
                Err(e) => {
                    log::warn!("Failed to use auto_plugin discovery: {}, falling back", e);
                    // Fallback to direct discovery
                    let system_conn = Connection::system().await?;
                    let services = self.list_dbus_names(&system_conn).await?;
                    println!(
                        "    ✓ Found {} services on system bus (fallback)",
                        services.len()
                    );
                    services
                }
            };

        // Direct discovery when MCP is not available
        #[cfg(not(feature = "mcp"))]
        let system_services = {
            let system_conn = Connection::system().await?;
            let services = self.list_dbus_names(&system_conn).await?;
            println!("    ✓ Found {} services on system bus", services.len());
            services
        };

        // Session bus (may not exist in server environments)
        let session_services = match Connection::session().await {
            Ok(conn) => {
                let services = self.list_dbus_names(&conn).await?;
                println!("    ✓ Found {} services on session bus", services.len());
                services
            }
            Err(_) => {
                println!("    ⊗ Session bus not available");
                Vec::new()
            }
        };

        Ok((system_services, session_services))
    }

    /// List all D-Bus service names on a connection
    async fn list_dbus_names(&self, conn: &Connection) -> Result<Vec<String>> {
        use zbus::fdo::DBusProxy;

        let proxy = DBusProxy::new(conn).await?;
        let names = proxy.list_names().await?;

        // Filter out temporary names (starting with :) and convert to String
        Ok(names
            .into_iter()
            .filter(|name| !name.starts_with(':'))
            .filter(|name| name.as_str() != "org.freedesktop.DBus")
            .map(|name| name.to_string())
            .collect())
    }

    /// Categorize D-Bus services as managed or unmanaged
    fn categorize_dbus_services(
        &self,
        system_services: Vec<String>,
        session_services: Vec<String>,
    ) -> (Vec<DbusServiceInfo>, Vec<DbusServiceInfo>) {
        let mut managed = Vec::new();
        let mut unmanaged = Vec::new();

        // Process system services
        for service in system_services {
            let info = self.analyze_dbus_service(&service, "system");
            match &info.management_status {
                ManagementStatus::ManagedBuiltIn { .. } | ManagementStatus::ManagedAuto => {
                    managed.push(info);
                }
                _ => {
                    unmanaged.push(info);
                }
            }
        }

        // Process session services
        for service in session_services {
            let info = self.analyze_dbus_service(&service, "session");
            match &info.management_status {
                ManagementStatus::ManagedBuiltIn { .. } | ManagementStatus::ManagedAuto => {
                    managed.push(info);
                }
                _ => {
                    unmanaged.push(info);
                }
            }
        }

        (managed, unmanaged)
    }

    /// Analyze a single D-Bus service
    fn analyze_dbus_service(&self, service_name: &str, bus_type: &str) -> DbusServiceInfo {
        let management_status = if service_name == "org.freedesktop.systemd1" {
            ManagementStatus::ManagedBuiltIn {
                plugin_name: "systemd".to_string(),
            }
        } else if service_name == "org.freedesktop.login1" {
            ManagementStatus::ManagedBuiltIn {
                plugin_name: "login1".to_string(),
            }
        } else if self.can_auto_generate(service_name) {
            // Service can be managed by auto-generated plugin
            ManagementStatus::ManagedAuto
        } else {
            ManagementStatus::Unmanaged {
                reason: "No plugin available, not auto-discoverable".to_string(),
            }
        };

        let recommended_plugin = self.recommend_plugin(service_name);

        DbusServiceInfo {
            service_name: service_name.to_string(),
            bus_type: bus_type.to_string(),
            object_path: format!("/{}", service_name.replace('.', "/")),
            interfaces: Vec::new(), // Would introspect in full implementation
            management_status,
            recommended_plugin,
        }
    }

    /// Check if a service can be auto-generated (uses same logic as PluginDiscovery)
    fn can_auto_generate(&self, service_name: &str) -> bool {
        // Reuse the same logic from auto_plugin.rs
        if service_name.starts_with(':') {
            return false; // Temporary unique names
        }

        if service_name == "org.freedesktop.DBus" {
            return false; // DBus daemon itself
        }

        if service_name.starts_with("org.freedesktop.DBus.") {
            return false; // DBus internal services
        }

        // Well-known freedesktop services can be auto-generated
        if service_name.starts_with("org.freedesktop.") {
            return true;
        }

        // Custom services with reverse domain names
        if service_name.contains('.') && !service_name.starts_with("org.freedesktop.systemd1.") {
            return true;
        }

        false
    }

    /// Recommend a plugin for a D-Bus service
    fn recommend_plugin(&self, service_name: &str) -> Option<String> {
        match service_name {
            "org.freedesktop.NetworkManager" => Some("networkmanager".to_string()),
            "org.freedesktop.PackageKit" => Some("packagekit".to_string()),
            "org.freedesktop.UPower" => Some("upower".to_string()),
            "org.freedesktop.UDisks2" => Some("udisks2".to_string()),
            "org.bluez" => Some("bluetooth".to_string()),
            _ => None,
        }
    }

    /// Find non-D-Bus services that could be converted
    async fn find_conversion_candidates(&self) -> Result<Vec<ConversionCandidate>> {
        println!("  🔍 Analyzing non-D-Bus services...");

        let mut candidates = Vec::new();

        // Get all systemd units
        let units = self.get_systemd_units()?;
        println!("    ✓ Found {} systemd units", units.len());

        // Analyze each unit for conversion potential
        for unit in units {
            if let Some(candidate) = self.analyze_for_conversion(&unit) {
                candidates.push(candidate);
            }
        }

        println!("    ✓ Found {} conversion candidates", candidates.len());

        Ok(candidates)
    }

    /// Get all systemd units
    fn get_systemd_units(&self) -> Result<Vec<String>> {
        let output = Command::new("systemctl")
            .args([
                "list-units",
                "--type=service",
                "--all",
                "--no-pager",
                "--no-legend",
            ])
            .output()
            .context("Failed to execute systemctl")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let units: Vec<String> = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if !parts.is_empty() {
                    Some(parts[0].to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(units)
    }

    /// Analyze a systemd unit for D-Bus conversion potential
    fn analyze_for_conversion(&self, unit: &str) -> Option<ConversionCandidate> {
        // Skip units already managed via D-Bus
        if unit.contains("dbus") || unit.contains("systemd") {
            return None;
        }

        // Check if this is a candidate for D-Bus conversion
        let (dbus_opportunity, complexity) = match unit {
            // Package managers
            u if u.contains("packagekit") => (
                "PackageKit provides D-Bus interface for package management".to_string(),
                ConversionComplexity::Easy,
            ),
            u if u.contains("apt") || u.contains("dnf") || u.contains("yum") => (
                "Could expose package management via D-Bus (use PackageKit)".to_string(),
                ConversionComplexity::Medium,
            ),

            // Network services
            u if u.contains("docker") => (
                "Docker could expose management API via D-Bus".to_string(),
                ConversionComplexity::Medium,
            ),
            u if u.contains("containerd") => (
                "Container runtime could benefit from D-Bus IPC".to_string(),
                ConversionComplexity::Hard,
            ),

            // Web servers
            u if u.contains("nginx") || u.contains("apache") || u.contains("httpd") => (
                "Web server status/reload could be exposed via D-Bus".to_string(),
                ConversionComplexity::Easy,
            ),

            // Databases
            u if u.contains("postgres") || u.contains("mysql") || u.contains("mariadb") => (
                "Database management could benefit from D-Bus interface".to_string(),
                ConversionComplexity::Medium,
            ),

            // VPN services
            u if u.contains("wg-quick") || u.contains("openvpn") || u.contains("wireguard") => (
                "VPN control could be exposed via D-Bus".to_string(),
                ConversionComplexity::Medium,
            ),

            // Backup services
            u if u.contains("restic") || u.contains("borgbackup") || u.contains("backup") => (
                "Backup service could expose status via D-Bus".to_string(),
                ConversionComplexity::Easy,
            ),

            _ => return None, // Not a conversion candidate
        };

        Some(ConversionCandidate {
            service_name: unit.to_string(),
            service_type: "systemd".to_string(),
            current_interface: "systemctl / systemd D-Bus (indirect)".to_string(),
            dbus_opportunity,
            complexity,
        })
    }

    /// Print human-readable report
    pub fn print_report(&self, report: &IntrospectionReport) {
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("   op-dbus System Introspection Report");
        println!("═══════════════════════════════════════════════════════════════\n");

        // Hardware Info
        println!("🖥️  HARDWARE & SYSTEM CONFIGURATION");
        println!("──────────────────────────────────────────────────────────────");
        println!("  Vendor:       {}", report.system_config.hardware.vendor);
        println!("  Model:        {}", report.system_config.hardware.model);
        println!(
            "  BIOS Version: {}",
            report.system_config.hardware.bios_version
        );

        if !report.system_config.hardware.known_issues.is_empty() {
            println!("\n  ⚠️  KNOWN HARDWARE ISSUES:");
            for issue in &report.system_config.hardware.known_issues {
                println!("    • {}", issue);
            }
        }

        // Virtualization info
        if let Some(virt) = &report.system_config.virtualization {
            println!(
                "\n  Virtualization: {} ({} VMs running)",
                virt.hypervisor, virt.vm_count
            );
            println!(
                "    CPU Passthrough: {}",
                if virt.cpu_passthrough {
                    "✓ Yes"
                } else {
                    "⊗ No"
                }
            );
            println!(
                "    Nested Virt:     {}",
                if virt.nested_virt {
                    "✓ Yes"
                } else {
                    "⊗ No"
                }
            );
        }
        println!();

        // CPU Mitigations
        println!("🛡️  CPU VULNERABILITY MITIGATIONS");
        println!("──────────────────────────────────────────────────────────────");
        let active_mitigations = report
            .system_config
            .cpu_mitigations
            .iter()
            .filter(|m| m.mitigation_active)
            .count();
        let total_vulnerabilities = report.system_config.cpu_mitigations.len();
        println!(
            "  {} of {} vulnerabilities mitigated\n",
            active_mitigations, total_vulnerabilities
        );

        for mitigation in &report.system_config.cpu_mitigations {
            let status_icon = if mitigation.mitigation_active {
                "✓"
            } else {
                "⚠"
            };
            let vuln_name = mitigation.vulnerability.replace('_', " ");
            println!("  {} {}: {}", status_icon, vuln_name, mitigation.status);
        }
        println!();

        // CPU Features and BIOS Locks
        if let Some(cpu_analysis) = &report.system_config.cpu_features {
            println!("🔓 CPU FEATURES & BIOS LOCKS");
            println!("──────────────────────────────────────────────────────────────");
            println!(
                "  CPU: {} (Family {})",
                cpu_analysis.cpu_model.model_name, cpu_analysis.cpu_model.family
            );
            println!("  Microcode: {}\n", cpu_analysis.cpu_model.microcode);

            // Show disabled/locked features first (most important)
            let critical_features: Vec<_> = cpu_analysis
                .features
                .iter()
                .filter(|f| {
                    matches!(
                        f.status,
                        FeatureStatus::DisabledByBios | FeatureStatus::LockedByBios
                    )
                })
                .collect();

            if !critical_features.is_empty() {
                println!("  ⚠️  DISABLED/LOCKED FEATURES:");
                for feature in critical_features {
                    let status_icon = match feature.status {
                        FeatureStatus::LockedByBios => "🔒",
                        FeatureStatus::DisabledByBios => "⊗",
                        _ => "?",
                    };
                    let status_text = match feature.status {
                        FeatureStatus::LockedByBios => "BIOS Locked",
                        FeatureStatus::DisabledByBios => "Disabled by BIOS",
                        _ => "Unknown",
                    };
                    println!(
                        "    {} {} ({}): {}",
                        status_icon, feature.name, feature.technical_name, status_text
                    );
                }
                println!();
            }

            // Show BIOS locks with details
            if !cpu_analysis.bios_locks.is_empty() {
                println!("  🔒 BIOS LOCKS DETECTED:");
                for lock in &cpu_analysis.bios_locks {
                    println!("    Register: {}", lock.register);
                    println!("      Lock Bit: {}", lock.lock_bit);
                    println!("      Affects: {}", lock.affected_features.join(", "));
                    println!("      Method: {}", lock.lock_method);
                    println!();
                }
            }

            // Show enabled features
            let enabled_features: Vec<_> = cpu_analysis
                .features
                .iter()
                .filter(|f| matches!(f.status, FeatureStatus::Enabled))
                .collect();

            if !enabled_features.is_empty() {
                println!("  ✓ ENABLED FEATURES:");
                for feature in enabled_features {
                    println!("    ✓ {} ({})", feature.name, feature.technical_name);
                }
                println!();
            }

            // Show recommendations
            if !cpu_analysis.recommendations.is_empty() {
                println!("  💡 RECOMMENDATIONS:");
                for rec in &cpu_analysis.recommendations {
                    let priority_icon = match rec.priority {
                        Priority::Critical => "🔴",
                        Priority::High => "🟠",
                        Priority::Medium => "🟡",
                        Priority::Low => "🟢",
                    };
                    println!(
                        "    {} {} - {:?} Priority",
                        priority_icon, rec.feature, rec.priority
                    );
                    println!("       Reason: {}", rec.reason);
                    println!("       Benefit: {}", rec.benefit);
                    println!("       Action: {}", rec.action);
                    println!();
                }
            }
        }

        // Kernel Parameters (show important ones)
        println!("⚙️  KERNEL CONFIGURATION");
        println!("──────────────────────────────────────────────────────────────");

        // Filter interesting kernel parameters
        let interesting_params: Vec<_> = report
            .system_config
            .kernel_cmdline
            .iter()
            .filter(|p| {
                p.contains("acpi")
                    || p.contains("idle")
                    || p.contains("aspm")
                    || p.contains("mitigation")
                    || p.contains("pci")
                    || p.contains("i915")
                    || p.contains("kvm")
            })
            .collect();

        if !interesting_params.is_empty() {
            println!(
                "  Critical kernel parameters ({}total):\n",
                report.system_config.kernel_cmdline.len()
            );
            for param in &interesting_params {
                println!("    • {}", param);
            }
        } else {
            println!("  No critical kernel parameters detected");
            println!(
                "  (Total {} parameters)",
                report.system_config.kernel_cmdline.len()
            );
        }
        println!();

        // Summary
        println!("📊 SERVICE MANAGEMENT SUMMARY");
        println!("──────────────────────────────────────────────────────────────");
        println!(
            "  Total D-Bus services:    {}",
            report.summary.total_dbus_services
        );
        println!(
            "  ✓ Managed services:      {}",
            report.summary.managed_services
        );
        println!(
            "  ⊗ Unmanaged services:    {}",
            report.summary.unmanaged_services
        );
        println!(
            "  🔄 Conversion candidates: {}",
            report.summary.conversion_candidates
        );
        println!(
            "  Coverage:                {:.1}%\n",
            report.summary.management_coverage
        );

        // Managed D-Bus services
        if !report.managed_dbus_services.is_empty() {
            println!("✅ MANAGED D-BUS SERVICES");
            println!("──────────────────────────────────────────────────────────────");

            let built_in: Vec<_> = report
                .managed_dbus_services
                .iter()
                .filter(|s| matches!(s.management_status, ManagementStatus::ManagedBuiltIn { .. }))
                .collect();

            let auto_gen: Vec<_> = report
                .managed_dbus_services
                .iter()
                .filter(|s| matches!(s.management_status, ManagementStatus::ManagedAuto))
                .collect();

            // Show built-in plugins first
            if !built_in.is_empty() {
                println!("  Built-in Plugins (read-write):");
                for service in built_in {
                    if let ManagementStatus::ManagedBuiltIn { plugin_name } =
                        &service.management_status
                    {
                        println!("    ✓ {} → {}", service.service_name, plugin_name);
                    }
                }
                println!();
            }

            // Show auto-generated plugins
            if !auto_gen.is_empty() {
                println!("  Auto-Generated Plugins (read-only):");
                for service in auto_gen {
                    println!("    🤖 {}", service.service_name);
                    if let Some(plugin) = &service.recommended_plugin {
                        println!(
                            "       Can become: {} plugin (with semantic mapping)",
                            plugin
                        );
                    }
                }
                println!();
                println!("  ℹ️  Auto-generated plugins can query state but cannot apply changes.");
                println!("     To enable writes, create a dedicated plugin or semantic mapping.");
            }
            println!();
        }

        // Unmanaged D-Bus services
        if !report.unmanaged_dbus_services.is_empty() {
            println!("🔍 UNMANAGED D-BUS SERVICES (Conversion Opportunity)");
            println!("──────────────────────────────────────────────────────────────");
            for service in &report.unmanaged_dbus_services {
                println!("  ⊗ {} ({})", service.service_name, service.bus_type);
                if let Some(plugin) = &service.recommended_plugin {
                    println!("    → Recommended plugin: {}", plugin);
                }
                if let ManagementStatus::Unmanaged { reason } = &service.management_status {
                    println!("    Reason: {}", reason);
                }
            }
            println!();
        }

        // Conversion candidates
        if !report.conversion_candidates.is_empty() {
            println!("🔄 NON-D-BUS SERVICES (Could Be Converted)");
            println!("──────────────────────────────────────────────────────────────");
            for candidate in &report.conversion_candidates {
                let complexity_emoji = match candidate.complexity {
                    ConversionComplexity::Easy => "🟢",
                    ConversionComplexity::Medium => "🟡",
                    ConversionComplexity::Hard => "🔴",
                };
                println!(
                    "  {} {} ({})",
                    complexity_emoji, candidate.service_name, candidate.service_type
                );
                println!("    Current: {}", candidate.current_interface);
                println!("    Opportunity: {}", candidate.dbus_opportunity);
            }
            println!();
        }

        println!("═══════════════════════════════════════════════════════════════\n");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/parser.rs">
//! Introspection XML parser

use op_core::{ObjectInfo, Result};

pub struct IntrospectionParser;

impl IntrospectionParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, _xml: &str, path: &str) -> Result<ObjectInfo> {
        // Parsing is done in scanner module
        Ok(ObjectInfo {
            path: path.to_string(),
            interfaces: Vec::new(),
            children: Vec::new(),
        })
    }
}

impl Default for IntrospectionParser {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/projection.rs">
use anyhow::Result;
use futures::{stream::iter, StreamExt};
use sha2::{Digest, Sha256};
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;

use crate::IntrospectionService;
pub use op_core::types::BusType;

use op_blockchain::StreamingBlockchain;
use op_core::types::ObjectSchemaRef;

// ============================================================================
// D-BUS PROJECTION
// ============================================================================

/// D-Bus Projection - delegates to op-introspection for all introspection
///
/// All results are JSON-serializable. No raw XML exposed.
///
/// For restorable system configs:
/// - Uses StreamingBlockchain.write_state() for BTRFS state subvolume
/// - Triggers blockchain block to signal state change for backup
#[derive(Clone)]
pub struct DbusProjection {
    introspection: Arc<IntrospectionService>,
    blockchain: Option<Arc<AsyncRwLock<StreamingBlockchain>>>,
}

impl DbusProjection {
    /// Create a new D-Bus projection
    pub fn new() -> Self {
        Self {
            introspection: Arc::new(IntrospectionService::new()),
            blockchain: None,
        }
    }

    /// Create with shared introspection service
    pub fn with_service(introspection: Arc<IntrospectionService>) -> Self {
        Self {
            introspection,
            blockchain: None,
        }
    }

    /// Attach a StreamingBlockchain for restorable state persistence
    /// JSON writes go to state_subvol (BTRFS) and trigger blockchain backup
    pub fn with_blockchain(mut self, blockchain: Arc<AsyncRwLock<StreamingBlockchain>>) -> Self {
        self.blockchain = Some(blockchain);
        self
    }

    /// List services on a bus - returns JSON
    pub async fn list_services(&self, bus_type: BusType) -> Result<Value> {
        let json = self.introspection.list_services_json(bus_type).await?;
        Ok(json)
    }

    /// Introspect a service/object - returns JSON
    ///
    /// XML is parsed internally by op-introspection; this returns pure JSON
    pub async fn introspect(&self, bus_type: BusType, service: &str, path: &str) -> Result<Value> {
        let json = self
            .introspection
            .introspect_json(bus_type, service, path)
            .await?;
        Ok(json)
    }

    /// Introspect and get structured ObjectInfo (for plugin schema linking)
    pub async fn introspect_object(
        &self,
        bus_type: BusType,
        service: &str,
        path: &str,
    ) -> Result<op_core::types::ObjectInfo> {
        let info = self
            .introspection
            .introspect(bus_type, service, path)
            .await?;
        Ok(info)
    }

    /// Introspect and persist to BTRFS state subvolume (restorable system config)
    ///
    /// This writes JSON to the blockchain's state_subvol AND triggers a blockchain
    /// block to signal that restorable state has changed (for backup)
    ///
    /// Only use this for managed services that should be restored in disaster recovery.
    pub async fn introspect_and_persist(
        &self,
        bus_type: BusType,
        service: &str,
        path: &str,
    ) -> Result<ObjectSchemaRef> {
        let json = self.introspect(bus_type, service, path).await?;

        // Compute schema hash
        let json_str = simd_json::to_string_pretty(&json)?;
        let schema_hash = {
            let mut hasher = Sha256::new();
            hasher.update(json_str.as_bytes());
            hex::encode(hasher.finalize())
        };

        let state_key = format!(
            "dbus/{}/{}",
            service.replace('.', "_"),
            path.replace('/', "_")
        );

        // Write to BTRFS state subvolume AND trigger blockchain block
        if let Some(blockchain) = &self.blockchain {
            let bc = blockchain.read().await;

            // Write JSON to state_subvol (restorable system config)
            bc.write_state(&state_key, &json).await?;

            // Trigger blockchain block to signal state change for backup
            bc.add_event(op_blockchain::BlockEvent::new(
                "dbus.schema.update",
                &schema_hash,
                simd_json::json!({"service": service, "path": path}),
            ))
            .await?;

            tracing::debug!(
                "Persisted D-Bus schema to BTRFS state subvol: {}",
                state_key
            );
        }

        Ok(ObjectSchemaRef::new(
            "dbus_interface",
            service,
            path,
            schema_hash,
        ))
    }

    /// Discover and persist all interfaces for a managed service
    /// (e.g., PackageKit, systemd, NetworkManager)
    pub async fn discover_service(
        &self,
        bus_type: BusType,
        service: &str,
    ) -> Result<Vec<ObjectSchemaRef>> {
        let root_info = self.introspect_object(bus_type, service, "/").await?;
        let schemas = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Persist root
        if let Ok(schema) = self.introspect_and_persist(bus_type, service, "/").await {
            schemas.lock().await.push(schema);
        }

        let self_clone = self.clone();

        // Recursively discover children in parallel
        iter(root_info.children)
            .for_each_concurrent(None, |child: String| {
                let child_path = if child.starts_with('/') {
                    child.clone()
                } else {
                    format!("/{}", child)
                };
                let schemas = schemas.clone();
                let self_clone = self_clone.clone();

                async move {
                    if let Ok(schema) = self_clone
                        .introspect_and_persist(bus_type, service, &child_path)
                        .await
                    {
                        schemas.lock().await.push(schema);
                    }
                }
            })
            .await;

        let final_schemas = Arc::try_unwrap(schemas).unwrap().into_inner();
        tracing::info!(
            "Discovered {} schemas for service {} (BTRFS state + blockchain trigger)",
            final_schemas.len(),
            service
        );
        Ok(final_schemas)
    }

    /// Get access to underlying introspection service
    pub fn introspection_service(&self) -> Arc<IntrospectionService> {
        Arc::clone(&self.introspection)
    }
}

impl Default for DbusProjection {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_type_display() {
        assert_eq!(format!("{:?}", BusType::System), "system");
        assert_eq!(format!("{:?}", BusType::Session), "session");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/src/scanner.rs">
//! DBus service scanning

use std::collections::HashMap;
use tracing::debug;

use op_core::{
    BusType, Error, InterfaceInfo, MethodInfo, ObjectInfo, PropertyInfo, Result, ServiceInfo,
    SignalInfo,
};

/// Service scanner for DBus
pub struct ServiceScanner {
    _cache: HashMap<(BusType, String), Vec<ServiceInfo>>,
}

impl ServiceScanner {
    pub fn new() -> Self {
        Self {
            _cache: HashMap::new(),
        }
    }

    /// List all services on a bus
    pub async fn list_services(&self, bus_type: BusType) -> Result<Vec<ServiceInfo>> {
        let connection = match bus_type {
            BusType::System => zbus::Connection::system().await?,
            BusType::Session => zbus::Connection::session().await?,
        };

        let proxy = zbus::fdo::DBusProxy::new(&connection).await?;
        let names = proxy.list_names().await?;

        let mut services = Vec::new();
        for name in names {
            let name_str = name.to_string();
            // Skip private names
            if name_str.starts_with(':') {
                continue;
            }

            services.push(ServiceInfo {
                name: name_str.clone(),
                bus_type,
                activatable: false,
                active: true,
                pid: None,
                uid: None,
            });
        }

        debug!("Found {} services on {:?} bus", services.len(), bus_type);
        Ok(services)
    }

    /// Introspect a specific service/path
    pub async fn introspect(
        &self,
        bus_type: BusType,
        service: &str,
        path: &str,
    ) -> Result<ObjectInfo> {
        let connection = match bus_type {
            BusType::System => zbus::Connection::system().await?,
            BusType::Session => zbus::Connection::session().await?,
        };

        let proxy = zbus::fdo::IntrospectableProxy::builder(&connection)
            .destination(service)?
            .path(path)?
            .build()
            .await?;

        let xml = proxy.introspect().await?;

        // Parse the XML
        let obj_info = parse_introspection_xml(&xml, path)?;

        debug!(
            "Introspected {} {} with {} interfaces",
            service,
            path,
            obj_info.interfaces.len()
        );
        Ok(obj_info)
    }
}

impl Default for ServiceScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse DBus introspection XML
fn parse_introspection_xml(xml: &str, path: &str) -> Result<ObjectInfo> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut interfaces = Vec::new();
    let mut children = Vec::new();
    let mut node_depth: usize = 0;

    let mut current_interface: Option<InterfaceInfo> = None;
    let mut current_method: Option<MethodInfo> = None;
    let mut current_signal: Option<SignalInfo> = None;
    let _current_property: Option<PropertyInfo> = None;
    let mut in_method = false;
    let mut in_signal = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");

                match name {
                    "node" => {
                        // Capture only direct children of the current node.
                        if node_depth == 1 {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    let child_name =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                    let child_path = if child_name.starts_with('/') {
                                        child_name
                                    } else if path == "/" {
                                        format!("/{}", child_name)
                                    } else {
                                        format!("{}/{}", path, child_name)
                                    };
                                    children.push(child_path);
                                }
                            }
                        }
                        node_depth += 1;
                    }
                    "interface" => {
                        if node_depth != 1 {
                            continue;
                        }
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                let iface_name = String::from_utf8_lossy(&attr.value).to_string();
                                current_interface = Some(InterfaceInfo {
                                    name: iface_name,
                                    methods: Vec::new(),
                                    properties: Vec::new(),
                                    signals: Vec::new(),
                                });
                            }
                        }
                    }
                    "method" => {
                        if node_depth != 1 {
                            continue;
                        }
                        in_method = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                current_method = Some(MethodInfo {
                                    name: String::from_utf8_lossy(&attr.value).to_string(),
                                    in_args: Vec::new(),
                                    out_args: Vec::new(),
                                    annotations: std::collections::HashMap::new(),
                                });
                            }
                        }
                    }
                    "signal" => {
                        if node_depth != 1 {
                            continue;
                        }
                        in_signal = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                current_signal = Some(SignalInfo {
                                    name: String::from_utf8_lossy(&attr.value).to_string(),
                                    args: Vec::new(),
                                });
                            }
                        }
                    }
                    "property" => {
                        if node_depth != 1 {
                            continue;
                        }
                        let mut prop_name = String::new();
                        let mut prop_type = String::new();
                        let mut prop_access = String::new();

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => {
                                    prop_name = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"type" => {
                                    prop_type = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"access" => {
                                    prop_access = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                _ => {}
                            }
                        }

                        let access = match prop_access.as_str() {
                            "read" => op_core::PropertyAccess::Read,
                            "write" => op_core::PropertyAccess::Write,
                            "readwrite" => op_core::PropertyAccess::ReadWrite,
                            _ => op_core::PropertyAccess::Read,
                        };

                        if let Some(ref mut iface) = current_interface {
                            iface.properties.push(PropertyInfo {
                                name: prop_name,
                                signature: prop_type,
                                access,
                            });
                        }
                    }
                    "arg" => {
                        if node_depth != 1 {
                            continue;
                        }
                        let mut arg_name = String::new();
                        let mut arg_type = String::new();
                        let mut arg_direction = "in".to_string();

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => {
                                    arg_name = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"type" => {
                                    arg_type = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"direction" => {
                                    arg_direction = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                _ => {}
                            }
                        }

                        let arg = op_core::ArgInfo {
                            name: if arg_name.is_empty() {
                                None
                            } else {
                                Some(arg_name)
                            },
                            signature: arg_type,
                            direction: if arg_direction == "out" {
                                op_core::ArgDirection::Out
                            } else {
                                op_core::ArgDirection::In
                            },
                        };

                        if in_method {
                            if let Some(ref mut method) = current_method {
                                if arg_direction == "out" {
                                    method.out_args.push(arg);
                                } else {
                                    method.in_args.push(arg);
                                }
                            }
                        } else if in_signal {
                            if let Some(ref mut signal) = current_signal {
                                signal.args.push(arg);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");

                match name {
                    "node" => {
                        if node_depth == 1 {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    let child_name =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                    let child_path = if child_name.starts_with('/') {
                                        child_name
                                    } else if path == "/" {
                                        format!("/{}", child_name)
                                    } else {
                                        format!("{}/{}", path, child_name)
                                    };
                                    children.push(child_path);
                                }
                            }
                        }
                    }
                    "property" => {
                        if node_depth != 1 {
                            continue;
                        }
                        let mut prop_name = String::new();
                        let mut prop_type = String::new();
                        let mut prop_access = String::new();

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => {
                                    prop_name = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"type" => {
                                    prop_type = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"access" => {
                                    prop_access = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                _ => {}
                            }
                        }

                        let access = match prop_access.as_str() {
                            "read" => op_core::PropertyAccess::Read,
                            "write" => op_core::PropertyAccess::Write,
                            "readwrite" => op_core::PropertyAccess::ReadWrite,
                            _ => op_core::PropertyAccess::Read,
                        };

                        if let Some(ref mut iface) = current_interface {
                            iface.properties.push(PropertyInfo {
                                name: prop_name,
                                signature: prop_type,
                                access,
                            });
                        }
                    }
                    "arg" => {
                        if node_depth != 1 {
                            continue;
                        }
                        let mut arg_name = String::new();
                        let mut arg_type = String::new();
                        let mut arg_direction = "in".to_string();

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => {
                                    arg_name = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"type" => {
                                    arg_type = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                b"direction" => {
                                    arg_direction = String::from_utf8_lossy(&attr.value).to_string()
                                }
                                _ => {}
                            }
                        }

                        let arg = op_core::ArgInfo {
                            name: if arg_name.is_empty() {
                                None
                            } else {
                                Some(arg_name)
                            },
                            signature: arg_type,
                            direction: if arg_direction == "out" {
                                op_core::ArgDirection::Out
                            } else {
                                op_core::ArgDirection::In
                            },
                        };

                        if in_method {
                            if let Some(ref mut method) = current_method {
                                if arg_direction == "out" {
                                    method.out_args.push(arg);
                                } else {
                                    method.in_args.push(arg);
                                }
                            }
                        } else if in_signal {
                            if let Some(ref mut signal) = current_signal {
                                signal.args.push(arg);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");

                match name {
                    "node" => {
                        node_depth = node_depth.saturating_sub(1);
                    }
                    "interface" => {
                        if let Some(iface) = current_interface.take() {
                            interfaces.push(iface);
                        }
                    }
                    "method" => {
                        in_method = false;
                        if let Some(method) = current_method.take() {
                            if let Some(ref mut iface) = current_interface {
                                iface.methods.push(method);
                            }
                        }
                    }
                    "signal" => {
                        in_signal = false;
                        if let Some(signal) = current_signal.take() {
                            if let Some(ref mut iface) = current_interface {
                                iface.signals.push(signal);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::introspection(format!("XML parse error: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    children.sort();
    children.dedup();

    Ok(ObjectInfo {
        path: path.to_string(),
        interfaces,
        children,
    })
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/Cargo.toml">
[package]
name = "op-introspection"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "DBus introspection capabilities for op-dbus-v2"

[dependencies]
op-core = { workspace = true }
op-blockchain = { path = "../op-blockchain" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
zbus = { workspace = true }
zbus_xml = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
futures = { workspace = true }
async-trait = { workspace = true }
quick-xml = { workspace = true }
rusqlite = { workspace = true, features = ["bundled"] }
chrono = { workspace = true }
parking_lot = "0.12"
sha2 = { workspace = true }
hex = "0.4"
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/compare-op-introspection.md">
# compare-op-introspection

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 10 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 6 |
| Partial artifacts | 0 |
| Spec-listed source files | 10 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- DBus introspection capabilities for op-dbus-v2
- Internal crate integrations: op-core, op-blockchain.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/scanner.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/scanner.rs |
| `src/projection.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/projection.rs |
| `src/parser.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/parser.rs |
| `src/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/mod.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/indexer_manager.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/indexer_manager.rs |
| `src/indexer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/indexer.rs |
| `src/hierarchical.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/hierarchical.rs |
| `src/cpu_features.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/cpu_features.rs |
| `src/cache.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/cache.rs |
| `root` | ✅ Present | root source group | src/cache.rs, src/cpu_features.rs, src/hierarchical.rs, src/indexer.rs, src/indexer_manager.rs, src/lib.rs, src/mod.rs, src/parser.rs, ... (+2 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| scanner | ✅ Implemented | src/scanner.rs | SPEC main module |
| projection | ✅ Implemented | src/projection.rs | SPEC main module |
| parser | ✅ Implemented | src/parser.rs | SPEC main module |
| mod | ✅ Implemented | src/mod.rs | SPEC main module |
| indexer_manager | ✅ Implemented | src/indexer_manager.rs | SPEC main module |
| indexer | ✅ Implemented | src/indexer.rs | SPEC main module |
| hierarchical | ✅ Implemented | src/hierarchical.rs | SPEC main module |
| cpu_features | ✅ Implemented | src/cpu_features.rs | SPEC main module |
| cache | ✅ Implemented | src/cache.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-blockchain` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `zbus` - documented in SPEC
- `zbus_xml` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `futures` - documented in SPEC
- `async-trait` - documented in SPEC
- `quick-xml` - documented in SPEC
- `rusqlite` - documented in SPEC
- `chrono` - documented in SPEC
- `parking_lot` - documented in SPEC
- `sha2` - documented in SPEC
- `hex` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: cache, indexer, indexer_manager, parser, projection, scanner.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-introspection/SPEC.md">
# op-introspection - Specification

## Overview
**Crate**: `op-introspection`  
**Location**: `crates/op-introspection`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-introspection"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-introspection/src/scanner.rs
op-introspection/src/projection.rs
op-introspection/src/parser.rs
op-introspection/src/mod.rs
op-introspection/src/lib.rs
op-introspection/src/indexer_manager.rs
op-introspection/src/indexer.rs
op-introspection/src/hierarchical.rs
op-introspection/src/cpu_features.rs
op-introspection/src/cache.rs
```

### Key Dependencies
```toml
op-core = { workspace = true }
op-blockchain = { path = "../op-blockchain" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
zbus = { workspace = true }
zbus_xml = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
futures = { workspace = true }
async-trait = { workspace = true }
quick-xml = { workspace = true }
rusqlite = { workspace = true, features = ["bundled"] }
chrono = { workspace = true }
parking_lot = "0.12"
sha2 = { workspace = true }
hex = "0.4"
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
      10 Rust source files

### Main Modules
scanner
projection
parser
mod
indexer_manager
indexer
hierarchical
cpu_features
cache

## Purpose
DBus introspection capabilities for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-blockchain

---
*Generated from crate analysis*
</file>

</files>
