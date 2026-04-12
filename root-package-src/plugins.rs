use anyhow::{anyhow, Result};
use jsonschema::Draft;
use serde_json::Value;
use sqlx::SqlitePool;
use std::path::Path;

pub struct PluginDefinition {
    pub name: &'static str,
    pub service_name: &'static str,
    pub schema_json: &'static str,
}

const PLUGIN_DEFINITIONS: &[PluginDefinition] = &[
    PluginDefinition {
        name: "directory",
        service_name: "org.opdbus.directory.v1",
        schema_json: r#"{
                "type": "DirectoryEntry",
                "description": "Enterprise directory and identity management (AD/LDAP replacement)",
                "replaces": ["Active Directory", "OpenLDAP", "FreeIPA"],
                "common_properties": {
                    "id": {"type": "uuid", "required": true},
                    "created_at": {"type": "timestamp", "required": true},
                    "updated_at": {"type": "timestamp", "required": true},
                    "object_type": {"type": "string", "required": true}
                },
                "object_types": {
                    "Domain": {
                        "description": "AD Domain or Forest Root",
                        "base_path": "/org/opdbus/directory/domains",
                        "interface": "org.opdbus.directory.v1.Domain",
                        "properties": {
                            "domain_name": {"type": "string", "required": true},
                            "dns_root": {"type": "string"},
                            "forest_name": {"type": "string"},
                            "functional_level": {"type": "integer"},
                            "domain_sid": {"type": "string"},
                            "netbios_name": {"type": "string"}
                        }
                    },
                    "OrganizationalUnit": {
                        "description": "Organizational unit (OU)",
                        "base_path": "/org/opdbus/directory/ou",
                        "interface": "org.opdbus.directory.v1.OrganizationalUnit",
                        "properties": {
                            "ou_name": {"type": "string", "required": true},
                            "description": {"type": "string"},
                            "parent_dn": {"type": "string"},
                            "gpo_links": {"type": "array"},
                            "managed_by": {"type": "string"}
                        }
                    },
                    "Site": {
                        "description": "AD Physical Site",
                        "base_path": "/org/opdbus/directory/sites",
                        "interface": "org.opdbus.directory.v1.Site",
                        "properties": {
                            "site_name": {"type": "string", "required": true},
                            "description": {"type": "string"},
                            "subnets": {"type": "array"},
                            "site_links": {"type": "array"},
                            "location": {"type": "string"}
                        }
                    },
                    "User": {
                        "description": "AD-compatible User Account",
                        "base_path": "/org/opdbus/directory/users",
                        "interface": "org.opdbus.directory.v1.User",
                        "properties": {
                            "username": {"type": "string", "required": true},
                            "display_name": {"type": "string"},
                            "email": {"type": "string"},
                            "department": {"type": "string"},
                            "title": {"type": "string"},
                            "member_of": {"type": "array"},
                            "object_sid": {"type": "string"},
                            "user_principal_name": {"type": "string"}
                        }
                    },
                    "Group": {
                        "description": "AD-compatible Group",
                        "base_path": "/org/opdbus/directory/groups",
                        "interface": "org.opdbus.directory.v1.Group",
                        "properties": {
                            "group_name": {"type": "string", "required": true},
                            "description": {"type": "string"},
                            "members": {"type": "array"},
                            "group_type": {"type": "integer"}
                        }
                    },
                    "Computer": {
                        "description": "AD-compatible Computer Account",
                        "base_path": "/org/opdbus/directory/computers",
                        "interface": "org.opdbus.directory.v1.Computer",
                        "properties": {
                            "computer_name": {"type": "string", "required": true},
                            "dns_hostname": {"type": "string"},
                            "operating_system": {"type": "string"},
                            "os_version": {"type": "string"}
                        }
                    },
                    "GroupPolicyObject": {
                        "description": "AD Group Policy Object",
                        "base_path": "/org/opdbus/directory/gpo",
                        "interface": "org.opdbus.directory.v1.GroupPolicyObject",
                        "properties": {
                            "gpo_name": {"type": "string", "required": true},
                            "gpo_guid": {"type": "string", "required": true},
                            "settings": {"type": "object"}
                        }
                    },
                    "DNSZone": {
                        "description": "AD-integrated DNS Zone",
                        "base_path": "/org/opdbus/directory/dns",
                        "interface": "org.opdbus.directory.v1.DNSZone",
                        "properties": {
                            "zone_name": {"type": "string", "required": true},
                            "zone_type": {"type": "string"},
                            "dns_records": {"type": "array"}
                        }
                    }
                }
            }"#,
    },
    PluginDefinition {
        name: "cms",
        service_name: "org.opdbus.cms.v1",
        schema_json: r#"{
                "type": "CMSObject",
                "description": "Enterprise CMS management (Drupal/WordPress replacement)",
                "replaces": ["Drupal", "WordPress"],
                "common_properties": {
                    "id": {"type": "uuid", "required": true},
                    "created_at": {"type": "timestamp", "required": true},
                    "object_type": {"type": "string", "required": true}
                },
                "object_types": {
                    "Node": {
                        "description": "Drupal-compatible Content Node",
                        "base_path": "/org/opdbus/cms/nodes",
                        "interface": "org.opdbus.cms.v1.Node",
                        "properties": {
                            "title": {"type": "string", "required": true},
                            "body": {"type": "string"},
                            "content_type": {"type": "string", "required": true},
                            "status": {"type": "string"}
                        }
                    },
                    "WPPost": {
                        "description": "WordPress-compatible Post",
                        "base_path": "/org/opdbus/cms/wp-posts",
                        "interface": "org.opdbus.cms.v1.WPPost",
                        "properties": {
                            "post_title": {"type": "string", "required": true},
                            "post_content": {"type": "string"},
                            "post_status": {"type": "string"},
                            "post_type": {"type": "string", "required": true}
                        }
                    },
                    "WPUser": {
                        "description": "WordPress-compatible User",
                        "base_path": "/org/opdbus/cms/wp-users",
                        "interface": "org.opdbus.cms.v1.WPUser",
                        "properties": {
                            "user_login": {"type": "string", "required": true},
                            "user_email": {"type": "string", "required": true},
                            "role": {"type": "string"}
                        }
                    },
                    "WCProduct": {
                        "description": "WooCommerce-compatible Product",
                        "base_path": "/org/opdbus/cms/wc-products",
                        "interface": "org.opdbus.cms.v1.WCProduct",
                        "properties": {
                            "sku": {"type": "string"},
                            "regular_price": {"type": "string"},
                            "stock_quantity": {"type": "integer"}
                        }
                    }
                }
            }"#,
    },
    PluginDefinition {
        name: "network",
        service_name: "org.opdbus.network.v1",
        schema_json: r#"{
                "type": "NetworkObject",
                "description": "Modern network management with virtual networking",
                "replaces": ["NetworkManager", "systemd-networkd"],
                "common_properties": {
                    "id": {"type": "uuid", "required": true},
                    "name": {"type": "string", "required": true},
                    "state": {"type": "string", "required": true},
                    "created_at": {"type": "timestamp", "required": true}
                },
                "object_types": {
                    "Interface": {
                        "description": "Network interface",
                        "schema_derived": true,
                        "rcp_db": "ovsdb",
                        "rcp_table": "Interface",
                        "id_field": "name",
                        "base_path": "/org/opdbus/network/interfaces",
                        "interface": "org.opdbus.network.v1.Interface"
                    },
                    "Bridge": {
                        "description": "Network bridge",
                        "schema_derived": true,
                        "rcp_db": "ovsdb",
                        "rcp_table": "Bridge",
                        "id_field": "name",
                        "base_path": "/org/opdbus/network/bridges",
                        "interface": "org.opdbus.network.v1.Bridge"
                    },
                    "Port": {
                        "description": "OVS port",
                        "schema_derived": true,
                        "rcp_db": "ovsdb",
                        "rcp_table": "Port",
                        "id_field": "name",
                        "base_path": "/org/opdbus/network/ports",
                        "interface": "org.opdbus.network.v1.Port"
                    },
                    "VLAN": {
                        "description": "VLAN interface",
                        "schema_derived": true,
                        "base_path": "/org/opdbus/network/vlans",
                        "interface": "org.opdbus.network.v1.VLAN"
                    }
                }
            }"#,
    },
    PluginDefinition {
        name: "hardware",
        service_name: "org.opdbus.hardware.v1",
        schema_json: r#"{
                "type": "HardwareDevice",
                "description": "Unified hardware discovery and management",
                "replaces": ["lshw", "dmidecode"],
                "common_properties": {
                    "id": {"type": "uuid", "required": true},
                    "device_type": {"type": "string", "required": true},
                    "vendor": {"type": "string"},
                    "model": {"type": "string"}
                },
                "object_types": {
                    "CPU": {"base_path": "/org/opdbus/hardware/cpu", "interface": "org.opdbus.hardware.v1.CPU"},
                    "Memory": {"base_path": "/org/opdbus/hardware/memory", "interface": "org.opdbus.hardware.v1.Memory"},
                    "Disk": {"base_path": "/org/opdbus/hardware/disk", "interface": "org.opdbus.hardware.v1.Disk"}
                }
            }"#,
    },
    PluginDefinition {
        name: "storage",
        service_name: "org.opdbus.storage.v1",
        schema_json: r#"{
                "type": "StorageObject",
                "description": "Unified storage management",
                "replaces": ["LVM", "mdadm"],
                "common_properties": {
                    "id": {"type": "uuid", "required": true},
                    "name": {"type": "string", "required": true},
                    "size": {"type": "uint64"}
                },
                "object_types": {
                    "Volume": {"base_path": "/org/opdbus/storage/volumes", "interface": "org.opdbus.storage.v1.Volume"},
                    "Filesystem": {"base_path": "/org/opdbus/storage/filesystems", "interface": "org.opdbus.storage.v1.Filesystem"}
                }
            }"#,
    },
    PluginDefinition {
        name: "container",
        service_name: "org.opdbus.container.v1",
        schema_json: r#"{
                "type": "ContainerObject",
                "description": "Container lifecycle and orchestration",
                "replaces": ["Docker", "Podman"],
                "common_properties": {
                    "id": {"type": "uuid", "required": true},
                    "name": {"type": "string", "required": true},
                    "state": {"type": "string", "required": true}
                },
                "object_types": {
                    "Container": {"base_path": "/org/opdbus/container/containers", "interface": "org.opdbus.container.v1.Container"},
                    "Image": {"base_path": "/org/opdbus/container/images", "interface": "org.opdbus.container.v1.Image"}
                }
            }"#,
    },
    PluginDefinition {
        name: "incus",
        service_name: "org.opdbus.incus.v1",
        schema_json: r#"{
            "version": "1.0.0",
            "plugin_type": "container",
            "type": "IncusObject",
            "description": "Incus container and virtual machine management",
            "replaces": ["LXD"],
            "capabilities": {
                "can_read": true,
                "can_write": true,
                "can_delete": true,
                "supports_rollback": true,
                "supports_checkpoints": true,
                "requires_root": true
            },
            "dependencies": [
                {"name": "incus", "version": "6.0", "required": true}
            ],
            "redactable_fields": [
                "config", "devices", "expanded_config", "expanded_devices", "volatile"
            ],
            "immutable_fields": [
                "name", "architecture", "type", "created_at", "location", "fingerprint"
            ],
            "common_properties": {
                "id": {"type": "string", "required": true},
                "name": {"type": "string", "required": true},
                "description": {"type": "string"},
                "created_at": {"type": "timestamp", "required": true},
                "updated_at": {"type": "timestamp", "required": true},
                "status": {"type": "string"},
                "status_code": {"type": "integer"},
                "architecture": {"type": "string"},
                "type": {"type": "string"},
                "project": {"type": "string"}
            },
            "object_types": {
                "Container": {
                    "description": "Incus container instance",
                    "base_path": "/org/opdbus/incus/containers",
                    "interface": "org.opdbus.incus.v1.Container",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "status": {"type": "string", "required": true},
                        "status_code": {"type": "integer"},
                        "type": {"type": "string"},
                        "architecture": {"type": "string"},
                        "created_at": {"type": "timestamp"},
                        "updated_at": {"type": "timestamp"},
                        "description": {"type": "string"},
                        "config": {"type": "object"},
                        "devices": {"type": "object"},
                        "expanded_config": {"type": "object"},
                        "expanded_devices": {"type": "object"},
                        "profiles": {"type": "array"},
                        "project": {"type": "string"},
                        "location": {"type": "string"},
                        "processes": {"type": "integer"},
                        "cpu": {"type": "number"},
                        "memory": {"type": "integer"},
                        "disk": {"type": "integer"},
                        "network": {"type": "object"},
                        "stateful": {"type": "boolean"},
                        "ephemeral": {"type": "boolean"},
                        "protected": {"type": "boolean"},
                        "restarted": {"type": "integer"},
                        "snapshots": {"type": "array"}
                    }
                },
                "VM": {
                    "description": "Incus virtual machine instance",
                    "base_path": "/org/opdbus/incus/vms",
                    "interface": "org.opdbus.incus.v1.VM",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "status": {"type": "string", "required": true},
                        "status_code": {"type": "integer"},
                        "type": {"type": "string"},
                        "architecture": {"type": "string"},
                        "created_at": {"type": "timestamp"},
                        "updated_at": {"type": "timestamp"},
                        "description": {"type": "string"},
                        "config": {"type": "object"},
                        "devices": {"type": "object"},
                        "expanded_config": {"type": "object"},
                        "expanded_devices": {"type": "object"},
                        "profiles": {"type": "array"},
                        "project": {"type": "string"},
                        "location": {"type": "string"},
                        "cpu": {"type": "number"},
                        "memory": {"type": "integer"},
                        "disk": {"type": "integer"},
                        "network": {"type": "object"},
                        "stateful": {"type": "boolean"},
                        "ephemeral": {"type": "boolean"},
                        "protected": {"type": "boolean"},
                        "restarted": {"type": "integer"},
                        "snapshots": {"type": "array"}
                    }
                },
                "Image": {
                    "description": "Incus image",
                    "base_path": "/org/opdbus/incus/images",
                    "interface": "org.opdbus.incus.v1.Image",
                    "properties": {
                        "id": {"type": "string", "required": true},
                        "aliases": {"type": "array"},
                        "architecture": {"type": "string"},
                        "type": {"type": "string"},
                        "description": {"type": "string"},
                        "created_at": {"type": "timestamp"},
                        "expires_at": {"type": "timestamp"},
                        "filename": {"type": "string"},
                        "fingerprint": {"type": "string"},
                        "size": {"type": "integer"},
                        "public": {"type": "boolean"},
                        "auto_update": {"type": "boolean"},
                        "project": {"type": "string"},
                        "source": {"type": "object"},
                        "properties": {"type": "object"},
                        "update_source": {"type": "object"},
                        "cached": {"type": "boolean"},
                        "last_used_at": {"type": "timestamp"}
                    }
                },
                "Network": {
                    "description": "Incus network",
                    "base_path": "/org/opdbus/incus/networks",
                    "interface": "org.opdbus.incus.v1.Network",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "type": {"type": "string"},
                        "status": {"type": "string"},
                        "description": {"type": "string"},
                        "config": {"type": "object"},
                        "project": {"type": "string"},
                        "locations": {"type": "array"},
                        "managed": {"type": "boolean"},
                        "network": {"type": "string"},
                        "ipv4_address": {"type": "string"},
                        "ipv4_netmask": {"type": "string"},
                        "ipv4_dhcp": {"type": "boolean"},
                        "ipv4_dhcp_ranges": {"type": "array"},
                        "ipv4_nat": {"type": "boolean"},
                        "ipv4_nat_address": {"type": "string"},
                        "ipv6_address": {"type": "string"},
                        "ipv6_netmask": {"type": "string"},
                        "ipv6_dhcp": {"type": "boolean"},
                        "ipv6_dhcp_ranges": {"type": "array"},
                        "ipv6_nat": {"type": "boolean"},
                        "dns_domain": {"type": "string"},
                        "dns_nameservers": {"type": "array"}
                    }
                },
                "StoragePool": {
                    "description": "Incus storage pool",
                    "base_path": "/org/opdbus/incus/storage-pools",
                    "interface": "org.opdbus.incus.v1.StoragePool",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "description": {"type": "string"},
                        "driver": {"type": "string"},
                        "status": {"type": "string"},
                        "config": {"type": "object"},
                        "used_by": {"type": "array"},
                        "locations": {"type": "array"},
                        "space": {"type": "object"},
                        "inodes": {"type": "object"},
                        "total_space": {"type": "integer"},
                        "used_space": {"type": "integer"},
                        "available_space": {"type": "integer"}
                    }
                },
                "Profile": {
                    "description": "Incus profile",
                    "base_path": "/org/opdbus/incus/profiles",
                    "interface": "org.opdbus.incus.v1.Profile",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "description": {"type": "string"},
                        "config": {"type": "object"},
                        "devices": {"type": "object"},
                        "project": {"type": "string"},
                        "used_by": {"type": "array"}
                    }
                },
                "Project": {
                    "description": "Incus project",
                    "base_path": "/org/opdbus/incus/projects",
                    "interface": "org.opdbus.incus.v1.Project",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "description": {"type": "string"},
                        "config": {"type": "object"},
                        "features_images": {"type": "boolean"},
                        "features_profiles": {"type": "boolean"},
                        "features_networks": {"type": "boolean"},
                        "features_networks_zones": {"type": "boolean"},
                        "features_storage_buckets": {"type": "boolean"},
                        "features_storage_volumes": {"type": "boolean"},
                        "limits_containers": {"type": "integer"},
                        "limits_instances": {"type": "integer"},
                        "limits_virtual_machines": {"type": "integer"},
                        "limits_memory": {"type": "integer"},
                        "limits_cpu": {"type": "integer"},
                        "limits_disk": {"type": "integer"},
                        "limits_networks": {"type": "integer"},
                        "used_by": {"type": "array"}
                    }
                },
                "Operation": {
                    "description": "Incus operation",
                    "base_path": "/org/opdbus/incus/operations",
                    "interface": "org.opdbus.incus.v1.Operation",
                    "properties": {
                        "id": {"type": "string", "required": true},
                        "type": {"type": "string"},
                        "status": {"type": "string"},
                        "status_code": {"type": "integer"},
                        "description": {"type": "string"},
                        "created_at": {"type": "timestamp"},
                        "updated_at": {"type": "timestamp"},
                        "may_cancel": {"type": "boolean"},
                        "err": {"type": "string"},
                        "class": {"type": "string"},
                        "location": {"type": "string"},
                        "project": {"type": "string"},
                        "resources": {"type": "object"},
                        "metadata": {"type": "object"},
                        "progress": {"type": "object"}
                    }
                },
                "Cluster": {
                    "description": "Incus cluster member",
                    "base_path": "/org/opdbus/incus/cluster",
                    "interface": "org.opdbus.incus.v1.Cluster",
                    "properties": {
                        "server_name": {"type": "string", "required": true},
                        "url": {"type": "string"},
                        "version": {"type": "string"},
                        "cert": {"type": "string"},
                        "fingerprint": {"type": "string"},
                        "status": {"type": "string"},
                        "message": {"type": "string"},
                        "roles": {"type": "array"},
                        "architecture": {"type": "string"},
                        "failure_domain": {"type": "string"},
                        "config": {"type": "object"},
                        "resources": {"type": "object"},
                        "connected": {"type": "boolean"}
                    }
                },
                "Certificate": {
                    "description": "Incus certificate",
                    "base_path": "/org/opdbus/incus/certificates",
                    "interface": "org.opdbus.incus.v1.Certificate",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "type": {"type": "string"},
                        "fingerprint": {"type": "string"},
                        "cert": {"type": "string"},
                        "subject": {"type": "string"},
                        "issuer": {"type": "string"},
                        "valid_from": {"type": "timestamp"},
                        "valid_to": {"type": "timestamp"},
                        "restricted": {"type": "boolean"},
                        "project": {"type": "string"},
                        "projects": {"type": "array"}
                    }
                },
                "StorageVolume": {
                    "description": "Incus storage volume",
                    "base_path": "/org/opdbus/incus/storage-volumes",
                    "interface": "org.opdbus.incus.v1.StorageVolume",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "type": {"type": "string"},
                        "status": {"type": "string"},
                        "description": {"type": "string"},
                        "config": {"type": "object"},
                        "project": {"type": "string"},
                        "pool": {"type": "string"},
                        "location": {"type": "string"},
                        "content_type": {"type": "string"},
                        "used_by": {"type": "array"},
                        "size": {"type": "integer"},
                        "size_state": {"type": "string"}
                    }
                },
                "InstanceSnapshot": {
                    "description": "Incus instance snapshot",
                    "base_path": "/org/opdbus/incus/snapshots",
                    "interface": "org.opdbus.incus.v1.InstanceSnapshot",
                    "properties": {
                        "name": {"type": "string", "required": true},
                        "description": {"type": "string"},
                        "created_at": {"type": "timestamp"},
                        "expires_at": {"type": "timestamp"},
                        "stateful": {"type": "boolean"},
                        "project": {"type": "string"},
                        "instance_only": {"type": "boolean"},
                        "instance_name": {"type": "string"},
                        "instance_type": {"type": "string"},
                        "config": {"type": "object"},
                        "devices": {"type": "object"}
                    }
                }
            }
        }"#,
    },
];

pub fn plugin_definitions() -> &'static [PluginDefinition] {
    PLUGIN_DEFINITIONS
}

pub fn get_plugin_schema_json(plugin: &str) -> Option<&'static str> {
    PLUGIN_DEFINITIONS
        .iter()
        .find(|p| p.name == plugin)
        .map(|p| p.schema_json)
}

pub fn validate_plugin_schemas_from_repo() -> Result<()> {
    let official_meta_path = Path::new("/git/json-schema-spec/specs/meta/meta.json");
    validate_plugin_schemas(official_meta_path)
}

pub fn validate_plugin_schemas(official_meta_schema_path: &Path) -> Result<()> {
    let official_meta_schema: Value = if official_meta_schema_path.exists() {
        let official_meta_str =
            std::fs::read_to_string(official_meta_schema_path).map_err(|e| {
                anyhow!(
                    "Failed to read meta-schema {}: {}",
                    official_meta_schema_path.display(),
                    e
                )
            })?;
        serde_json::from_str(&official_meta_str)
            .map_err(|e| anyhow!("Failed to parse official meta-schema JSON: {}", e))?
    } else {
        let embedded = include_str!("../schemas/jsonschema-meta.json");
        serde_json::from_str(embedded)
            .map_err(|e| anyhow!("Failed to parse embedded meta-schema JSON: {}", e))?
    };

    let plugin_meta_schema: Value = {
        let embedded = include_str!("../schemas/opdbus-plugin-schema.json");
        serde_json::from_str(embedded)
            .map_err(|e| anyhow!("Failed to parse embedded plugin meta-schema JSON: {}", e))?
    };

    // Validate our plugin meta-schema against the official meta-schema.
    let official_compiled = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&official_meta_schema)
        .map_err(|e| anyhow!("Failed to compile official meta-schema: {}", e))?;
    let errors: Vec<String> = official_compiled
        .iter_errors(&plugin_meta_schema)
        .map(|err| format!("meta-schema error: {} at {}", err, err.instance_path))
        .collect();
    if !errors.is_empty() {
        return Err(anyhow!(
            "Plugin meta-schema validation failed:\n{}",
            errors.join("\n")
        ));
    }

    // Validate plugin schemas against the plugin meta-schema.
    let plugin_compiled = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&plugin_meta_schema)
        .map_err(|e| anyhow!("Failed to compile plugin meta-schema: {}", e))?;

    let mut errors = Vec::new();
    for plugin in PLUGIN_DEFINITIONS {
        let schema_value: Value = serde_json::from_str(plugin.schema_json)
            .map_err(|e| anyhow!("Invalid JSON for plugin {}: {}", plugin.name, e))?;
        for err in plugin_compiled.iter_errors(&schema_value) {
            errors.push(format!(
                "plugin {}: {} at {}",
                plugin.name, err, err.instance_path
            ));
        }
    }

    if !errors.is_empty() {
        return Err(anyhow!(
            "Plugin schema validation failed:\n{}",
            errors.join("\n")
        ));
    }

    Ok(())
}

/// Return plugin schema definitions keyed by name.
///
/// The values are the parsed `schema_json` from each `PluginDefinition`,
/// containing `object_types` with `base_path` entries. The D-Bus mirror
/// uses these to project state to schema-derived paths.
pub fn plugin_schema_defs() -> std::collections::HashMap<String, simd_json::OwnedValue> {
    let mut defs = std::collections::HashMap::new();
    for plugin in PLUGIN_DEFINITIONS {
        // Use standard serde_json for initial robust parsing
        let value: serde_json::Value = match serde_json::from_str(plugin.schema_json) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("ERROR: Failed to parse schema for plugin {}: {}", plugin.name, e);
                continue;
            }
        };

        // Convert to OwnedValue for compatibility with the existing return type
        let json_str = value.to_string();
        if let Ok(val) = unsafe { simd_json::from_str(&mut json_str.clone()) } {
            defs.insert(plugin.name.to_string(), val);
        } else {
            eprintln!("ERROR: Failed to convert schema to OwnedValue for plugin {}", plugin.name);
        }
    }
    defs
}

pub async fn insert_plugins(pool: &SqlitePool) -> Result<()> {
    for plugin in PLUGIN_DEFINITIONS {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO plugins (name, service_name, base_object)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(plugin.name)
        .bind(plugin.service_name)
        .bind(plugin.schema_json)
        .execute(pool)
        .await?;
    }

    Ok(())
}
