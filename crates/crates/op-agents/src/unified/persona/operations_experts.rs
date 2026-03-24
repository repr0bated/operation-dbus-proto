//! Operations Expert Agents

use super::base::PersonaAgent;
use super::super::agent_trait::AgentCapability;
use super::super::prompts::operations::{KUBERNETES_EXPERT, SYSTEMD_EXPERT, DBUS_EXPERT};

pub struct KubernetesExpert(PersonaAgent);

impl KubernetesExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "kubernetes-expert",
            "Kubernetes Expert",
            "Expert in Kubernetes, container orchestration, Helm, and cloud-native patterns.",
            "kubernetes",
            "You are a Kubernetes expert with deep knowledge of container orchestration and cloud-native patterns.",
            KUBERNETES_EXPERT,
        )
        .with_capability(AgentCapability::ArchitectureDesign)
    }
}

impl Default for KubernetesExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct SystemdExpert(PersonaAgent);

impl SystemdExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "systemd-expert",
            "Systemd Expert",
            "Expert in systemd service management, unit files, and Linux system administration.",
            "systemd",
            "You are a systemd expert with deep knowledge of Linux service management and system administration.",
            SYSTEMD_EXPERT,
        )
    }
}

impl Default for SystemdExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct DbusExpert(PersonaAgent);

impl DbusExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "dbus-expert",
            "D-Bus Expert",
            "Expert in D-Bus IPC, introspection, and Linux desktop/system integration.",
            "dbus",
            "You are a D-Bus expert with deep knowledge of inter-process communication on Linux.",
            DBUS_EXPERT,
        )
    }
}

impl Default for DbusExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}
