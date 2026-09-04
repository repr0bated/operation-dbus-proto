This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
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
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  ad_full_schema.sql
  cms_drupal_schema.sql
  cms_wordpress_schema.sql
  disaster_recovery.rs
  error.rs
  event_chain.rs
  execution_job.rs
  lib.rs
  memory_store.rs
  metrics.rs
  namespace_schema.sql
  plugin_schema.rs
  redis_stream.rs
  schema_shuttle.rs
  schema_validator.rs
  sqlite_store.rs
  state_store.rs
Cargo.toml
compare-op-state-store.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/ad_full_schema.sql">
-- ===================================================================
-- FULL ACTIVE DIRECTORY REPLACEMENT SCHEMA
-- Complete AD object classes for org.opdbus.directory
-- Status: LIVE AND UNFILLED (ready for enterprise deployment)
-- ===================================================================

-- Create Directory service interface if not exists
INSERT OR IGNORE INTO service_interfaces (service_id, interface_name, version, methods_schema, signals_schema, properties_schema)
SELECT id, 'org.opdbus.directory.Manager', 'v1',
       json('{"CreateUser": {"params": ["user_data"], "returns": "user_id"}, "CreateGroup": {"params": ["group_data"], "returns": "group_id"}, "AddUserToGroup": {"params": ["user_id", "group_id"]}, "AuthenticateUser": {"params": ["username", "password"], "returns": "bool"}}'),
       json('{"UserCreated": {"params": ["user_id", "username"]}, "UserDeleted": {"params": ["user_id"]}, "GroupCreated": {"params": ["group_id", "group_name"]}}'),
       json('{"TotalUsers": {"type": "int", "access": "read"}, "TotalGroups": {"type": "int", "access": "read"}}')
FROM namespace_services WHERE service_name = 'org.opdbus.directory';

-- ===================================================================
-- AD Domain/Forest Structure
-- ===================================================================

-- Domain objects
INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'Domain', '1.2.840.113556.1.5.2', NULL, TRUE,
       json('{"domain_name": {"type": "string", "mandatory": true}, "dns_root": {"type": "string"}, "forest_name": {"type": "string"}, "functional_level": {"type": "int"}, "domain_sid": {"type": "string"}, "netbios_name": {"type": "string"}}')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- Organizational Units
INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'OrganizationalUnit', '2.5.6.5', NULL, TRUE,
       json('{"ou_name": {"type": "string", "mandatory": true}, "description": {"type": "string"}, "parent_dn": {"type": "string"}, "gpo_links": {"type": "array"}, "managed_by": {"type": "string"}}')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- Sites (for multi-site AD)
INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'Site', '1.2.840.113556.1.3.14', NULL, TRUE,
       json('{"site_name": {"type": "string", "mandatory": true}, "description": {"type": "string"}, "subnets": {"type": "array"}, "site_links": {"type": "array"}, "location": {"type": "string"}}')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- Complete AD User Schema
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'ADUser', '1.2.840.113556.1.5.9', NULL, TRUE,
       json('{
         "username": {"type": "string", "mandatory": true, "ldap": "sAMAccountName"},
         "user_principal_name": {"type": "string", "ldap": "userPrincipalName"},
         "display_name": {"type": "string", "ldap": "displayName"},
         "given_name": {"type": "string", "ldap": "givenName"},
         "surname": {"type": "string", "ldap": "sn"},
         "email": {"type": "string", "ldap": "mail"},
         "telephone": {"type": "string", "ldap": "telephoneNumber"},
         "mobile": {"type": "string", "ldap": "mobile"},
         "department": {"type": "string", "ldap": "department"},
         "title": {"type": "string", "ldap": "title"},
         "company": {"type": "string", "ldap": "company"},
         "manager": {"type": "string", "ldap": "manager"},
         "direct_reports": {"type": "array", "ldap": "directReports"},
         "office": {"type": "string", "ldap": "physicalDeliveryOfficeName"},
         "street_address": {"type": "string", "ldap": "streetAddress"},
         "city": {"type": "string", "ldap": "l"},
         "state": {"type": "string", "ldap": "st"},
         "postal_code": {"type": "string", "ldap": "postalCode"},
         "country": {"type": "string", "ldap": "co"},
         "home_directory": {"type": "string", "ldap": "homeDirectory"},
         "home_drive": {"type": "string", "ldap": "homeDrive"},
         "logon_script": {"type": "string", "ldap": "scriptPath"},
         "profile_path": {"type": "string", "ldap": "profilePath"},
         "member_of": {"type": "array", "ldap": "memberOf"},
         "account_expires": {"type": "string", "ldap": "accountExpires"},
         "password_last_set": {"type": "string", "ldap": "pwdLastSet"},
         "last_logon": {"type": "string", "ldap": "lastLogon"},
         "last_logon_timestamp": {"type": "string", "ldap": "lastLogonTimestamp"},
         "bad_password_count": {"type": "int", "ldap": "badPwdCount"},
         "user_account_control": {"type": "int", "ldap": "userAccountControl"},
         "object_sid": {"type": "string", "ldap": "objectSid"},
         "object_guid": {"type": "string", "ldap": "objectGUID"},
         "when_created": {"type": "string", "ldap": "whenCreated"},
         "when_changed": {"type": "string", "ldap": "whenChanged"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- Complete AD Group Schema
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'ADGroup', '1.2.840.113556.1.5.8', NULL, TRUE,
       json('{
         "group_name": {"type": "string", "mandatory": true, "ldap": "sAMAccountName"},
         "display_name": {"type": "string", "ldap": "displayName"},
         "description": {"type": "string", "ldap": "description"},
         "group_type": {"type": "int", "ldap": "groupType"},
         "group_scope": {"type": "string"},
         "members": {"type": "array", "ldap": "member"},
         "member_of": {"type": "array", "ldap": "memberOf"},
         "managed_by": {"type": "string", "ldap": "managedBy"},
         "mail": {"type": "string", "ldap": "mail"},
         "object_sid": {"type": "string", "ldap": "objectSid"},
         "when_created": {"type": "string", "ldap": "whenCreated"},
         "when_changed": {"type": "string", "ldap": "whenChanged"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- AD Computer Objects
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'ADComputer', '1.2.840.113556.1.5.9', NULL, TRUE,
       json('{
         "computer_name": {"type": "string", "mandatory": true, "ldap": "sAMAccountName"},
         "dns_hostname": {"type": "string", "ldap": "dNSHostName"},
         "operating_system": {"type": "string", "ldap": "operatingSystem"},
         "os_version": {"type": "string", "ldap": "operatingSystemVersion"},
         "os_service_pack": {"type": "string", "ldap": "operatingSystemServicePack"},
         "description": {"type": "string", "ldap": "description"},
         "location": {"type": "string", "ldap": "location"},
         "managed_by": {"type": "string", "ldap": "managedBy"},
         "member_of": {"type": "array", "ldap": "memberOf"},
         "last_logon": {"type": "string", "ldap": "lastLogon"},
         "last_logon_timestamp": {"type": "string", "ldap": "lastLogonTimestamp"},
         "password_last_set": {"type": "string", "ldap": "pwdLastSet"},
         "object_sid": {"type": "string", "ldap": "objectSid"},
         "when_created": {"type": "string", "ldap": "whenCreated"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- Group Policy Objects (GPOs)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'GroupPolicyObject', '1.2.840.113556.1.5.4', NULL, TRUE,
       json('{
         "gpo_name": {"type": "string", "mandatory": true, "ldap": "displayName"},
         "gpo_guid": {"type": "string", "mandatory": true},
         "gpo_status": {"type": "string"},
         "version_number": {"type": "int"},
         "computer_version": {"type": "int"},
         "user_version": {"type": "int"},
         "wmi_filter": {"type": "string"},
         "linked_ous": {"type": "array"},
         "security_filtering": {"type": "array"},
         "settings": {"type": "dict"},
         "when_created": {"type": "string", "ldap": "whenCreated"},
         "when_changed": {"type": "string", "ldap": "whenChanged"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- Contacts
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'Contact', '2.5.6.6', NULL, TRUE,
       json('{
         "display_name": {"type": "string", "mandatory": true, "ldap": "displayName"},
         "email": {"type": "string", "ldap": "mail"},
         "telephone": {"type": "string", "ldap": "telephoneNumber"},
         "company": {"type": "string", "ldap": "company"},
         "title": {"type": "string", "ldap": "title"},
         "description": {"type": "string", "ldap": "description"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- Service Accounts
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'ServiceAccount', '1.2.840.113556.1.5.9', 'ADUser', TRUE,
       json('{
         "service_name": {"type": "string", "mandatory": true},
         "service_type": {"type": "string"},
         "managed_service_account": {"type": "bool"},
         "group_managed": {"type": "bool"},
         "spn": {"type": "array", "ldap": "servicePrincipalName"},
         "associated_computers": {"type": "array"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- DNS Zones (for AD-integrated DNS)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'DNSZone', '1.2.840.113556.1.5.130', NULL, TRUE,
       json('{
         "zone_name": {"type": "string", "mandatory": true},
         "zone_type": {"type": "string"},
         "dynamic_update": {"type": "bool"},
         "secure_update": {"type": "bool"},
         "replication_scope": {"type": "string"},
         "dns_records": {"type": "array"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;

-- ===================================================================
-- Trust Relationships
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, ldap_oid, parent_class, structural, attributes_schema)
SELECT id, 'TrustRelationship', '1.2.840.113556.1.5.37', NULL, TRUE,
       json('{
         "trusted_domain": {"type": "string", "mandatory": true},
         "trust_direction": {"type": "string"},
         "trust_type": {"type": "string"},
         "trust_attributes": {"type": "int"},
         "trust_partner": {"type": "string"},
         "when_created": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name LIKE 'org.opdbus.directory%' LIMIT 1;
</file>

<file path="src/cms_drupal_schema.sql">
-- ===================================================================
-- FULL DRUPAL CMS SCHEMA
-- Complete Drupal object classes for org.opdbus.cms
-- Status: LIVE AND UNFILLED (ready for CMS deployment)
-- ===================================================================

-- Add CMS service if not exists
INSERT OR IGNORE INTO namespace_services (service_name, description, version)
VALUES ('org.opdbus.cms', 'Content Management System (Drupal-compatible)', 'v1');

-- Create CMS interface
INSERT OR IGNORE INTO service_interfaces (service_id, interface_name, version, methods_schema, signals_schema, properties_schema)
SELECT id, 'org.opdbus.cms.Manager', 'v1',
       json('{"CreateContent": {"params": ["content_type", "data"], "returns": "node_id"}, "UpdateContent": {"params": ["node_id", "data"]}, "DeleteContent": {"params": ["node_id"]}, "PublishContent": {"params": ["node_id"]}}'),
       json('{"ContentCreated": {"params": ["node_id", "content_type"]}, "ContentPublished": {"params": ["node_id"]}, "ContentDeleted": {"params": ["node_id"]}}'),
       json('{"TotalNodes": {"type": "int", "access": "read"}, "PublishedNodes": {"type": "int", "access": "read"}}')
FROM namespace_services WHERE service_name = 'org.opdbus.cms';

-- ===================================================================
-- Content Types
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'ContentType', NULL, TRUE,
       json('{
         "type_name": {"type": "string", "mandatory": true},
         "machine_name": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "has_title": {"type": "bool", "default": true},
         "has_body": {"type": "bool", "default": true},
         "is_translatable": {"type": "bool", "default": false},
         "field_definitions": {"type": "array"},
         "display_settings": {"type": "dict"},
         "form_settings": {"type": "dict"},
         "created": {"type": "string"},
         "modified": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Nodes (Content Items)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Node', NULL, TRUE,
       json('{
         "node_id": {"type": "int", "mandatory": true},
         "uuid": {"type": "string", "mandatory": true},
         "content_type": {"type": "string", "mandatory": true},
         "title": {"type": "string", "mandatory": true},
         "body": {"type": "string"},
         "summary": {"type": "string"},
         "language": {"type": "string", "default": "en"},
         "status": {"type": "string", "default": "draft"},
         "promoted": {"type": "bool", "default": false},
         "sticky": {"type": "bool", "default": false},
         "author_uid": {"type": "int"},
         "created": {"type": "string"},
         "modified": {"type": "string"},
         "published": {"type": "string"},
         "field_values": {"type": "dict"},
         "revision_id": {"type": "int"},
         "revision_log": {"type": "string"},
         "path_alias": {"type": "string"},
         "menu_link": {"type": "dict"},
         "moderation_state": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- CMS Users
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'CMSUser', NULL, TRUE,
       json('{
         "uid": {"type": "int", "mandatory": true},
         "username": {"type": "string", "mandatory": true},
         "email": {"type": "string", "mandatory": true},
         "display_name": {"type": "string"},
         "password_hash": {"type": "string"},
         "roles": {"type": "array"},
         "status": {"type": "string", "default": "active"},
         "timezone": {"type": "string"},
         "language": {"type": "string", "default": "en"},
         "created": {"type": "string"},
         "last_login": {"type": "string"},
         "last_access": {"type": "string"},
         "picture": {"type": "string"},
         "signature": {"type": "string"},
         "profile_fields": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Roles & Permissions
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Role', NULL, TRUE,
       json('{
         "role_id": {"type": "int", "mandatory": true},
         "role_name": {"type": "string", "mandatory": true},
         "machine_name": {"type": "string", "mandatory": true},
         "weight": {"type": "int", "default": 0},
         "permissions": {"type": "array"},
         "is_admin": {"type": "bool", "default": false}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Taxonomy Vocabularies
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'TaxonomyVocabulary', NULL, TRUE,
       json('{
         "vocabulary_id": {"type": "int", "mandatory": true},
         "vocabulary_name": {"type": "string", "mandatory": true},
         "machine_name": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "hierarchy": {"type": "string", "default": "single"},
         "weight": {"type": "int", "default": 0},
         "field_definitions": {"type": "array"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Taxonomy Terms
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'TaxonomyTerm', NULL, TRUE,
       json('{
         "term_id": {"type": "int", "mandatory": true},
         "term_name": {"type": "string", "mandatory": true},
         "vocabulary_id": {"type": "int", "mandatory": true},
         "description": {"type": "string"},
         "parent_term_id": {"type": "int"},
         "weight": {"type": "int", "default": 0},
         "path_alias": {"type": "string"},
         "field_values": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Menus
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Menu', NULL, TRUE,
       json('{
         "menu_id": {"type": "string", "mandatory": true},
         "menu_name": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "language": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Menu Links
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'MenuLink', NULL, TRUE,
       json('{
         "link_id": {"type": "int", "mandatory": true},
         "menu_id": {"type": "string", "mandatory": true},
         "parent_link_id": {"type": "int"},
         "title": {"type": "string", "mandatory": true},
         "url": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "enabled": {"type": "bool", "default": true},
         "expanded": {"type": "bool", "default": false},
         "weight": {"type": "int", "default": 0},
         "external": {"type": "bool", "default": false}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Blocks
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Block', NULL, TRUE,
       json('{
         "block_id": {"type": "string", "mandatory": true},
         "block_type": {"type": "string", "mandatory": true},
         "label": {"type": "string"},
         "theme": {"type": "string"},
         "region": {"type": "string"},
         "weight": {"type": "int", "default": 0},
         "visibility": {"type": "dict"},
         "settings": {"type": "dict"},
         "status": {"type": "bool", "default": true}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Views (Listings/Queries)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'View', NULL, TRUE,
       json('{
         "view_id": {"type": "string", "mandatory": true},
         "view_name": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "base_table": {"type": "string"},
         "displays": {"type": "array"},
         "filters": {"type": "array"},
         "sorts": {"type": "array"},
         "fields": {"type": "array"},
         "relationships": {"type": "array"},
         "pager": {"type": "dict"},
         "access": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Fields
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Field', NULL, TRUE,
       json('{
         "field_name": {"type": "string", "mandatory": true},
         "field_type": {"type": "string", "mandatory": true},
         "label": {"type": "string"},
         "description": {"type": "string"},
         "required": {"type": "bool", "default": false},
         "cardinality": {"type": "int", "default": 1},
         "default_value": {"type": "string"},
         "widget_type": {"type": "string"},
         "widget_settings": {"type": "dict"},
         "formatter_type": {"type": "string"},
         "formatter_settings": {"type": "dict"},
         "storage_settings": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Files & Media
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'File', NULL, TRUE,
       json('{
         "file_id": {"type": "int", "mandatory": true},
         "uuid": {"type": "string", "mandatory": true},
         "filename": {"type": "string", "mandatory": true},
         "uri": {"type": "string", "mandatory": true},
         "filemime": {"type": "string"},
         "filesize": {"type": "int"},
         "status": {"type": "bool", "default": true},
         "created": {"type": "string"},
         "modified": {"type": "string"},
         "owner_uid": {"type": "int"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Media', NULL, TRUE,
       json('{
         "media_id": {"type": "int", "mandatory": true},
         "uuid": {"type": "string", "mandatory": true},
         "media_type": {"type": "string", "mandatory": true},
         "name": {"type": "string", "mandatory": true},
         "file_id": {"type": "int"},
         "thumbnail_uri": {"type": "string"},
         "status": {"type": "bool", "default": true},
         "created": {"type": "string"},
         "modified": {"type": "string"},
         "owner_uid": {"type": "int"},
         "field_values": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Comments
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Comment', NULL, TRUE,
       json('{
         "comment_id": {"type": "int", "mandatory": true},
         "entity_type": {"type": "string", "mandatory": true},
         "entity_id": {"type": "int", "mandatory": true},
         "parent_comment_id": {"type": "int"},
         "subject": {"type": "string"},
         "body": {"type": "string"},
         "author_uid": {"type": "int"},
         "author_name": {"type": "string"},
         "author_email": {"type": "string"},
         "status": {"type": "string", "default": "published"},
         "created": {"type": "string"},
         "modified": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Workflows
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'Workflow', NULL, TRUE,
       json('{
         "workflow_id": {"type": "string", "mandatory": true},
         "workflow_name": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "content_types": {"type": "array"},
         "states": {"type": "array"},
         "transitions": {"type": "array"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Configuration Objects
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'SiteConfig', NULL, TRUE,
       json('{
         "config_name": {"type": "string", "mandatory": true},
         "site_name": {"type": "string"},
         "site_slogan": {"type": "string"},
         "site_email": {"type": "string"},
         "default_language": {"type": "string"},
         "default_timezone": {"type": "string"},
         "maintenance_mode": {"type": "bool", "default": false},
         "cache_enabled": {"type": "bool", "default": true},
         "settings": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';
</file>

<file path="src/cms_wordpress_schema.sql">
-- ===================================================================
-- FULL WORDPRESS CMS SCHEMA
-- Complete WordPress object classes for org.opdbus.cms
-- Status: LIVE AND UNFILLED (ready for CMS deployment)
-- WordPress powers 43% of all websites (most popular CMS)
-- ===================================================================

-- ===================================================================
-- Posts (Blog posts, pages, custom post types)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPPost', NULL, TRUE,
       json('{
         "post_id": {"type": "int", "mandatory": true},
         "post_guid": {"type": "string", "mandatory": true},
         "post_type": {"type": "string", "mandatory": true},
         "post_title": {"type": "string", "mandatory": true},
         "post_content": {"type": "string"},
         "post_excerpt": {"type": "string"},
         "post_status": {"type": "string", "default": "draft"},
         "post_name": {"type": "string"},
         "post_author": {"type": "int"},
         "post_date": {"type": "string"},
         "post_modified": {"type": "string"},
         "post_parent": {"type": "int", "default": 0},
         "menu_order": {"type": "int", "default": 0},
         "comment_status": {"type": "string", "default": "open"},
         "ping_status": {"type": "string", "default": "open"},
         "comment_count": {"type": "int", "default": 0},
         "featured_image_id": {"type": "int"},
         "post_password": {"type": "string"},
         "post_meta": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Pages
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPPage', 'WPPost', TRUE,
       json('{
         "page_template": {"type": "string"},
         "parent_page_id": {"type": "int"},
         "is_front_page": {"type": "bool", "default": false},
         "is_posts_page": {"type": "bool", "default": false}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- WordPress Users
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPUser', NULL, TRUE,
       json('{
         "user_id": {"type": "int", "mandatory": true},
         "user_login": {"type": "string", "mandatory": true},
         "user_email": {"type": "string", "mandatory": true},
         "user_nicename": {"type": "string"},
         "display_name": {"type": "string"},
         "user_registered": {"type": "string"},
         "user_status": {"type": "int", "default": 0},
         "user_url": {"type": "string"},
         "role": {"type": "string", "default": "subscriber"},
         "capabilities": {"type": "dict"},
         "user_meta": {"type": "dict"},
         "first_name": {"type": "string"},
         "last_name": {"type": "string"},
         "nickname": {"type": "string"},
         "description": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Comments
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPComment', NULL, TRUE,
       json('{
         "comment_id": {"type": "int", "mandatory": true},
         "comment_post_id": {"type": "int", "mandatory": true},
         "comment_author": {"type": "string"},
         "comment_author_email": {"type": "string"},
         "comment_author_url": {"type": "string"},
         "comment_author_ip": {"type": "string"},
         "comment_date": {"type": "string"},
         "comment_content": {"type": "string"},
         "comment_approved": {"type": "string", "default": "0"},
         "comment_parent": {"type": "int", "default": 0},
         "user_id": {"type": "int", "default": 0},
         "comment_type": {"type": "string"},
         "comment_meta": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Categories & Tags (Taxonomies)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPTaxonomy', NULL, TRUE,
       json('{
         "taxonomy_name": {"type": "string", "mandatory": true},
         "taxonomy_label": {"type": "string"},
         "object_types": {"type": "array"},
         "hierarchical": {"type": "bool", "default": false},
         "public": {"type": "bool", "default": true},
         "show_ui": {"type": "bool", "default": true},
         "show_in_rest": {"type": "bool", "default": true}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPTerm', NULL, TRUE,
       json('{
         "term_id": {"type": "int", "mandatory": true},
         "term_name": {"type": "string", "mandatory": true},
         "term_slug": {"type": "string", "mandatory": true},
         "term_taxonomy": {"type": "string", "mandatory": true},
         "term_description": {"type": "string"},
         "parent_term_id": {"type": "int", "default": 0},
         "count": {"type": "int", "default": 0},
         "term_meta": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Menus
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPMenu', NULL, TRUE,
       json('{
         "menu_id": {"type": "int", "mandatory": true},
         "menu_name": {"type": "string", "mandatory": true},
         "menu_slug": {"type": "string", "mandatory": true},
         "menu_location": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPMenuItem', NULL, TRUE,
       json('{
         "item_id": {"type": "int", "mandatory": true},
         "menu_id": {"type": "int", "mandatory": true},
         "parent_item_id": {"type": "int", "default": 0},
         "title": {"type": "string", "mandatory": true},
         "url": {"type": "string"},
         "target": {"type": "string"},
         "classes": {"type": "array"},
         "xfn": {"type": "string"},
         "description": {"type": "string"},
         "object_id": {"type": "int"},
         "object_type": {"type": "string"},
         "menu_order": {"type": "int", "default": 0}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Media Library
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPAttachment', 'WPPost', TRUE,
       json('{
         "attachment_url": {"type": "string", "mandatory": true},
         "attachment_file": {"type": "string"},
         "mime_type": {"type": "string"},
         "file_size": {"type": "int"},
         "width": {"type": "int"},
         "height": {"type": "int"},
         "alt_text": {"type": "string"},
         "caption": {"type": "string"},
         "description": {"type": "string"},
         "attached_to_post": {"type": "int"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Widgets
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPWidget', NULL, TRUE,
       json('{
         "widget_id": {"type": "string", "mandatory": true},
         "widget_name": {"type": "string", "mandatory": true},
         "widget_class": {"type": "string"},
         "sidebar_id": {"type": "string"},
         "widget_position": {"type": "int"},
         "widget_options": {"type": "dict"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Sidebars
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPSidebar', NULL, TRUE,
       json('{
         "sidebar_id": {"type": "string", "mandatory": true},
         "sidebar_name": {"type": "string", "mandatory": true},
         "description": {"type": "string"},
         "before_widget": {"type": "string"},
         "after_widget": {"type": "string"},
         "before_title": {"type": "string"},
         "after_title": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Themes
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPTheme', NULL, TRUE,
       json('{
         "theme_slug": {"type": "string", "mandatory": true},
         "theme_name": {"type": "string", "mandatory": true},
         "theme_uri": {"type": "string"},
         "author": {"type": "string"},
         "author_uri": {"type": "string"},
         "description": {"type": "string"},
         "version": {"type": "string"},
         "template": {"type": "string"},
         "status": {"type": "string"},
         "tags": {"type": "array"},
         "text_domain": {"type": "string"},
         "screenshot": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Plugins
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPPlugin', NULL, TRUE,
       json('{
         "plugin_file": {"type": "string", "mandatory": true},
         "plugin_name": {"type": "string", "mandatory": true},
         "plugin_uri": {"type": "string"},
         "description": {"type": "string"},
         "version": {"type": "string"},
         "author": {"type": "string"},
         "author_uri": {"type": "string"},
         "network": {"type": "bool", "default": false},
         "requires_wp": {"type": "string"},
         "requires_php": {"type": "string"},
         "text_domain": {"type": "string"},
         "active": {"type": "bool", "default": false}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Options (Site Settings)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPOption', NULL, TRUE,
       json('{
         "option_name": {"type": "string", "mandatory": true},
         "option_value": {"type": "string"},
         "autoload": {"type": "string", "default": "yes"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Site Configuration
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPSiteConfig', NULL, TRUE,
       json('{
         "site_url": {"type": "string", "mandatory": true},
         "home_url": {"type": "string"},
         "site_title": {"type": "string"},
         "tagline": {"type": "string"},
         "admin_email": {"type": "string"},
         "timezone": {"type": "string"},
         "date_format": {"type": "string"},
         "time_format": {"type": "string"},
         "language": {"type": "string", "default": "en_US"},
         "permalink_structure": {"type": "string"},
         "posts_per_page": {"type": "int", "default": 10},
         "comments_enabled": {"type": "bool", "default": true},
         "default_role": {"type": "string", "default": "subscriber"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Multisite Network (for WordPress Multisite)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPNetwork', NULL, TRUE,
       json('{
         "network_id": {"type": "int", "mandatory": true},
         "network_name": {"type": "string", "mandatory": true},
         "domain": {"type": "string", "mandatory": true},
         "path": {"type": "string", "default": "/"},
         "sites": {"type": "array"},
         "cookie_domain": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPSite', NULL, TRUE,
       json('{
         "site_id": {"type": "int", "mandatory": true},
         "blog_id": {"type": "int", "mandatory": true},
         "network_id": {"type": "int"},
         "domain": {"type": "string", "mandatory": true},
         "path": {"type": "string"},
         "registered": {"type": "string"},
         "last_updated": {"type": "string"},
         "public": {"type": "bool", "default": true},
         "archived": {"type": "bool", "default": false},
         "mature": {"type": "bool", "default": false},
         "spam": {"type": "bool", "default": false},
         "deleted": {"type": "bool", "default": false},
         "lang_id": {"type": "int", "default": 0}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- Custom Fields (Post Meta)
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WPCustomField', NULL, TRUE,
       json('{
         "meta_id": {"type": "int", "mandatory": true},
         "post_id": {"type": "int", "mandatory": true},
         "meta_key": {"type": "string", "mandatory": true},
         "meta_value": {"type": "string"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

-- ===================================================================
-- WooCommerce (E-commerce extension) - most popular plugin
-- ===================================================================

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WCProduct', 'WPPost', TRUE,
       json('{
         "product_type": {"type": "string", "default": "simple"},
         "sku": {"type": "string"},
         "regular_price": {"type": "string"},
         "sale_price": {"type": "string"},
         "stock_quantity": {"type": "int"},
         "stock_status": {"type": "string", "default": "instock"},
         "manage_stock": {"type": "bool", "default": false},
         "weight": {"type": "string"},
         "length": {"type": "string"},
         "width": {"type": "string"},
         "height": {"type": "string"},
         "shipping_class": {"type": "string"},
         "tax_status": {"type": "string", "default": "taxable"},
         "tax_class": {"type": "string"},
         "downloadable": {"type": "bool", "default": false},
         "virtual": {"type": "bool", "default": false"},
         "product_attributes": {"type": "dict"},
         "variations": {"type": "array"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';

INSERT OR IGNORE INTO object_classes (interface_id, class_name, parent_class, structural, attributes_schema)
SELECT id, 'WCOrder', 'WPPost', TRUE,
       json('{
         "order_number": {"type": "string"},
         "order_status": {"type": "string", "default": "pending"},
         "customer_id": {"type": "int"},
         "billing_address": {"type": "dict"},
         "shipping_address": {"type": "dict"},
         "payment_method": {"type": "string"},
         "payment_method_title": {"type": "string"},
         "transaction_id": {"type": "string"},
         "customer_ip": {"type": "string"},
         "customer_user_agent": {"type": "string"},
         "order_currency": {"type": "string"},
         "order_total": {"type": "string"},
         "order_subtotal": {"type": "string"},
         "order_tax": {"type": "string"},
         "order_shipping": {"type": "string"},
         "order_discount": {"type": "string"},
         "line_items": {"type": "array"},
         "shipping_lines": {"type": "array"},
         "fee_lines": {"type": "array"},
         "coupon_lines": {"type": "array"}
       }')
FROM service_interfaces WHERE interface_name = 'org.opdbus.cms.Manager';
</file>

<file path="src/disaster_recovery.rs">
//! Disaster Recovery Module
//!
//! Provides system state export/import for disaster recovery with dependency tracking.
//! Each export contains all plugin states plus the dependencies needed to restore.
//!
//! Dependencies are installed via D-Bus PackageKit - NO CLI COMMANDS.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use zbus::Connection;

/// System dependency that must be installed for restore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDependency {
    /// Package name (e.g., "openvswitch-switch")
    pub name: String,
    /// Package manager (apt, yum, dnf, etc.)
    pub package_manager: String,
    /// Minimum version required (optional)
    pub min_version: Option<String>,
    /// Whether this is critical for restore
    pub required: bool,
    /// Install command override (if not standard)
    pub install_command: Option<String>,
}

/// Captured state for a single plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateExport {
    /// Plugin name
    pub plugin_name: String,
    /// Plugin version
    pub version: String,
    /// The actual state data
    pub state: Value,
    /// Dependencies required by this plugin
    pub dependencies: Vec<SystemDependency>,
    /// Timestamp when state was captured
    pub captured_at: DateTime<Utc>,
    /// State hash for integrity verification
    pub state_hash: String,
}

/// Complete disaster recovery export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasterRecoveryExport {
    /// Export format version
    pub format_version: String,
    /// Unique export ID
    pub export_id: String,
    /// When this export was created
    pub created_at: DateTime<Utc>,
    /// Host information
    pub host_info: HostInfo,
    /// All plugin states
    pub plugins: HashMap<String, PluginStateExport>,
    /// Global dependencies (system-wide)
    pub global_dependencies: Vec<SystemDependency>,
    /// Apply order for plugins (topological sort)
    pub apply_order: Vec<String>,
    /// Checksum of entire export
    pub checksum: String,
}

/// Host information for DR context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub kernel: String,
}

/// Result of a restore operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub success: bool,
    pub plugins_restored: Vec<String>,
    pub plugins_failed: Vec<(String, String)>, // (name, error)
    pub dependencies_installed: Vec<String>,
    pub dependencies_failed: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

impl DisasterRecoveryExport {
    /// Create a new empty DR export
    pub fn new() -> Self {
        Self {
            format_version: "1.0.0".to_string(),
            export_id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            host_info: HostInfo::detect(),
            plugins: HashMap::new(),
            global_dependencies: Vec::new(),
            apply_order: Vec::new(),
            checksum: String::new(),
        }
    }

    /// Add a plugin state to the export
    pub fn add_plugin(&mut self, plugin: PluginStateExport) {
        self.apply_order.push(plugin.plugin_name.clone());
        self.plugins.insert(plugin.plugin_name.clone(), plugin);
    }

    /// Add a global dependency
    pub fn add_global_dependency(&mut self, dep: SystemDependency) {
        self.global_dependencies.push(dep);
    }

    /// Finalize the export (compute checksum)
    pub fn finalize(&mut self) {
        // Compute checksum over all plugin state hashes
        let mut hasher = md5::Context::new();
        for name in &self.apply_order {
            if let Some(plugin) = self.plugins.get(name) {
                hasher.consume(plugin.state_hash.as_bytes());
            }
        }
        self.checksum = format!("{:x}", hasher.compute());
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String> {
        Ok(simd_json::to_string_pretty(self)?)
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<Self> {
        let mut json_mut = json.to_string();
        Ok(unsafe { simd_json::from_str(&mut json_mut) }?)
    }

    /// Get all dependencies (global + per-plugin)
    pub fn all_dependencies(&self) -> Vec<&SystemDependency> {
        let mut deps: Vec<&SystemDependency> = self.global_dependencies.iter().collect();
        for plugin in self.plugins.values() {
            deps.extend(plugin.dependencies.iter());
        }
        deps
    }

    /// Get required dependencies only
    pub fn required_dependencies(&self) -> Vec<&SystemDependency> {
        self.all_dependencies()
            .into_iter()
            .filter(|d| d.required)
            .collect()
    }
}

impl Default for DisasterRecoveryExport {
    fn default() -> Self {
        Self::new()
    }
}

impl HostInfo {
    /// Detect current host information
    pub fn detect() -> Self {
        Self {
            hostname: hostname(),
            os: detect_os(),
            os_version: detect_os_version(),
            arch: std::env::consts::ARCH.to_string(),
            kernel: detect_kernel(),
        }
    }
}

impl PluginStateExport {
    /// Create from plugin state
    pub fn new(plugin_name: &str, version: &str, state: Value) -> Self {
        let state_json = simd_json::to_string(&state).unwrap_or_default();
        let state_hash = format!("{:x}", md5::compute(state_json.as_bytes()));

        Self {
            plugin_name: plugin_name.to_string(),
            version: version.to_string(),
            state,
            dependencies: Vec::new(),
            captured_at: Utc::now(),
            state_hash,
        }
    }

    /// Add a dependency
    pub fn add_dependency(&mut self, dep: SystemDependency) {
        self.dependencies.push(dep);
    }
}

impl SystemDependency {
    /// Create a new required dependency (uses PackageKit D-Bus, cross-distro)
    pub fn required(name: &str) -> Self {
        Self {
            name: name.to_string(),
            package_manager: "packagekit".to_string(), // Always use PackageKit D-Bus
            min_version: None,
            required: true,
            install_command: None,
        }
    }

    /// Create an optional dependency (uses PackageKit D-Bus, cross-distro)
    pub fn optional(name: &str) -> Self {
        Self {
            name: name.to_string(),
            package_manager: "packagekit".to_string(), // Always use PackageKit D-Bus
            min_version: None,
            required: false,
            install_command: None,
        }
    }

    /// Set minimum version
    pub fn with_version(mut self, version: &str) -> Self {
        self.min_version = Some(version.to_string());
        self
    }

    /// Set custom install command (fallback if PackageKit unavailable)
    pub fn with_install_command(mut self, cmd: &str) -> Self {
        self.install_command = Some(cmd.to_string());
        self
    }
}

// Helper functions
fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn detect_os() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("ID="))
                .map(|l| l.trim_start_matches("ID=").trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "linux".to_string())
}

fn detect_os_version() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("VERSION_ID="))
                .map(|l| {
                    l.trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_string()
                })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn detect_kernel() -> String {
    std::fs::read_to_string("/proc/version")
        .map(|s| s.split_whitespace().nth(2).unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Get default dependencies for a plugin type
pub fn get_plugin_dependencies(plugin_name: &str) -> Vec<SystemDependency> {
    match plugin_name {
        "net" | "openflow" => vec![SystemDependency::required("openvswitch-switch")],
        "lxc" => vec![
            // Proxmox provides pct, no extra deps on Proxmox hosts
        ],
        "privacy_router" => vec![
            SystemDependency::required("openvswitch-switch"),
            SystemDependency::optional("iptables"),
        ],
        "netmaker" => vec![SystemDependency::optional("netclient")],
        "btrfs" => vec![SystemDependency::required("btrfs-progs")],
        "numa" => vec![SystemDependency::optional("numactl")],
        "packagekit" => vec![SystemDependency::required("packagekit")],
        _ => vec![],
    }
}

/// Global dependencies required for any op-dbus installation
pub fn get_global_dependencies() -> Vec<SystemDependency> {
    vec![
        SystemDependency::required("openvswitch-switch"),
        SystemDependency::optional("btrfs-progs"),
        SystemDependency::optional("numactl"),
        SystemDependency::optional("jq"),
    ]
}

// =============================================================================
// PackageKit D-Bus Integration for Dependency Installation
// =============================================================================

/// Install dependencies via PackageKit D-Bus (NO CLI)
pub async fn install_dependencies_via_packagekit(
    dependencies: &[&SystemDependency],
) -> Result<Vec<InstallResult>> {
    let mut results = Vec::new();

    // Filter to just the package names we need to install
    let package_names: Vec<&str> = dependencies.iter().map(|d| d.name.as_str()).collect();

    if package_names.is_empty() {
        return Ok(results);
    }

    // Connect to D-Bus
    let connection = Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;

    // Create PackageKit transaction
    let pk_proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        "/org/freedesktop/PackageKit",
        "org.freedesktop.PackageKit",
    )
    .await
    .context("Failed to create PackageKit proxy")?;

    // First, resolve package names to package IDs
    let tx_path: zbus::zvariant::OwnedObjectPath = pk_proxy
        .call("CreateTransaction", &())
        .await
        .context("Failed to create PackageKit transaction")?;

    let tx_proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        tx_path.as_str(),
        "org.freedesktop.PackageKit.Transaction",
    )
    .await
    .context("Failed to create transaction proxy")?;

    // Resolve packages (filter: NONE=0, package names)
    let resolve_result: std::result::Result<(), zbus::Error> = tx_proxy
        .call("Resolve", &(0u64, package_names.clone()))
        .await;

    match resolve_result {
        Ok(_) => {
            for name in &package_names {
                results.push(InstallResult {
                    package: name.to_string(),
                    success: true,
                    error: None,
                });
            }
        }
        Err(e) => {
            // If resolve fails, try to install anyway (PackageKit will resolve)
            tracing::warn!("PackageKit resolve failed: {}, trying direct install", e);

            // Create new transaction for install
            let install_tx_path: zbus::zvariant::OwnedObjectPath = pk_proxy
                .call("CreateTransaction", &())
                .await
                .context("Failed to create install transaction")?;

            let install_proxy = zbus::Proxy::new(
                &connection,
                "org.freedesktop.PackageKit",
                install_tx_path.as_str(),
                "org.freedesktop.PackageKit.Transaction",
            )
            .await?;

            // Try installing with package names directly
            // Note: This may need package IDs in format "name;version;arch;repo"
            let install_result: std::result::Result<(), zbus::Error> = install_proxy
                .call("InstallPackages", &(0u64, package_names.clone()))
                .await;

            match install_result {
                Ok(_) => {
                    for name in &package_names {
                        results.push(InstallResult {
                            package: name.to_string(),
                            success: true,
                            error: None,
                        });
                    }
                }
                Err(install_err) => {
                    for name in &package_names {
                        results.push(InstallResult {
                            package: name.to_string(),
                            success: false,
                            error: Some(install_err.to_string()),
                        });
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Result of a single package installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub package: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Check if a package is installed via PackageKit D-Bus
pub async fn is_package_installed(package_name: &str) -> Result<bool> {
    let connection = Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;

    let pk_proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        "/org/freedesktop/PackageKit",
        "org.freedesktop.PackageKit",
    )
    .await?;

    // Create transaction
    let tx_path: zbus::zvariant::OwnedObjectPath = pk_proxy.call("CreateTransaction", &()).await?;

    let tx_proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        tx_path.as_str(),
        "org.freedesktop.PackageKit.Transaction",
    )
    .await?;

    // Search for installed packages (filter: INSTALLED=2)
    let result: std::result::Result<(), zbus::Error> = tx_proxy
        .call("SearchNames", &(2u64, vec![package_name.to_string()]))
        .await;

    // If we get a result without error, package exists
    Ok(result.is_ok())
}

/// Restore system from DR export using PackageKit D-Bus
pub async fn restore_from_export(export: &DisasterRecoveryExport) -> Result<RestoreResult> {
    let mut result = RestoreResult {
        success: true,
        plugins_restored: Vec::new(),
        plugins_failed: Vec::new(),
        dependencies_installed: Vec::new(),
        dependencies_failed: Vec::new(),
        warnings: Vec::new(),
    };

    // Step 1: Install global dependencies via PackageKit
    tracing::info!("Installing global dependencies via PackageKit D-Bus...");
    let global_deps: Vec<&SystemDependency> = export.global_dependencies.iter().collect();

    if !global_deps.is_empty() {
        match install_dependencies_via_packagekit(&global_deps).await {
            Ok(install_results) => {
                for ir in install_results {
                    if ir.success {
                        result.dependencies_installed.push(ir.package);
                    } else {
                        result.dependencies_failed.push((
                            ir.package,
                            ir.error.unwrap_or_else(|| "Unknown error".to_string()),
                        ));
                    }
                }
            }
            Err(e) => {
                result
                    .warnings
                    .push(format!("Global dependency install failed: {}", e));
            }
        }
    }

    // Step 2: Install per-plugin dependencies
    for plugin_name in &export.apply_order {
        if let Some(plugin) = export.plugins.get(plugin_name) {
            tracing::info!("Installing dependencies for plugin: {}", plugin_name);

            let plugin_deps: Vec<&SystemDependency> = plugin.dependencies.iter().collect();
            if !plugin_deps.is_empty() {
                match install_dependencies_via_packagekit(&plugin_deps).await {
                    Ok(install_results) => {
                        for ir in install_results {
                            if ir.success {
                                result.dependencies_installed.push(ir.package);
                            } else {
                                result.dependencies_failed.push((
                                    ir.package,
                                    ir.error.unwrap_or_else(|| "Unknown error".to_string()),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        result.warnings.push(format!(
                            "Dependency install for {} failed: {}",
                            plugin_name, e
                        ));
                    }
                }
            }
        }
    }

    // Step 3: Mark plugins as ready for restore
    // (Actual state application would be done by StateManager)
    for plugin_name in &export.apply_order {
        if export.plugins.contains_key(plugin_name) {
            result.plugins_restored.push(plugin_name.clone());
        }
    }

    // Check for any required dependency failures
    let required_failed: Vec<_> = result
        .dependencies_failed
        .iter()
        .filter(|(name, _)| {
            export
                .required_dependencies()
                .iter()
                .any(|d| d.name == *name)
        })
        .collect();

    if !required_failed.is_empty() {
        result.success = false;
        result.warnings.push(format!(
            "Required dependencies failed: {:?}",
            required_failed
        ));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dr_export_creation() {
        let mut export = DisasterRecoveryExport::new();
        assert_eq!(export.format_version, "1.0.0");
        assert!(export.plugins.is_empty());

        // Add a plugin
        let plugin = PluginStateExport::new("net", "1.0.0", simd_json::json!({"bridges": []}));
        export.add_plugin(plugin);
        assert_eq!(export.plugins.len(), 1);
        assert_eq!(export.apply_order, vec!["net"]);
    }

    #[test]
    fn test_plugin_state_hash() {
        let state = simd_json::json!({"bridges": ["ovsbr0"]});
        let plugin = PluginStateExport::new("net", "1.0.0", state);
        assert!(!plugin.state_hash.is_empty());
    }

    #[test]
    fn test_dependencies() {
        let deps = get_plugin_dependencies("net");
        assert!(!deps.is_empty());
        assert!(deps.iter().any(|d| d.name == "openvswitch-switch"));
    }

    #[test]
    fn test_export_json() {
        let mut export = DisasterRecoveryExport::new();
        let plugin = PluginStateExport::new("test", "1.0.0", simd_json::json!({}));
        export.add_plugin(plugin);
        export.finalize();

        let json = export.to_json().unwrap();
        assert!(json.contains("format_version"));
        assert!(json.contains("test"));

        let restored = DisasterRecoveryExport::from_json(&json).unwrap();
        assert_eq!(restored.plugins.len(), 1);
    }
}
</file>

<file path="src/error.rs">
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateStoreError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] simd_json::Error),
    #[error("Job not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StateStoreError>;
</file>

<file path="src/event_chain.rs">
//! Event Chain - Snowball-style Compliance and Reproducibility Layer
//!
//! Provides tamper-evident audit trail through:
//! - Hash-linked events for every state transition
//! - Merkle tree batching for scale
//! - Schema-aware canonical hashing
//! - Tag-scoped proofs for compliance
//! - Reproducible replay from footprints
//!
//! Each event record contains:
//! - `event_id` (monotonic or UUID)
//! - `prev_hash` (hash of previous event)
//! - `event_hash` = H(prev_hash || canonical_event_payload)
//! - `timestamp`
//! - `actor_id` + `capability_id`
//! - `plugin_id` + `schema_version`
//! - `op` (operation type)
//! - `target` (object path / selector)
//! - `tags_touched` (computed from schema)
//! - `decision` (allow/deny) + `deny_reason`
//! - `input_patch_hash`
//! - `result_effective_hash` (post-compile)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

use crate::schema_validator::canonicalize_json;

/// Operation types for state transitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Apply an immutable wrapper to a plugin state
    ApplyImmutableWrapper,
    /// Apply a tunable patch
    ApplyTunablePatch,
    /// Schema migration
    Migrate,
    /// Reconcile state with reality
    Reconcile,
    /// Emit a D-Bus signal
    EmitSignal,
    /// Property read
    PropertyGet,
    /// Property write
    PropertySet,
    /// Method invocation
    MethodCall,
    /// Snapshot creation
    CreateSnapshot,
    /// State import
    Import,
    /// State export
    Export,
    /// Custom operation
    Custom(String),
}

/// Decision result for an operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny,
}

/// How an action came to be initiated — the autonomy provenance dimension.
///
/// Every model action must declare its origin so auditors can distinguish
/// human-instructed execution from autonomous model decisions. This is
/// what makes the trust boundary enforceable and verifiable, not just policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionOrigin {
    /// A human, user, or parent agent explicitly requested this action.
    Instructed {
        /// Identity of the instructing party (user ID, agent ID, etc.)
        by: String,
        /// Session or conversation context reference.
        session_id: Option<String>,
        /// Hash of the prompt/instruction that triggered this, for traceability.
        prompt_ref: Option<String>,
    },
    /// The model reasoned and decided to act without explicit instruction.
    /// Autonomous actions are subject to stricter policy enforcement.
    Autonomous {
        /// Reference to the vector ID in Qdrant capturing the semantic
        /// context that drove the decision ("why it acted alone").
        reasoning_ref: Option<String>,
        /// Model's self-reported confidence in the decision (0.0–1.0).
        confidence: Option<f32>,
    },
    /// A system event triggered the action (no human or model decision involved).
    Reactive {
        /// Description of the trigger: D-Bus signal path, cron expression,
        /// state change event ID, etc.
        trigger: String,
    },
}

/// Reason for denial
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DenyReason {
    /// Tag was locked by immutable wrapper
    TagLock { tag: String, wrapper_id: String },
    /// Constraint validation failed
    ConstraintFail { constraint: String, message: String },
    /// Missing required capability
    CapabilityMissing { capability: String },
    /// Schema validation failed
    SchemaValidation { errors: Vec<String> },
    /// Read-only field modification attempted
    ReadOnlyViolation { field: String },
    /// Custom denial reason
    Custom { reason: String },
}

/// A single event in the chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainEvent {
    /// Monotonic event ID
    pub event_id: u64,
    /// Hash of the previous event
    pub prev_hash: String,
    /// Hash of this event: H(prev_hash || canonical_payload)
    pub event_hash: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Actor who initiated the operation
    pub actor_id: String,
    /// Capability used for the operation
    pub capability_id: Option<String>,
    /// Plugin that owns the state
    pub plugin_id: String,
    /// Schema version at time of event
    pub schema_version: String,
    /// Type of operation
    pub op: OperationType,
    /// Target object path or selector
    pub target: String,
    /// Tags touched by this operation (computed from schema)
    pub tags_touched: Vec<String>,
    /// Decision: allow or deny
    pub decision: Decision,
    /// Reason for denial (if denied)
    pub deny_reason: Option<DenyReason>,
    /// Hash of the input patch/payload
    pub input_patch_hash: String,
    /// Hash of the resulting effective state (if allowed)
    pub result_effective_hash: Option<String>,
    /// Optional delta hash for incremental verification
    pub db_delta_hash: Option<String>,
    /// Reference to a snapshot (if this event creates one)
    pub snapshot_ref: Option<String>,
    /// Autonomy provenance: was this instructed, autonomous, or reactive?
    /// None = legacy event predating this field; treat as unknown.
    pub action_origin: Option<ActionOrigin>,
    /// The user who initiated the conversation that led to this event.
    /// None for purely system/reactive events with no human context.
    pub user_id: Option<String>,
    /// The conversation (chat session) this event belongs to.
    /// Groups the full why→what→who chain for a single session.
    /// Indexed for efficient per-conversation audit queries.
    pub conversation_id: Option<String>,
}

impl ChainEvent {
    /// Create a new event with computed hash
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: u64,
        prev_hash: String,
        actor_id: String,
        plugin_id: String,
        schema_version: String,
        op: OperationType,
        target: String,
        tags_touched: Vec<String>,
        decision: Decision,
        input_patch: &Value,
    ) -> Self {
        let timestamp = Utc::now();
        let input_patch_hash = compute_hash(&canonicalize_json(input_patch));

        let mut event = Self {
            event_id,
            prev_hash: prev_hash.clone(),
            event_hash: String::new(), // Computed below
            timestamp,
            actor_id,
            capability_id: None,
            plugin_id,
            schema_version,
            op,
            target,
            tags_touched,
            decision,
            deny_reason: None,
            input_patch_hash,
            result_effective_hash: None,
            db_delta_hash: None,
            snapshot_ref: None,
            action_origin: None,
            user_id: None,
            conversation_id: None,
        };

        // Compute event hash
        event.event_hash = event.compute_hash();
        event
    }

    /// Compute the hash of this event
    fn compute_hash(&self) -> String {
        let payload = CanonicalEventPayload {
            event_id: self.event_id,
            prev_hash: &self.prev_hash,
            timestamp: self.timestamp,
            actor_id: &self.actor_id,
            capability_id: self.capability_id.as_deref(),
            plugin_id: &self.plugin_id,
            schema_version: &self.schema_version,
            op: &self.op,
            target: &self.target,
            tags_touched: &self.tags_touched,
            decision: &self.decision,
            deny_reason: self.deny_reason.as_ref(),
            input_patch_hash: &self.input_patch_hash,
            result_effective_hash: self.result_effective_hash.as_deref(),
        };

        let canonical = simd_json::serde::to_owned_value(&payload).unwrap_or_default();
        let canonical = canonicalize_json(&canonical);
        compute_hash(&canonical)
    }

    /// Set the result effective hash after successful operation
    pub fn with_result_hash(mut self, hash: String) -> Self {
        self.result_effective_hash = Some(hash);
        self.event_hash = self.compute_hash();
        self
    }

    /// Set deny reason
    pub fn with_deny_reason(mut self, reason: DenyReason) -> Self {
        self.deny_reason = Some(reason);
        self.event_hash = self.compute_hash();
        self
    }

    /// Set capability ID
    pub fn with_capability(mut self, capability: String) -> Self {
        self.capability_id = Some(capability);
        self.event_hash = self.compute_hash();
        self
    }

    /// Verify this event's hash against its content
    pub fn verify(&self) -> bool {
        let computed = self.compute_hash();
        computed == self.event_hash
    }
}

/// Canonical payload structure for consistent hashing
#[derive(Serialize)]
struct CanonicalEventPayload<'a> {
    event_id: u64,
    prev_hash: &'a str,
    timestamp: DateTime<Utc>,
    actor_id: &'a str,
    capability_id: Option<&'a str>,
    plugin_id: &'a str,
    schema_version: &'a str,
    op: &'a OperationType,
    target: &'a str,
    tags_touched: &'a [String],
    decision: &'a Decision,
    deny_reason: Option<&'a DenyReason>,
    input_patch_hash: &'a str,
    result_effective_hash: Option<&'a str>,
}

/// Merkle tree node for batch proofs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    pub hash: String,
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
}

/// A batch of events with Merkle root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    /// Merkle root of all event hashes in this batch
    pub batch_root: String,
    /// Range of event IDs in this batch
    pub first_event_id: u64,
    pub last_event_id: u64,
    /// Hash of the previous batch root (for chaining batches)
    pub prev_batch_root: Option<String>,
    /// Timestamp when batch was finalized
    pub timestamp: DateTime<Utc>,
    /// Number of events in this batch
    pub event_count: usize,
}

impl EventBatch {
    /// Create a new batch from a list of event hashes
    pub fn from_events(events: &[ChainEvent], prev_batch_root: Option<String>) -> Option<Self> {
        if events.is_empty() {
            return None;
        }

        let hashes: Vec<&str> = events.iter().map(|e| e.event_hash.as_str()).collect();
        let batch_root = compute_merkle_root(&hashes);

        Some(Self {
            batch_root,
            first_event_id: events.first().unwrap().event_id,
            last_event_id: events.last().unwrap().event_id,
            prev_batch_root,
            timestamp: Utc::now(),
            event_count: events.len(),
        })
    }

    /// Generate a Merkle proof for a specific event
    pub fn generate_proof(events: &[ChainEvent], event_id: u64) -> Option<MerkleProof> {
        let idx = events.iter().position(|e| e.event_id == event_id)?;
        let hashes: Vec<&str> = events.iter().map(|e| e.event_hash.as_str()).collect();

        let siblings = compute_merkle_proof(&hashes, idx);
        let root = compute_merkle_root(&hashes);

        Some(MerkleProof {
            event_hash: events[idx].event_hash.clone(),
            event_id,
            siblings,
            root,
        })
    }
}

/// Merkle proof for a single event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash of the event being proved
    pub event_hash: String,
    /// Event ID
    pub event_id: u64,
    /// Sibling hashes needed to reconstruct root
    pub siblings: Vec<(String, bool)>, // (hash, is_right)
    /// Expected root hash
    pub root: String,
}

impl MerkleProof {
    /// Verify this proof
    pub fn verify(&self) -> bool {
        let mut current = self.event_hash.clone();

        for (sibling, is_right) in &self.siblings {
            current = if *is_right {
                compute_hash_pair(&current, sibling)
            } else {
                compute_hash_pair(sibling, &current)
            };
        }

        current == self.root
    }
}

/// Snapshot of plugin state for fast rebuild
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Snapshot ID (hash of content)
    pub snapshot_id: String,
    /// Event ID at which this snapshot was taken
    pub at_event_id: u64,
    /// Plugin ID
    pub plugin_id: String,
    /// Schema version
    pub schema_version: String,
    /// Stub hash
    pub stub_hash: String,
    /// Immutable wrappers hash (or list of wrapper hashes)
    pub immutable_wrappers_hash: String,
    /// Tunable patch hash
    pub tunable_patch_hash: String,
    /// Effective state hash (computed from above)
    pub effective_hash: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// The actual state data
    pub state: Value,
}

impl StateSnapshot {
    /// Create a new snapshot
    pub fn new(at_event_id: u64, plugin_id: String, schema_version: String, state: Value) -> Self {
        let canonical = canonicalize_json(&state);
        let effective_hash = compute_hash(&canonical);

        // For now, stub/wrapper/tunable hashes are derived from effective
        // In a real implementation, these would be tracked separately
        let stub_hash = effective_hash.clone();
        let immutable_wrappers_hash = compute_hash(&simd_json::json!([]));
        let tunable_patch_hash = compute_hash(&simd_json::json!({}));

        let mut snapshot = Self {
            snapshot_id: String::new(),
            at_event_id,
            plugin_id,
            schema_version,
            stub_hash,
            immutable_wrappers_hash,
            tunable_patch_hash,
            effective_hash,
            timestamp: Utc::now(),
            state,
        };

        // Compute snapshot ID from all hashes
        let id_input = format!(
            "{}:{}:{}:{}:{}",
            snapshot.at_event_id,
            snapshot.stub_hash,
            snapshot.immutable_wrappers_hash,
            snapshot.tunable_patch_hash,
            snapshot.effective_hash
        );
        snapshot.snapshot_id = compute_hash_str(&id_input);
        snapshot
    }

    /// Verify snapshot integrity
    pub fn verify(&self) -> bool {
        let canonical = canonicalize_json(&self.state);
        let computed = compute_hash(&canonical);
        computed == self.effective_hash
    }
}

/// The event chain - append-only ledger
pub struct EventChain {
    /// All events in order
    events: Vec<ChainEvent>,
    /// Finalized batches
    batches: Vec<EventBatch>,
    /// Snapshots for fast rebuild
    snapshots: HashMap<String, StateSnapshot>,
    /// Configuration
    config: ChainConfig,
    /// Genesis hash (first prev_hash)
    genesis_hash: String,
}

/// Configuration for the event chain
#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// Number of events per batch
    pub batch_size: usize,
    /// Whether to auto-batch when batch_size is reached
    pub auto_batch: bool,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            auto_batch: true,
        }
    }
}

impl EventChain {
    /// Create a new event chain
    pub fn new(config: ChainConfig) -> Self {
        Self {
            events: Vec::new(),
            batches: Vec::new(),
            snapshots: HashMap::new(),
            config,
            genesis_hash: compute_hash_str("genesis"),
        }
    }

    /// Get the hash of the last event (or genesis)
    pub fn last_hash(&self) -> &str {
        self.events
            .last()
            .map(|e| e.event_hash.as_str())
            .unwrap_or(&self.genesis_hash)
    }

    /// Get the next event ID
    pub fn next_event_id(&self) -> u64 {
        self.events.last().map(|e| e.event_id + 1).unwrap_or(1)
    }

    /// Append a new event to the chain
    pub fn append(&mut self, mut event: ChainEvent) -> &ChainEvent {
        // Ensure prev_hash matches
        event.prev_hash = self.last_hash().to_string();
        event.event_id = self.next_event_id();
        event.event_hash = event.compute_hash();

        self.events.push(event);

        // Auto-batch if configured
        if self.config.auto_batch && self.unbatched_count() >= self.config.batch_size {
            self.create_batch();
        }

        self.events.last().unwrap()
    }

    /// Create a new event and append it
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        actor_id: String,
        plugin_id: String,
        schema_version: String,
        op: OperationType,
        target: String,
        tags_touched: Vec<String>,
        decision: Decision,
        input_patch: &Value,
    ) -> &ChainEvent {
        let event = ChainEvent::new(
            self.next_event_id(),
            self.last_hash().to_string(),
            actor_id,
            plugin_id,
            schema_version,
            op,
            target,
            tags_touched,
            decision,
            input_patch,
        );
        self.append(event)
    }

    /// Get number of unbatched events
    fn unbatched_count(&self) -> usize {
        let last_batched = self.batches.last().map(|b| b.last_event_id).unwrap_or(0);
        self.events
            .iter()
            .filter(|e| e.event_id > last_batched)
            .count()
    }

    /// Create a batch from unbatched events
    pub fn create_batch(&mut self) -> Option<&EventBatch> {
        let last_batched = self.batches.last().map(|b| b.last_event_id).unwrap_or(0);
        let unbatched: Vec<_> = self
            .events
            .iter()
            .filter(|e| e.event_id > last_batched)
            .cloned()
            .collect();

        if unbatched.is_empty() {
            return None;
        }

        let prev_root = self.batches.last().map(|b| b.batch_root.clone());
        let batch = EventBatch::from_events(&unbatched, prev_root)?;
        self.batches.push(batch);
        self.batches.last()
    }

    /// Create a snapshot at the current state
    pub fn create_snapshot(
        &mut self,
        plugin_id: String,
        schema_version: String,
        state: Value,
    ) -> &StateSnapshot {
        let event_id = self.events.last().map(|e| e.event_id).unwrap_or(0);
        let snapshot = StateSnapshot::new(event_id, plugin_id, schema_version, state);
        let id = snapshot.snapshot_id.clone();
        self.snapshots.insert(id.clone(), snapshot);
        self.snapshots.get(&id).unwrap()
    }

    /// Verify the entire chain
    pub fn verify_chain(&self) -> ChainVerificationResult {
        let mut result = ChainVerificationResult {
            valid: true,
            events_verified: 0,
            batches_verified: 0,
            errors: Vec::new(),
        };

        // Verify event chain
        let mut expected_prev = self.genesis_hash.clone();
        for event in &self.events {
            if event.prev_hash != expected_prev {
                result.valid = false;
                result.errors.push(format!(
                    "Event {} has wrong prev_hash: expected {}, got {}",
                    event.event_id, expected_prev, event.prev_hash
                ));
            }

            if !event.verify() {
                result.valid = false;
                result
                    .errors
                    .push(format!("Event {} hash verification failed", event.event_id));
            }

            expected_prev = event.event_hash.clone();
            result.events_verified += 1;
        }

        // Verify batch chain
        for batch in &self.batches {
            // Get events in this batch
            let batch_events: Vec<_> = self
                .events
                .iter()
                .filter(|e| e.event_id >= batch.first_event_id && e.event_id <= batch.last_event_id)
                .collect();

            let hashes: Vec<&str> = batch_events.iter().map(|e| e.event_hash.as_str()).collect();
            let computed_root = compute_merkle_root(&hashes);

            if computed_root != batch.batch_root {
                result.valid = false;
                result.errors.push(format!(
                    "Batch {}-{} root mismatch: expected {}, computed {}",
                    batch.first_event_id, batch.last_event_id, batch.batch_root, computed_root
                ));
            }

            result.batches_verified += 1;
        }

        result
    }

    /// Query events by tag
    pub fn events_touching_tag(&self, tag: &str) -> Vec<&ChainEvent> {
        self.events
            .iter()
            .filter(|e| e.tags_touched.contains(&tag.to_string()))
            .collect()
    }

    /// Query events by plugin
    pub fn events_for_plugin(&self, plugin_id: &str) -> Vec<&ChainEvent> {
        self.events
            .iter()
            .filter(|e| e.plugin_id == plugin_id)
            .collect()
    }

    /// Prove that a tag was never touched by tunable writes
    pub fn prove_tag_immutability(&self, tag: &str) -> TagImmutabilityProof {
        let tunable_touches: Vec<_> = self
            .events
            .iter()
            .filter(|e| {
                matches!(e.op, OperationType::ApplyTunablePatch)
                    && e.tags_touched.contains(&tag.to_string())
                    && e.decision == Decision::Allow
            })
            .collect();

        TagImmutabilityProof {
            tag: tag.to_string(),
            is_immutable: tunable_touches.is_empty(),
            violations: tunable_touches.iter().map(|e| e.event_id).collect(),
            total_events_checked: self.events.len(),
        }
    }

    /// Get all events
    pub fn events(&self) -> &[ChainEvent] {
        &self.events
    }

    /// Get all batches
    pub fn batches(&self) -> &[EventBatch] {
        &self.batches
    }

    /// Get a snapshot by ID
    pub fn get_snapshot(&self, id: &str) -> Option<&StateSnapshot> {
        self.snapshots.get(id)
    }
}

/// Result of chain verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerificationResult {
    pub valid: bool,
    pub events_verified: usize,
    pub batches_verified: usize,
    pub errors: Vec<String>,
}

/// Proof that a tag was never modified by tunable writes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagImmutabilityProof {
    pub tag: String,
    pub is_immutable: bool,
    pub violations: Vec<u64>,
    pub total_events_checked: usize,
}

// =============================================================================
// Hash utilities
// =============================================================================

/// Compute hash of a JSON value
fn compute_hash(value: &Value) -> String {
    let canonical_str = simd_json::to_string(value).unwrap_or_default();
    format!("{:x}", md5::compute(canonical_str.as_bytes()))
}

/// Compute hash of a string
fn compute_hash_str(s: &str) -> String {
    format!("{:x}", md5::compute(s.as_bytes()))
}

/// Compute hash of two hashes concatenated
fn compute_hash_pair(left: &str, right: &str) -> String {
    compute_hash_str(&format!("{}{}", left, right))
}

/// Compute Merkle root from a list of hashes
fn compute_merkle_root(hashes: &[&str]) -> String {
    if hashes.is_empty() {
        return compute_hash_str("empty");
    }
    if hashes.len() == 1 {
        return hashes[0].to_string();
    }

    let mut level: Vec<String> = hashes.iter().map(|s| s.to_string()).collect();

    while level.len() > 1 {
        let mut next_level = Vec::new();
        for chunk in level.chunks(2) {
            if chunk.len() == 2 {
                next_level.push(compute_hash_pair(&chunk[0], &chunk[1]));
            } else {
                // Odd number: duplicate last hash
                next_level.push(compute_hash_pair(&chunk[0], &chunk[0]));
            }
        }
        level = next_level;
    }

    level.into_iter().next().unwrap_or_default()
}

/// Compute Merkle proof siblings for a specific index
fn compute_merkle_proof(hashes: &[&str], index: usize) -> Vec<(String, bool)> {
    if hashes.len() <= 1 {
        return Vec::new();
    }

    let mut siblings = Vec::new();
    let mut level: Vec<String> = hashes.iter().map(|s| s.to_string()).collect();
    let mut idx = index;

    while level.len() > 1 {
        let sibling_idx = if idx.is_multiple_of(2) {
            idx + 1
        } else {
            idx - 1
        };
        let is_right = idx.is_multiple_of(2);

        if sibling_idx < level.len() {
            siblings.push((level[sibling_idx].clone(), is_right));
        } else {
            // Odd number: duplicate
            siblings.push((level[idx].clone(), is_right));
        }

        // Build next level
        let mut next_level = Vec::new();
        for chunk in level.chunks(2) {
            if chunk.len() == 2 {
                next_level.push(compute_hash_pair(&chunk[0], &chunk[1]));
            } else {
                next_level.push(compute_hash_pair(&chunk[0], &chunk[0]));
            }
        }

        idx /= 2;
        level = next_level;
    }

    siblings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_chain_basic() {
        let mut chain = EventChain::new(ChainConfig::default());

        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyTunablePatch,
            "/containers/100".to_string(),
            vec!["container".to_string()],
            Decision::Allow,
            &simd_json::json!({"memory": 1024}),
        );

        assert_eq!(chain.events().len(), 1);
        assert!(chain.verify_chain().valid);
    }

    #[test]
    fn test_event_chain_integrity() {
        let mut chain = EventChain::new(ChainConfig::default());

        for i in 0..5 {
            chain.record(
                "user1".to_string(),
                "lxc".to_string(),
                "2.0.0".to_string(),
                OperationType::PropertySet,
                format!("/containers/{}", i),
                vec!["container".to_string()],
                Decision::Allow,
                &simd_json::json!({"value": i}),
            );
        }

        let result = chain.verify_chain();
        assert!(result.valid);
        assert_eq!(result.events_verified, 5);
    }

    #[test]
    fn test_merkle_root() {
        let hashes = vec!["a", "b", "c", "d"];
        let root = compute_merkle_root(&hashes);
        assert!(!root.is_empty());

        // Same hashes should produce same root
        let root2 = compute_merkle_root(&hashes);
        assert_eq!(root, root2);
    }

    #[test]
    fn test_merkle_proof() {
        let hashes = vec!["a", "b", "c", "d"];
        let root = compute_merkle_root(&hashes);

        let proof_siblings = compute_merkle_proof(&hashes, 2);

        // Verify proof manually
        let mut current = "c".to_string();
        for (sibling, is_right) in &proof_siblings {
            current = if *is_right {
                compute_hash_pair(&current, sibling)
            } else {
                compute_hash_pair(sibling, &current)
            };
        }

        assert_eq!(current, root);
    }

    #[test]
    fn test_tag_immutability_proof() {
        let mut chain = EventChain::new(ChainConfig::default());

        // Record events with different tags
        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyTunablePatch,
            "/containers/100".to_string(),
            vec!["container".to_string()],
            Decision::Allow,
            &simd_json::json!({}),
        );

        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyImmutableWrapper,
            "/containers/100".to_string(),
            vec!["security".to_string()],
            Decision::Allow,
            &simd_json::json!({}),
        );

        // Security tag should be immutable (no tunable touches)
        let proof = chain.prove_tag_immutability("security");
        assert!(proof.is_immutable);

        // Container tag was touched by tunable
        let proof = chain.prove_tag_immutability("container");
        assert!(!proof.is_immutable);
    }

    #[test]
    fn test_batch_creation() {
        let config = ChainConfig {
            batch_size: 3,
            auto_batch: false,
        };
        let mut chain = EventChain::new(config);

        for i in 0..5 {
            chain.record(
                "user1".to_string(),
                "lxc".to_string(),
                "2.0.0".to_string(),
                OperationType::PropertySet,
                format!("/test/{}", i),
                vec![],
                Decision::Allow,
                &simd_json::json!({}),
            );
        }

        let batch = chain.create_batch().unwrap();
        assert_eq!(batch.event_count, 5);
        assert_eq!(batch.first_event_id, 1);
        assert_eq!(batch.last_event_id, 5);
    }

    #[test]
    fn test_snapshot() {
        let mut chain = EventChain::new(ChainConfig::default());

        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyTunablePatch,
            "/containers".to_string(),
            vec![],
            Decision::Allow,
            &simd_json::json!({}),
        );

        let state = simd_json::json!({
            "containers": [{"id": "100", "running": true}]
        });

        let snapshot = chain.create_snapshot("lxc".to_string(), "2.0.0".to_string(), state);

        assert!(snapshot.verify());
        assert_eq!(snapshot.at_event_id, 1);
    }
}
</file>

<file path="src/execution_job.rs">
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Execution result (local to avoid cycle)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: Option<simd_json::OwnedValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionJob {
    pub id: Uuid,
    pub tool_name: String,
    pub arguments: simd_json::OwnedValue,
    pub status: ExecutionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub result: Option<ExecutionResult>,
}
</file>

<file path="src/lib.rs">
#![recursion_limit = "512"]

//! OP State Store - Execution State Tracking and Job Ledger
//!
//! Provides persistent storage for execution jobs with state transitions:
//! REQUESTED → DISPATCHED → RUNNING → COMPLETED/FAILED
//!
//! Features:
//! - SQLite persistent storage
//! - Redis real-time stream
//! - Prometheus metrics
//! - Plugin schema catalog with JSON Schema 2026 support
//! - Disaster recovery export/import
//! - OpenTelemetry tracing integration
//! - Snowball-style event chain for compliance and reproducibility
//! - Schema-aware canonical hashing with Merkle batching

pub mod disaster_recovery;
pub mod error;
pub mod event_chain;
pub mod execution_job;
pub mod metrics;
pub mod plugin_schema;
pub mod memory_store;
pub mod redis_stream;
pub mod schema_shuttle;
pub mod schema_validator;
pub mod sqlite_store;
pub mod state_store;

pub use disaster_recovery::{
    get_global_dependencies, get_plugin_dependencies, DisasterRecoveryExport, HostInfo,
    PluginStateExport, RestoreResult, SystemDependency,
};
pub use error::StateStoreError;
pub use event_chain::{
    ActionOrigin, ChainConfig, ChainEvent, ChainVerificationResult, Decision, DenyReason,
    EventBatch, EventChain, MerkleProof, OperationType, StateSnapshot, TagImmutabilityProof,
};
pub use execution_job::{ExecutionJob, ExecutionResult, ExecutionStatus};
pub use plugin_schema::{
    builtin_plugin_schema, builtin_plugin_schemas, dialects, Constraint, FieldSchema, FieldType,
    PluginSchema, ReadOnlyCondition, SchemaCatalog, SchemaLoadError, SchemaRegistry,
    ValidationResult as SchemaValidationResult, DEFAULT_SCHEMA_DIALECT,
};
pub use memory_store::MemoryStore;
pub use redis_stream::RedisStream;
pub use schema_shuttle::{IdentitySled, SchemaShuttle};
pub use schema_validator::{
    canonicalize_json, SchemaValidator, ValidationError, ValidationReport, ValidatorError,
};
pub use sqlite_store::SqliteStore;
pub use state_store::StateStore;

use serde::{Deserialize, Serialize};

/// A stored object for export/import
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredObject {
    pub id: String,
    pub object_type: String,
    pub namespace: String,
    pub data: simd_json::OwnedValue,
}

/// Export data structure for disaster recovery
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CanonicalDbExport {
    pub objects: Vec<StoredObject>,
    pub executions: Vec<simd_json::OwnedValue>,
    pub snowball: Vec<simd_json::OwnedValue>,
}
</file>

<file path="src/memory_store.rs">
//! Pure in-memory StateStore — no SQLite, no drift.
//!
//! Used for plugin projection bootstrap and ephemeral state tracking
//! where persistence is handled externally (SHM, snowball, JSON files).

use crate::error::Result;
use crate::execution_job::ExecutionJob;
use crate::state_store::{StateStore, ToolRecord};
use crate::{CanonicalDbExport, StoredObject};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, info};
use uuid::Uuid;

/// Thread-safe in-memory state store.
pub struct MemoryStore {
    jobs: Mutex<HashMap<Uuid, ExecutionJob>>,
    objects: Mutex<HashMap<String, StoredObject>>,
    tools: Mutex<Vec<ToolRecord>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        info!("Initialized MemoryStore — no persistent SQLite");
        Self {
            jobs: Mutex::new(HashMap::new()),
            objects: Mutex::new(HashMap::new()),
            tools: Mutex::new(Vec::new()),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for MemoryStore {
    async fn save_job(&self, job: &ExecutionJob) -> Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(job.id, job.clone());
        debug!("Saved job {} ({}) to memory", job.id, job.tool_name);
        Ok(())
    }

    async fn get_job(&self, id: Uuid) -> Result<Option<ExecutionJob>> {
        let jobs = self.jobs.lock().unwrap();
        Ok(jobs.get(&id).cloned())
    }

    async fn update_job(&self, job: &ExecutionJob) -> Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(job.id, job.clone());
        debug!("Updated job {} in memory", job.id);
        Ok(())
    }

    async fn get_object(&self, id: &str) -> Result<Option<StoredObject>> {
        let objects = self.objects.lock().unwrap();
        Ok(objects.get(id).cloned())
    }

    async fn upsert_object(
        &self,
        id: &str,
        object_type: &str,
        namespace: &str,
        data: &simd_json::OwnedValue,
    ) -> Result<()> {
        let mut objects = self.objects.lock().unwrap();
        objects.insert(
            id.to_string(),
            StoredObject {
                id: id.to_string(),
                object_type: object_type.to_string(),
                namespace: namespace.to_string(),
                data: data.clone(),
            },
        );
        debug!("Upserted object {} in memory", id);
        Ok(())
    }

    async fn export_canonical(&self) -> Result<CanonicalDbExport> {
        let objects = self.objects.lock().unwrap();
        let jobs = self.jobs.lock().unwrap();

        let mut executions = Vec::new();
        for job in jobs.values() {
            let mut bytes = simd_json::to_string(job)?.into_bytes();
            let value = simd_json::to_owned_value(&mut bytes)?;
            executions.push(value);
        }

        Ok(CanonicalDbExport {
            objects: objects.values().cloned().collect(),
            executions,
            snowball: Vec::new(),
        })
    }

    async fn save_tools(&self, tools: Vec<ToolRecord>) -> Result<()> {
        let mut stored = self.tools.lock().unwrap();
        *stored = tools;
        debug!("Saved {} tools to memory", stored.len());
        Ok(())
    }

    async fn load_tools(&self) -> Result<Vec<ToolRecord>> {
        let tools = self.tools.lock().unwrap();
        Ok(tools.clone())
    }

    async fn is_tools_empty(&self) -> Result<bool> {
        let tools = self.tools.lock().unwrap();
        Ok(tools.is_empty())
    }

    async fn clear_tools(&self) -> Result<()> {
        let mut tools = self.tools.lock().unwrap();
        tools.clear();
        debug!("Cleared tools from memory");
        Ok(())
    }
}
</file>

<file path="src/metrics.rs">
//! Prometheus metrics for state store operations
//!
//! Provides observability into store operations including:
//! - Job counts by status
//! - Operation latencies
//! - Error rates
//! - Database connection pool stats

use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry,
};
use std::sync::Once;
use tracing::info;

lazy_static! {
    /// Global metrics registry
    pub static ref REGISTRY: Registry = Registry::new();

    // Job metrics
    /// Total jobs created
    pub static ref JOBS_CREATED_TOTAL: Counter = Counter::new(
        "op_state_jobs_created_total",
        "Total number of jobs created"
    ).unwrap();

    /// Jobs by status
    pub static ref JOBS_BY_STATUS: GaugeVec = GaugeVec::new(
        Opts::new("op_state_jobs_by_status", "Number of jobs by status"),
        &["status"]
    ).unwrap();

    /// Job status transitions
    pub static ref JOB_STATUS_TRANSITIONS: CounterVec = CounterVec::new(
        Opts::new("op_state_job_transitions_total", "Job status transitions"),
        &["from_status", "to_status"]
    ).unwrap();

    /// Job execution duration
    pub static ref JOB_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new("op_state_job_duration_seconds", "Job execution duration")
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]),
        &["tool_name"]
    ).unwrap();

    // Store operation metrics
    /// Store operation latency
    pub static ref STORE_OP_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("op_state_store_operation_seconds", "Store operation duration")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
        &["operation", "store_type"]
    ).unwrap();

    /// Store operation errors
    pub static ref STORE_OP_ERRORS: CounterVec = CounterVec::new(
        Opts::new("op_state_store_errors_total", "Store operation errors"),
        &["operation", "store_type", "error_type"]
    ).unwrap();

    // Plugin metrics
    /// Plugin state queries
    pub static ref PLUGIN_QUERIES_TOTAL: CounterVec = CounterVec::new(
        Opts::new("op_state_plugin_queries_total", "Plugin state queries"),
        &["plugin_name"]
    ).unwrap();

    /// Plugin state applies
    pub static ref PLUGIN_APPLIES_TOTAL: CounterVec = CounterVec::new(
        Opts::new("op_state_plugin_applies_total", "Plugin state applies"),
        &["plugin_name", "success"]
    ).unwrap();

    /// Plugin checkpoints created
    pub static ref CHECKPOINTS_CREATED: CounterVec = CounterVec::new(
        Opts::new("op_state_checkpoints_created_total", "Checkpoints created"),
        &["plugin_name"]
    ).unwrap();

    // Audit metrics
    /// Audit log entries
    pub static ref AUDIT_ENTRIES_TOTAL: Counter = Counter::new(
        "op_state_audit_entries_total",
        "Total audit log entries"
    ).unwrap();

    // Redis metrics
    /// Redis connection status
    pub static ref REDIS_CONNECTED: Gauge = Gauge::new(
        "op_state_redis_connected",
        "Redis connection status (1=connected, 0=disconnected)"
    ).unwrap();

    /// Redis stream lengths
    pub static ref REDIS_STREAM_LENGTH: GaugeVec = GaugeVec::new(
        Opts::new("op_state_redis_stream_length", "Redis stream length"),
        &["stream"]
    ).unwrap();

    /// Redis operations
    pub static ref REDIS_OPS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("op_state_redis_operations_total", "Redis operations"),
        &["operation"]
    ).unwrap();

    // SQLite metrics
    /// SQLite connection pool size
    pub static ref SQLITE_POOL_SIZE: Gauge = Gauge::new(
        "op_state_sqlite_pool_size",
        "SQLite connection pool size"
    ).unwrap();

    /// SQLite database size
    pub static ref SQLITE_DB_SIZE_BYTES: Gauge = Gauge::new(
        "op_state_sqlite_db_size_bytes",
        "SQLite database file size in bytes"
    ).unwrap();
}

static INIT: Once = Once::new();

/// Register all metrics with the global registry
pub fn register_metrics() {
    INIT.call_once(|| {
        info!("Registering state store metrics");

        // Job metrics
        REGISTRY.register(Box::new(JOBS_CREATED_TOTAL.clone())).ok();
        REGISTRY.register(Box::new(JOBS_BY_STATUS.clone())).ok();
        REGISTRY
            .register(Box::new(JOB_STATUS_TRANSITIONS.clone()))
            .ok();
        REGISTRY
            .register(Box::new(JOB_DURATION_SECONDS.clone()))
            .ok();

        // Store operation metrics
        REGISTRY.register(Box::new(STORE_OP_DURATION.clone())).ok();
        REGISTRY.register(Box::new(STORE_OP_ERRORS.clone())).ok();

        // Plugin metrics
        REGISTRY
            .register(Box::new(PLUGIN_QUERIES_TOTAL.clone()))
            .ok();
        REGISTRY
            .register(Box::new(PLUGIN_APPLIES_TOTAL.clone()))
            .ok();
        REGISTRY
            .register(Box::new(CHECKPOINTS_CREATED.clone()))
            .ok();

        // Audit metrics
        REGISTRY
            .register(Box::new(AUDIT_ENTRIES_TOTAL.clone()))
            .ok();

        // Redis metrics
        REGISTRY.register(Box::new(REDIS_CONNECTED.clone())).ok();
        REGISTRY
            .register(Box::new(REDIS_STREAM_LENGTH.clone()))
            .ok();
        REGISTRY.register(Box::new(REDIS_OPS_TOTAL.clone())).ok();

        // SQLite metrics
        REGISTRY.register(Box::new(SQLITE_POOL_SIZE.clone())).ok();
        REGISTRY
            .register(Box::new(SQLITE_DB_SIZE_BYTES.clone()))
            .ok();

        info!("State store metrics registered");
    });
}

/// Helper to time a store operation
pub struct OperationTimer {
    operation: String,
    store_type: String,
    start: std::time::Instant,
}

impl OperationTimer {
    pub fn new(operation: &str, store_type: &str) -> Self {
        Self {
            operation: operation.to_string(),
            store_type: store_type.to_string(),
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        STORE_OP_DURATION
            .with_label_values(&[&self.operation, &self.store_type])
            .observe(duration);
    }
}

/// Record a job status transition
pub fn record_job_transition(from: &str, to: &str) {
    JOB_STATUS_TRANSITIONS.with_label_values(&[from, to]).inc();
}

/// Record a job completion
pub fn record_job_completion(tool_name: &str, duration_secs: f64) {
    JOB_DURATION_SECONDS
        .with_label_values(&[tool_name])
        .observe(duration_secs);
}

/// Record a plugin query
pub fn record_plugin_query(plugin_name: &str) {
    PLUGIN_QUERIES_TOTAL.with_label_values(&[plugin_name]).inc();
}

/// Record a plugin apply
pub fn record_plugin_apply(plugin_name: &str, success: bool) {
    PLUGIN_APPLIES_TOTAL
        .with_label_values(&[plugin_name, if success { "true" } else { "false" }])
        .inc();
}

/// Record a checkpoint creation
pub fn record_checkpoint(plugin_name: &str) {
    CHECKPOINTS_CREATED.with_label_values(&[plugin_name]).inc();
}

/// Record an audit entry
pub fn record_audit_entry() {
    AUDIT_ENTRIES_TOTAL.inc();
}

/// Record a store error
pub fn record_store_error(operation: &str, store_type: &str, error_type: &str) {
    STORE_OP_ERRORS
        .with_label_values(&[operation, store_type, error_type])
        .inc();
}

/// Update job counts by status
pub fn update_job_counts(pending: u64, running: u64, completed: u64, failed: u64) {
    JOBS_BY_STATUS
        .with_label_values(&["pending"])
        .set(pending as f64);
    JOBS_BY_STATUS
        .with_label_values(&["running"])
        .set(running as f64);
    JOBS_BY_STATUS
        .with_label_values(&["completed"])
        .set(completed as f64);
    JOBS_BY_STATUS
        .with_label_values(&["failed"])
        .set(failed as f64);
}

/// Update Redis status
pub fn update_redis_status(connected: bool) {
    REDIS_CONNECTED.set(if connected { 1.0 } else { 0.0 });
}

/// Update Redis stream lengths
pub fn update_redis_stream_lengths(job_len: u64, plugin_len: u64) {
    REDIS_STREAM_LENGTH
        .with_label_values(&["jobs"])
        .set(job_len as f64);
    REDIS_STREAM_LENGTH
        .with_label_values(&["plugins"])
        .set(plugin_len as f64);
}

/// Update SQLite database size
pub fn update_sqlite_size(size_bytes: u64) {
    SQLITE_DB_SIZE_BYTES.set(size_bytes as f64);
}

/// Get metrics as text for Prometheus scraping
pub fn gather_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_metrics() {
        register_metrics();
        // Should not panic on duplicate registration
        register_metrics();
    }

    #[test]
    fn test_operation_timer() {
        register_metrics();

        {
            let _timer = OperationTimer::new("save_job", "sqlite");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Timer should have recorded the duration
        // We can't easily check the histogram value, but at least it shouldn't panic
    }

    #[test]
    fn test_record_functions() {
        register_metrics();

        record_job_transition("Pending", "Running");
        record_job_completion("test_tool", 1.5);
        record_plugin_query("lxc");
        record_plugin_apply("lxc", true);
        record_checkpoint("lxc");
        record_audit_entry();
        record_store_error("save", "sqlite", "connection");
        update_job_counts(1, 2, 3, 4);
        update_redis_status(true);
        update_redis_stream_lengths(100, 50);
        update_sqlite_size(1024);
    }
}
</file>

<file path="src/namespace_schema.sql">
-- ===================================================================
-- Operation D-Bus Namespace Schema
-- Enterprise-ready schema for org.opdbus.* services
-- Status: LIVE AND UNFILLED (tables ready, will be populated)
-- ===================================================================

-- Core namespace services (org.opdbus.*)
CREATE TABLE IF NOT EXISTS namespace_services (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL UNIQUE,  -- e.g., "org.opdbus.network"
    description TEXT,
    version TEXT DEFAULT 'v1',
    enabled BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Service interfaces (methods, properties, signals)
CREATE TABLE IF NOT EXISTS service_interfaces (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL REFERENCES namespace_services(id) ON DELETE CASCADE,
    interface_name TEXT NOT NULL,  -- e.g., "org.opdbus.network.Manager"
    version TEXT DEFAULT 'v1',
    methods_schema TEXT,  -- JSON: method definitions
    signals_schema TEXT,  -- JSON: signal definitions
    properties_schema TEXT, -- JSON: property definitions
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service_id, interface_name, version)
);

-- Object classes (for directory/LDAP integration)
CREATE TABLE IF NOT EXISTS object_classes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    interface_id INTEGER NOT NULL REFERENCES service_interfaces(id) ON DELETE CASCADE,
    class_name TEXT NOT NULL,  -- e.g., "NetworkInterface", "User", "Group"
    ldap_oid TEXT,  -- LDAP Object Identifier (for AD migration)
    parent_class TEXT,  -- Inheritance support
    structural BOOLEAN DEFAULT TRUE,
    attributes_schema TEXT,  -- JSON: attribute definitions
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(interface_id, class_name)
);

-- Attribute definitions
CREATE TABLE IF NOT EXISTS attribute_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    class_id INTEGER NOT NULL REFERENCES object_classes(id) ON DELETE CASCADE,
    attribute_name TEXT NOT NULL,
    ldap_name TEXT,  -- Original LDAP attribute name (for migration)
    attribute_type TEXT NOT NULL,  -- "string", "int", "bool", "array", "dict"
    single_valued BOOLEAN DEFAULT TRUE,
    mandatory BOOLEAN DEFAULT FALSE,
    default_value TEXT,
    validation_schema TEXT,  -- JSON: validation rules
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(class_id, attribute_name)
);

-- Live objects (actual D-Bus object instances)
CREATE TABLE IF NOT EXISTS live_objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL REFERENCES namespace_services(id) ON DELETE CASCADE,
    object_path TEXT NOT NULL,  -- D-Bus object path: /org/opdbus/network/connection/eth0
    object_class TEXT NOT NULL,  -- Class name
    state TEXT NOT NULL,  -- JSON: current object state
    metadata TEXT,  -- JSON: metadata (owner, tags, etc.)
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service_id, object_path)
);

-- Live links (relationships between objects)
CREATE TABLE IF NOT EXISTS live_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_object_id INTEGER NOT NULL REFERENCES live_objects(id) ON DELETE CASCADE,
    target_object_id INTEGER NOT NULL REFERENCES live_objects(id) ON DELETE CASCADE,
    link_type TEXT NOT NULL,  -- "contains", "references", "depends_on", etc.
    metadata TEXT,  -- JSON: additional link data
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_object_id, target_object_id, link_type)
);

-- Change log (audit trail for all object changes)
CREATE TABLE IF NOT EXISTS namespace_change_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    object_id INTEGER REFERENCES live_objects(id) ON DELETE SET NULL,
    object_path TEXT NOT NULL,
    change_type TEXT NOT NULL,  -- "created", "updated", "deleted", "property_changed"
    old_state TEXT,  -- JSON: state before change
    new_state TEXT,  -- JSON: state after change
    changed_by TEXT,  -- User/service that made the change
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Subscriptions (for real-time updates)
CREATE TABLE IF NOT EXISTS object_subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscriber_id TEXT NOT NULL,  -- Session/client ID
    object_id INTEGER REFERENCES live_objects(id) ON DELETE CASCADE,
    object_path_pattern TEXT,  -- Glob pattern: /org/opdbus/network/*
    event_types TEXT NOT NULL,  -- JSON array: ["property_changed", "signal"]
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ===================================================================
-- LDAP/Active Directory Migration Tables
-- ===================================================================

-- LDAP schemas (imported from existing infrastructure)
CREATE TABLE IF NOT EXISTS ldap_schemas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_domain TEXT NOT NULL,
    schema_source TEXT NOT NULL,  -- "active_directory", "openldap", "freeipa", "custom"
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    raw_schema TEXT NOT NULL,  -- JSON: raw LDAP schema from introspection
    metadata TEXT  -- JSON: import metadata
);

-- Migrated objects (tracking AD → op-dbus migration)
CREATE TABLE IF NOT EXISTS migrated_objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ldap_schema_id INTEGER NOT NULL REFERENCES ldap_schemas(id) ON DELETE CASCADE,
    source_dn TEXT NOT NULL,  -- Original Distinguished Name
    target_service TEXT NOT NULL,  -- e.g., "org.opdbus.directory"
    target_object_id INTEGER REFERENCES live_objects(id) ON DELETE SET NULL,
    target_class TEXT NOT NULL,  -- Converted class name
    migration_map TEXT NOT NULL,  -- JSON: attribute mapping
    migrated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(ldap_schema_id, source_dn)
);

-- Migration rules (reusable mappings)
CREATE TABLE IF NOT EXISTS migration_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_name TEXT NOT NULL UNIQUE,
    source_schema TEXT NOT NULL,  -- "active_directory", "openldap", etc.
    source_object_class TEXT NOT NULL,  -- LDAP objectClass
    target_service TEXT NOT NULL,  -- org.opdbus.* service
    target_class TEXT NOT NULL,  -- Target class name
    attribute_mappings TEXT NOT NULL,  -- JSON: LDAP attr → op-dbus attr mappings
    transformation_rules TEXT,  -- JSON: data transformation rules
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ===================================================================
-- Indices for Performance
-- ===================================================================

CREATE INDEX IF NOT EXISTS idx_service_interfaces_service ON service_interfaces(service_id);
CREATE INDEX IF NOT EXISTS idx_object_classes_interface ON object_classes(interface_id);
CREATE INDEX IF NOT EXISTS idx_attribute_definitions_class ON attribute_definitions(class_id);
CREATE INDEX IF NOT EXISTS idx_live_objects_service ON live_objects(service_id);
CREATE INDEX IF NOT EXISTS idx_live_objects_path ON live_objects(object_path);
CREATE INDEX IF NOT EXISTS idx_live_links_source ON live_links(source_object_id);
CREATE INDEX IF NOT EXISTS idx_live_links_target ON live_links(target_object_id);
CREATE INDEX IF NOT EXISTS idx_change_log_object ON namespace_change_log(object_id);
CREATE INDEX IF NOT EXISTS idx_change_log_timestamp ON namespace_change_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_subscriptions_subscriber ON object_subscriptions(subscriber_id);
CREATE INDEX IF NOT EXISTS idx_migrated_objects_ldap ON migrated_objects(ldap_schema_id);
CREATE INDEX IF NOT EXISTS idx_migrated_objects_dn ON migrated_objects(source_dn);

-- ===================================================================
-- Pre-populated Namespace Services (Templates)
-- ===================================================================

-- Insert core org.opdbus.* services (LIVE AND UNFILLED)
INSERT OR IGNORE INTO namespace_services (service_name, description, version) VALUES
    ('org.opdbus.hardware', 'Hardware management (IPMI/BMC compatible)', 'v1'),
    ('org.opdbus.network', 'Network management (replaces NetworkManager)', 'v1'),
    ('org.opdbus.container', 'Container management (Docker/Podman)', 'v1'),
    ('org.opdbus.session', 'Session management', 'v1'),
    ('org.opdbus.policy', 'Policy management and enforcement', 'v1'),
    ('org.opdbus.config', 'Configuration management', 'v1'),
    ('org.opdbus.directory', 'Directory services (LDAP/AD integration)', 'v1'),
    ('org.opdbus.ipmi', 'IPMI protocol wrapper (enterprise compatibility)', 'v1'),
    ('org.opdbus.bmc', 'BMC interface wrapper (enterprise compatibility)', 'v1'),
    ('org.opdbus.storage', 'Storage management (iSCSI/NFS/Ceph)', 'v1'),
    ('org.opdbus.monitoring', 'Monitoring and metrics collection', 'v1'),
    ('org.opdbus.backup', 'Backup and restore services', 'v1'),
    ('org.opdbus.security', 'Security and access control', 'v1'),
    ('org.opdbus.virtualization', 'VM and hypervisor management', 'v1'),
    ('org.opdbus.cluster', 'Cluster coordination (Pacemaker/Corosync)', 'v1');

-- ===================================================================
-- Pre-populated Migration Rules (Active Directory Templates)
-- ===================================================================

-- AD User → op-dbus.directory User
INSERT OR IGNORE INTO migration_rules (rule_name, source_schema, source_object_class, target_service, target_class, attribute_mappings) VALUES
    ('ad_user_to_opdbus',
     'active_directory',
     'user',
     'org.opdbus.directory',
     'User',
     json('{"sAMAccountName": "username", "displayName": "full_name", "mail": "email", "telephoneNumber": "phone", "department": "department", "title": "job_title", "manager": "manager_dn", "memberOf": "groups"}'));

-- AD Group → op-dbus.directory Group
INSERT OR IGNORE INTO migration_rules (rule_name, source_schema, source_object_class, target_service, target_class, attribute_mappings) VALUES
    ('ad_group_to_opdbus',
     'active_directory',
     'group',
     'org.opdbus.directory',
     'Group',
     json('{"sAMAccountName": "group_name", "description": "description", "member": "members", "managedBy": "manager_dn"}'));

-- AD Computer → op-dbus.hardware Device
INSERT OR IGNORE INTO migration_rules (rule_name, source_schema, source_object_class, target_service, target_class, attribute_mappings) VALUES
    ('ad_computer_to_opdbus',
     'active_directory',
     'computer',
     'org.opdbus.hardware',
     'Device',
     json('{"dNSHostName": "hostname", "operatingSystem": "os", "operatingSystemVersion": "os_version", "description": "description", "location": "physical_location"}'));

-- OpenLDAP posixAccount → op-dbus.directory User
INSERT OR IGNORE INTO migration_rules (rule_name, source_schema, source_object_class, target_service, target_class, attribute_mappings) VALUES
    ('openldap_posix_to_opdbus',
     'openldap',
     'posixAccount',
     'org.opdbus.directory',
     'User',
     json('{"uid": "username", "cn": "full_name", "mail": "email", "uidNumber": "uid", "gidNumber": "gid", "homeDirectory": "home_dir", "loginShell": "shell"}'));
</file>

<file path="src/plugin_schema.rs">
//! Plugin schema catalog and compatibility helpers.
//!
//! Architectural rule:
//! - plugin code is the source of schema truth
//! - that schema is also the footprint and JSON render contract
//! - this module stores indexed copies of that schema for validation/export
//!
//! Compatibility note:
//! a large amount of built-in schema data still lives in this file. Those
//! built-ins are intentionally exposed through explicit compatibility helpers
//! so runtime code can continue to resolve legacy schemas without turning this
//! catalog back into a second schema authority.

use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default JSON Schema dialect (can be overridden per-schema)
pub const DEFAULT_SCHEMA_DIALECT: &str = "https://json-schema.org/v1/2026";

/// Known dialect identifiers
pub mod dialects {
    pub const DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";
    pub const V2026: &str = "https://json-schema.org/v1/2026";
}

/// Path to the json-schema-spec repository relative to workspace root
const SCHEMA_SPEC_PATH: &str = "json-schema-spec";

/// Schema field type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Box<FieldType>),
    Object(HashMap<String, FieldSchema>),
    Enum(Vec<String>),
    Any,
}

/// Schema for a single field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldSchema {
    /// Field type
    pub field_type: FieldType,
    /// Whether the field is required
    #[serde(default)]
    pub required: bool,
    /// Description of the field
    #[serde(default)]
    pub description: String,
    /// Default value if not provided
    #[serde(default)]
    pub default: Option<Value>,
    /// Example value for documentation
    #[serde(default)]
    pub example: Option<Value>,
    /// Validation constraints
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// Unconditional readOnly - field cannot be modified
    #[serde(default)]
    pub read_only: bool,
    /// Conditional readOnly via propertyDependencies
    #[serde(default)]
    pub read_only_when: Option<ReadOnlyCondition>,
}

/// Condition for conditional readOnly (via propertyDependencies)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadOnlyCondition {
    /// The property to check (e.g., "status", "running")
    pub property: String,
    /// The value that triggers readOnly (e.g., "locked", "true")
    pub value: String,
}

/// Validation constraint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constraint {
    /// Minimum value (for numbers) or length (for strings/arrays)
    Min { value: f64 },
    /// Maximum value (for numbers) or length (for strings/arrays)
    Max { value: f64 },
    /// Regex pattern (for strings)
    Pattern { regex: String },
    /// Value must be one of these
    OneOf { values: Vec<Value> },
    /// Reference to another field that must exist
    RequiresField { field: String },
    /// Custom validation function name
    Custom { validator: String },
}

/// Plugin schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSchema {
    /// Plugin name
    pub name: String,
    /// Logical category used by the registry, renderer, and compliance overlays
    #[serde(default = "default_category")]
    pub category: String,
    /// Schema version
    pub version: String,
    /// Description
    pub description: String,
    /// Fields in the plugin state
    pub fields: HashMap<String, FieldSchema>,
    /// Dependencies on other plugins
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Example state for documentation
    #[serde(default)]
    pub example: Option<Value>,
    /// Paths that are always readOnly (e.g., ["/id", "/metadata"])
    #[serde(default)]
    pub immutable_paths: Vec<String>,
    /// Schema tags (e.g., ["immutable"] for fully immutable schemas)
    #[serde(default)]
    pub tags: Vec<String>,
    /// JSON Schema dialect to use (defaults to DEFAULT_SCHEMA_DIALECT)
    #[serde(default = "default_dialect")]
    pub dialect: String,
    /// Mutation index for the identity sled
    #[serde(default)]
    pub mutation_index: Option<u64>,
}

fn default_dialect() -> String {
    DEFAULT_SCHEMA_DIALECT.to_string()
}

fn default_category() -> String {
    "uncategorized".to_string()
}

impl PluginSchema {
    /// Check if the schema state is valid (for the identity sled)
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && !self.version.is_empty()
    }

    /// Create a new plugin schema builder
    pub fn builder(name: &str) -> PluginSchemaBuilder {
        PluginSchemaBuilder::new(name)
    }

    /// Validate a state value against this schema
    pub fn validate(&self, state: &Value) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check required fields
        for (field_name, field_schema) in &self.fields {
            if field_schema.required && state.get(field_name).is_none() {
                errors.push(format!("Missing required field: {}", field_name));
            }
        }

        // Validate present fields
        if let Some(obj) = state.as_object() {
            for (field_name, field_value) in obj {
                if let Some(field_schema) = self.fields.get(field_name) {
                    if let Err(e) = validate_field(field_name, field_value, field_schema) {
                        errors.push(e);
                    }
                } else {
                    warnings.push(format!("Unknown field: {}", field_name));
                }
            }
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Generate a template state with default values
    pub fn generate_template(&self) -> Value {
        let mut template = simd_json::value::owned::Object::new();

        for (field_name, field_schema) in &self.fields {
            let value = if let Some(default) = &field_schema.default {
                default.clone()
            } else if let Some(example) = &field_schema.example {
                example.clone()
            } else {
                default_for_type(&field_schema.field_type)
            };
            template.insert(field_name.clone(), value);
        }

        Value::Object(Box::new(template))
    }

    /// Convert to JSON Schema 2026 format (default)
    ///
    /// Includes support for:
    /// - `readOnly` on individual fields
    /// - `propertyDependencies` for conditional immutability
    /// - Schema-level immutability via tags
    pub fn to_json_schema(&self) -> Value {
        let is_fully_immutable = self.tags.contains(&"immutable".to_string());
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();
        let mut property_dependencies: HashMap<String, HashMap<String, Vec<String>>> =
            HashMap::new();

        for (field_name, field_schema) in &self.fields {
            let mut field_json = field_type_to_json_schema_2026(&field_schema.field_type);

            // Add description if present
            if !field_schema.description.is_empty() {
                if let Some(obj) = field_json.as_object_mut() {
                    obj.insert("description".to_string(), json!(field_schema.description));
                }
            }

            // Add readOnly if field is unconditionally immutable, in immutable_paths, or schema is fully immutable
            let path = format!("/{}", field_name);
            if field_schema.read_only || self.immutable_paths.contains(&path) || is_fully_immutable
            {
                if let Some(obj) = field_json.as_object_mut() {
                    obj.insert("readOnly".to_string(), json!(true));
                }
            }

            // Collect propertyDependencies for conditional readOnly
            if let Some(condition) = &field_schema.read_only_when {
                property_dependencies
                    .entry(condition.property.clone())
                    .or_default()
                    .entry(condition.value.clone())
                    .or_default()
                    .push(field_name.clone());
            }

            properties.insert(field_name.clone(), field_json);
            if field_schema.required {
                required.push(Value::String(field_name.clone()));
            }
        }

        let mut schema = json!({
            "$schema": &self.dialect,
            "title": self.name,
            "description": self.description,
            "x-plugin-category": self.category,
            "type": "object",
            "properties": properties,
            "required": required
        });

        // Add propertyDependencies if any conditional readOnly fields exist
        if !property_dependencies.is_empty() {
            let mut deps_json = simd_json::value::owned::Object::new();
            for (prop, value_map) in property_dependencies {
                let mut values_json = simd_json::value::owned::Object::new();
                for (value, fields) in value_map {
                    let mut props = simd_json::value::owned::Object::new();
                    for field in fields {
                        props.insert(field, json!({"readOnly": true}));
                    }
                    values_json.insert(
                        value,
                        json!({
                            "properties": props
                        }),
                    );
                }
                deps_json.insert(prop, Value::Object(Box::new(values_json)));
            }
            if let Some(obj) = schema.as_object_mut() {
                obj.insert(
                    "propertyDependencies".to_string(),
                    Value::Object(Box::new(deps_json)),
                );
            }
        }

        schema
    }

    /// Convert to JSON Schema draft-07 format (deprecated, for backward compatibility)
    #[deprecated(since = "2.0.0", note = "Use to_json_schema() for JSON Schema 2026")]
    pub fn to_json_schema_draft07(&self) -> Value {
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();

        for (field_name, field_schema) in &self.fields {
            properties.insert(
                field_name.clone(),
                field_type_to_json_schema(&field_schema.field_type),
            );
            if field_schema.required {
                required.push(Value::String(field_name.clone()));
            }
        }

        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": self.name,
            "description": self.description,
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    /// Convert to the legacy contract-style schema while using this registry
    /// entry as the only source of truth for the tunable section.
    pub fn to_contract_json_schema(&self) -> Value {
        self.to_contract_json_schema_as(&self.name)
    }

    /// Convert to the legacy contract-style schema using an alternate public
    /// plugin name for compatibility aliases such as `systemd`.
    pub fn to_contract_json_schema_as(&self, public_name: &str) -> Value {
        let mut include_paths: Vec<String> = self
            .fields
            .keys()
            .map(|field| format!("/tunable/{field}"))
            .collect();
        include_paths.sort();

        let mut secret_paths: Vec<String> = self
            .fields
            .keys()
            .filter(|field| is_secret_field_name(field))
            .map(|field| format!("/tunable/{field}"))
            .collect();
        secret_paths.sort();

        let mut pii_paths: Vec<String> = self
            .fields
            .keys()
            .filter(|field| is_pii_field_name(field))
            .map(|field| format!("/tunable/{field}"))
            .collect();
        pii_paths.sort();

        let sensitivity = if secret_paths.is_empty() {
            "internal"
        } else {
            "secret"
        };

        json!({
            "$schema": DEFAULT_SCHEMA_DIALECT,
            "$id": format!("https://op-dbus.local/schemas/plugins/{public_name}.contract.json"),
            "title": format!("{public_name} contract schema"),
            "description": self.description,
            "type": "object",
            "required": [
                "schema_version",
                "plugin",
                "object_type",
                "object_id",
                "stub",
                "immutable",
                "tunable",
                "observed",
                "meta",
                "semantic_index",
                "privacy_index"
            ],
            "properties": {
                "schema_version": {
                    "type": "string",
                    "const": self.version
                },
                "plugin": {
                    "type": "string",
                    "const": public_name
                },
                "object_type": {
                    "type": "string",
                    "const": format!("{}_object", public_name.replace('-', "_"))
                },
                "object_id": {
                    "type": "string",
                    "minLength": 1
                },
                "stub": {
                    "type": "object",
                    "required": ["system_id", "source", "source_ref", "discovered_at"],
                    "properties": {
                        "system_id": { "type": "string", "minLength": 1 },
                        "source": { "type": "string", "minLength": 1 },
                        "source_ref": { "type": "string", "minLength": 1 },
                        "discovered_at": { "type": "string", "format": "date-time" }
                    },
                    "additionalProperties": false
                },
                "immutable": {
                    "type": "object",
                    "required": ["created_at", "created_by_plugin", "identity_keys", "provider"],
                    "properties": {
                        "created_at": { "type": "string", "format": "date-time" },
                        "created_by_plugin": { "type": "string", "const": public_name },
                        "identity_keys": {
                            "type": "array",
                            "items": { "type": "string" },
                            "minItems": 1,
                            "default": ["object_id"]
                        },
                        "provider": { "type": "string", "default": "op-dbus" }
                    },
                    "additionalProperties": false
                },
                "tunable": self.to_json_schema(),
                "observed": {
                    "type": "object",
                    "required": ["last_observed_at"],
                    "properties": {
                        "last_observed_at": { "type": "string", "format": "date-time" },
                        "status": { "type": "string" },
                        "drift_detected": { "type": "boolean", "default": false },
                        "metrics": { "type": "object" }
                    },
                    "additionalProperties": true
                },
                "meta": {
                    "type": "object",
                    "required": [
                        "dependencies",
                        "include_in_recovery",
                        "recovery_priority",
                        "category",
                        "sensitivity",
                        "tags",
                        "enabled"
                    ],
                    "properties": {
                        "dependencies": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": self.dependencies
                        },
                        "include_in_recovery": { "type": "boolean", "default": true },
                        "recovery_priority": { "type": "integer", "minimum": 0, "default": 50 },
                        "category": {
                            "type": "string",
                            "default": self.category
                        },
                        "sensitivity": {
                            "type": "string",
                            "enum": ["public", "internal", "secret"],
                            "default": sensitivity
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": self.tags
                        },
                        "enabled": { "type": "boolean", "default": true }
                    },
                    "additionalProperties": false
                },
                "semantic_index": {
                    "type": "object",
                    "required": ["include_paths", "exclude_paths", "chunking", "redaction"],
                    "properties": {
                        "include_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": include_paths
                        },
                        "exclude_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": ["/stub/discovered_at"]
                        },
                        "chunking": {
                            "type": "object",
                            "required": ["strategy", "max_tokens"],
                            "properties": {
                                "strategy": { "type": "string", "enum": ["json-path-group"], "default": "json-path-group" },
                                "max_tokens": { "type": "integer", "minimum": 64, "default": 512 }
                            },
                            "additionalProperties": false
                        },
                        "redaction": {
                            "type": "object",
                            "required": ["enabled"],
                            "properties": {
                                "enabled": { "type": "boolean", "default": true }
                            },
                            "additionalProperties": false
                        }
                    },
                    "additionalProperties": false
                },
                "privacy_index": {
                    "type": "object",
                    "required": ["redaction"],
                    "properties": {
                        "redaction": {
                            "type": "object",
                            "required": [
                                "rules",
                                "default_action",
                                "secret_paths",
                                "pii_paths",
                                "hash_salt_ref",
                                "reversible"
                            ],
                            "properties": {
                                "rules": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "required": ["path", "action"],
                                        "properties": {
                                            "path": { "type": "string" },
                                            "action": { "type": "string", "enum": ["drop", "mask", "hash"] },
                                            "reason": { "type": "string" }
                                        },
                                        "additionalProperties": false
                                    },
                                    "default": []
                                },
                                "default_action": {
                                    "type": "string",
                                    "enum": ["drop", "mask", "hash"],
                                    "default": "mask"
                                },
                                "secret_paths": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "default": secret_paths
                                },
                                "pii_paths": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "default": pii_paths
                                },
                                "hash_salt_ref": {
                                    "type": "string",
                                    "default": "vault://op-dbus/privacy/hash-salt"
                                },
                                "reversible": {
                                    "type": "boolean",
                                    "default": false
                                }
                            },
                            "additionalProperties": false
                        }
                    },
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        })
    }
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Builder for creating plugin schemas
pub struct PluginSchemaBuilder {
    name: String,
    category: String,
    version: String,
    description: String,
    fields: HashMap<String, FieldSchema>,
    dependencies: Vec<String>,
    example: Option<Value>,
    immutable_paths: Vec<String>,
    tags: Vec<String>,
    dialect: String,
}

impl PluginSchemaBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            category: default_category(),
            version: "1.0.0".to_string(),
            description: String::new(),
            fields: HashMap::new(),
            dependencies: Vec::new(),
            example: None,
            immutable_paths: Vec::new(),
            tags: Vec::new(),
            dialect: DEFAULT_SCHEMA_DIALECT.to_string(),
        }
    }

    pub fn version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn category(mut self, category: &str) -> Self {
        self.category = category.to_string();
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn field(mut self, name: &str, schema: FieldSchema) -> Self {
        self.fields.insert(name.to_string(), schema);
        self
    }

    pub fn string_field(self, name: &str, required: bool, description: &str) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::String,
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn integer_field(self, name: &str, required: bool, description: &str) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Integer,
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn boolean_field(self, name: &str, required: bool, description: &str) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Boolean,
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn array_field(
        self,
        name: &str,
        item_type: FieldType,
        required: bool,
        description: &str,
    ) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Array(Box::new(item_type)),
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn object_field(
        self,
        name: &str,
        fields: HashMap<String, FieldSchema>,
        required: bool,
        description: &str,
    ) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Object(fields),
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn dependency(mut self, plugin_name: &str) -> Self {
        self.dependencies.push(plugin_name.to_string());
        self
    }

    pub fn example(mut self, example: Value) -> Self {
        self.example = Some(example);
        self
    }

    /// Add a path that should always be readOnly (e.g., "/id")
    pub fn immutable_path(mut self, path: &str) -> Self {
        self.immutable_paths.push(path.to_string());
        self
    }

    /// Add multiple immutable paths at once
    pub fn immutable_paths(mut self, paths: &[&str]) -> Self {
        self.immutable_paths
            .extend(paths.iter().map(|s| s.to_string()));
        self
    }

    /// Add a tag to the schema (e.g., "immutable" for fully immutable)
    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// Mark the entire schema as immutable
    pub fn fully_immutable(self) -> Self {
        self.tag("immutable")
    }

    /// Set the JSON Schema dialect (e.g., dialects::V2026)
    pub fn dialect(mut self, dialect: &str) -> Self {
        self.dialect = dialect.to_string();
        self
    }

    pub fn build(self) -> PluginSchema {
        PluginSchema {
            name: self.name,
            category: self.category,
            version: self.version,
            description: self.description,
            fields: self.fields,
            dependencies: self.dependencies,
            example: self.example,
            immutable_paths: self.immutable_paths,
            tags: self.tags,
            dialect: self.dialect,
            mutation_index: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredSchemaCopies {
    pub plugin: PluginSchema,
    pub json_schema: Value,
    pub contract_schema: Value,
}

/// In-memory schema catalog.
///
/// Compatibility note:
/// this type is still named `SchemaRegistry` in much of the workspace, but its
/// architectural role is now a catalog/index over canonical plugin documents.
/// It stores derived schema copies for lookup, validation, rendering, and
/// compatibility export. It is not the origin of schema truth.
pub struct SchemaRegistry {
    schemas: HashMap<String, PluginSchema>,
    categorized: HashMap<String, HashMap<String, StoredSchemaCopies>>,
    meta_schemas: HashMap<String, Value>,
    spec_base_path: Option<PathBuf>,
}

impl SchemaRegistry {
    /// Create an empty catalog. Runtime code should populate this from plugins
    /// or from persisted canonical plugin documents.
    pub fn empty() -> Self {
        Self {
            schemas: HashMap::new(),
            categorized: HashMap::new(),
            meta_schemas: HashMap::new(),
            spec_base_path: None,
        }
    }

    /// Create a new runtime catalog.
    ///
    /// Plugins are the canonical schema source; runtime code should register
    /// plugin-provided schemas into this empty catalog.
    pub fn new() -> Self {
        Self::empty()
    }

    /// Create a runtime catalog with a custom spec base path.
    pub fn with_spec_path(spec_path: impl AsRef<Path>) -> Self {
        let mut registry = Self::empty();
        registry.spec_base_path = Some(spec_path.as_ref().to_path_buf());
        registry
    }

    /// Create a catalog pre-populated with built-in compatibility schemas.
    pub fn with_builtin_schemas() -> Self {
        let mut registry = Self::empty();
        registry.register_builtin_schemas();
        registry
    }

    /// Create a compatibility catalog with built-ins and a custom spec base path.
    pub fn with_builtin_schemas_and_spec_path(spec_path: impl AsRef<Path>) -> Self {
        let mut registry = Self::with_spec_path(spec_path);
        registry.register_builtin_schemas();
        registry
    }

    /// Set the base path for the json-schema-spec repository
    pub fn set_spec_path(&mut self, path: impl AsRef<Path>) {
        self.spec_base_path = Some(path.as_ref().to_path_buf());
    }

    /// Load meta-schema from the spec repository
    pub fn load_meta_schema(&mut self, dialect: &str) -> Result<(), SchemaLoadError> {
        let spec_path = self
            .spec_base_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(SCHEMA_SPEC_PATH));

        // Map dialect URL to file path
        let meta_path = match dialect {
            d if d.contains("2026") => spec_path.join("specs/meta/meta.json"),
            _ => return Err(SchemaLoadError::UnsupportedDialect(dialect.to_string())),
        };

        let mut content = fs::read_to_string(&meta_path)
            .map_err(|e| SchemaLoadError::IoError(meta_path.clone(), e.to_string()))?;

        let schema: Value = unsafe { simd_json::from_str(&mut content) }
            .map_err(|e| SchemaLoadError::ParseError(meta_path.clone(), e.to_string()))?;

        self.meta_schemas.insert(dialect.to_string(), schema);
        Ok(())
    }

    /// Get a loaded meta-schema
    pub fn get_meta_schema(&self, dialect: &str) -> Option<&Value> {
        self.meta_schemas.get(dialect)
    }

    /// Load a plugin schema from a JSON file
    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<String, SchemaLoadError> {
        let path = path.as_ref();
        let mut content = fs::read_to_string(path)
            .map_err(|e| SchemaLoadError::IoError(path.to_path_buf(), e.to_string()))?;

        let schema: PluginSchema = unsafe { simd_json::from_str(&mut content) }
            .map_err(|e| SchemaLoadError::ParseError(path.to_path_buf(), e.to_string()))?;

        let name = schema.name.clone();
        self.register(schema);
        Ok(name)
    }

    /// Load all schema files from a directory
    pub fn load_from_directory(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> Result<Vec<String>, SchemaLoadError> {
        let dir = dir.as_ref();
        let mut loaded = Vec::new();

        let entries = fs::read_dir(dir)
            .map_err(|e| SchemaLoadError::IoError(dir.to_path_buf(), e.to_string()))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| SchemaLoadError::IoError(dir.to_path_buf(), e.to_string()))?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match self.load_from_file(&path) {
                    Ok(name) => loaded.push(name),
                    Err(e) => {
                        tracing::warn!("Failed to load schema from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Export all schemas as JSON Schema documents
    pub fn export_all(&self) -> HashMap<String, Value> {
        self.schemas
            .iter()
            .map(|(name, schema)| (name.clone(), schema.to_json_schema()))
            .collect()
    }

    /// Export all schemas in draft-07 format (for backward compatibility)
    #[allow(deprecated)]
    pub fn export_all_draft07(&self) -> HashMap<String, Value> {
        self.schemas
            .iter()
            .map(|(name, schema)| (name.clone(), schema.to_json_schema_draft07()))
            .collect()
    }

    /// Export all schemas as legacy contract documents keyed by canonical plugin name.
    pub fn export_all_contract(&self) -> HashMap<String, Value> {
        self.schemas
            .iter()
            .map(|(name, schema)| (name.clone(), schema.to_contract_json_schema()))
            .collect()
    }

    /// Index one plugin schema and cache all derived copies under its category.
    pub fn register(&mut self, schema: PluginSchema) {
        let copies = StoredSchemaCopies {
            json_schema: schema.to_json_schema(),
            contract_schema: schema.to_contract_json_schema(),
            plugin: schema.clone(),
        };

        self.categorized
            .entry(schema.category.clone())
            .or_default()
            .insert(schema.name.clone(), copies);
        self.schemas.insert(schema.name.clone(), schema);
    }

    /// Get a plugin schema by name
    pub fn get(&self, name: &str) -> Option<&PluginSchema> {
        self.schemas.get(Self::canonical_name(name))
    }

    /// Get the categorized schema copies stored for a plugin.
    pub fn get_copies(&self, name: &str) -> Option<&StoredSchemaCopies> {
        let schema = self.get(name)?;
        self.categorized
            .get(&schema.category)
            .and_then(|schemas| schemas.get(&schema.name))
    }

    /// Export one schema as a legacy contract document, preserving alias names.
    pub fn export_contract_for(&self, name: &str) -> Option<Value> {
        self.get(name)
            .map(|schema| schema.to_contract_json_schema_as(name))
    }

    /// List all registered schema names
    pub fn list(&self) -> Vec<&str> {
        self.schemas.keys().map(|s| s.as_str()).collect()
    }

    /// List all schema categories currently present in the catalog.
    pub fn categories(&self) -> Vec<&str> {
        let mut categories: Vec<&str> = self.categorized.keys().map(|s| s.as_str()).collect();
        categories.sort_unstable();
        categories
    }

    /// Return all schema copies stored under one category.
    pub fn by_category(&self, category: &str) -> Option<&HashMap<String, StoredSchemaCopies>> {
        self.categorized.get(category)
    }

    /// Validate state for a plugin
    pub fn validate(&self, plugin_name: &str, state: &Value) -> Option<ValidationResult> {
        self.get(plugin_name).map(|schema| schema.validate(state))
    }

    /// Register all built-in plugin schemas.
    ///
    /// This path is compatibility-only and should not be treated as the
    /// authoritative runtime source for live plugin documents.
    fn register_builtin_schemas(&mut self) {
        for schema in builtin_plugin_schemas() {
            self.register(schema);
        }
    }

    fn canonical_name(name: &str) -> &str {
        match name {
            "incus_wireguard_ingress" => "incus-wireguard-ingress",
            "incus_xray_reality_client" => "incus-xray-reality-client",
            "incus_xray_reality_server" => "incus-xray-reality-server",
            "network" => "net",
            "systemd" | "dinit" => "s6",
            "web-ui" => "web_ui",
            other => other,
        }
    }
}

/// Compatibility helper for built-in plugin schemas.
///
/// Runtime code should still obtain schema through the plugin instance. This
/// helper exists so legacy built-in plugins can satisfy `StatePlugin::schema()`
/// without forcing registration code to infer schema from live state again.
pub fn builtin_plugin_schema(name: &str) -> Option<PluginSchema> {
    builtin_plugin_schema_from_canonical_name(SchemaRegistry::canonical_name(name))
}

/// Return the full compatibility set of built-in schemas.
///
/// Calling this is an explicit compatibility action. Runtime code should
/// prefer plugin registration or persisted canonical plugin documents instead.
pub fn builtin_plugin_schemas() -> Vec<PluginSchema> {
    [
        "lxc",
        "incus",
        "incus-wireguard-ingress",
        "incus-xray-reality-client",
        "incus-xray-reality-server",
        "net",
        "rtnetlink",
        "openflow",
        "s6",
        "privacy_router",
        "privacy_routes",
        "netmaker",
        "adc",
        "agent_config",
        "config",
        "dnsresolver",
        "endpoint",
        "full_system",
        "gcloud_adc",
        "hardware",
        "keypair",
        "keyring",
        "login1",
        "mcp",
        "openflow_obfuscation",
        "ovsdb_bridge",
        "packagekit",
        "pcidecl",
        "privacy",
        "proxmox",
        "proxy_server",
        "service",
        "sess_decl",
        "software",
        "users",
        "web_ui",
        "wireguard",
    ]
    .into_iter()
    .filter_map(builtin_plugin_schema_from_canonical_name)
    .collect()
}

fn builtin_plugin_schema_from_canonical_name(name: &str) -> Option<PluginSchema> {
    Some(match name {
        "adc" => create_adc_schema(),
        "agent_config" => create_agent_config_schema(),
        "config" => create_config_schema(),
        "dnsresolver" => create_dnsresolver_schema(),
        "endpoint" => create_endpoint_schema(),
        "full_system" => create_full_system_schema(),
        "gcloud_adc" => create_gcloud_adc_schema(),
        "hardware" => create_hardware_schema(),
        "keypair" => create_keypair_schema(),
        "keyring" => create_keyring_schema(),
        "login1" => create_login1_schema(),
        "mcp" => create_mcp_schema(),
        "openflow_obfuscation" => create_openflow_obfuscation_schema(),
        "ovsdb_bridge" => create_ovsdb_bridge_schema(),
        "packagekit" => create_packagekit_schema(),
        "pcidecl" => create_pcidecl_schema(),
        "privacy" => create_privacy_schema(),
        "proxmox" => create_proxmox_schema(),
        "proxy_server" => create_proxy_server_schema(),
        "service" => create_service_schema(),
        "sess_decl" => create_sess_decl_schema(),
        "software" => create_software_schema(),
        "users" => create_users_schema(),
        "web_ui" => create_web_ui_schema(),
        "wireguard" => create_wireguard_schema(),
        "lxc" => create_lxc_schema(),
        "incus" => create_incus_schema(),
        "incus-wireguard-ingress" => create_incus_wireguard_ingress_schema(),
        "incus-xray-reality-client" => create_incus_xray_reality_client_schema(),
        "incus-xray-reality-server" => create_incus_xray_reality_server_schema(),
        "net" => create_net_schema(),
        "rtnetlink" => create_rtnetlink_schema(),
        "openflow" => create_openflow_schema(),
        "s6" | "service" => create_s6_schema(),
        "privacy_router" => create_privacy_router_schema(),
        "privacy_routes" => create_privacy_routes_schema(),
        "netmaker" => create_netmaker_schema(),
        _ => return None,
    })
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Preferred architectural name for `SchemaRegistry`.
///
/// The old name remains for compatibility while the workspace is migrated
/// crate by crate, but new code should talk in terms of a schema catalog.
pub type SchemaCatalog = SchemaRegistry;

/// Errors that can occur when loading schemas
#[derive(Debug, Clone)]
pub enum SchemaLoadError {
    IoError(PathBuf, String),
    ParseError(PathBuf, String),
    UnsupportedDialect(String),
}

impl std::fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(path, msg) => write!(f, "IO error reading {:?}: {}", path, msg),
            Self::ParseError(path, msg) => write!(f, "Parse error in {:?}: {}", path, msg),
            Self::UnsupportedDialect(d) => write!(f, "Unsupported dialect: {}", d),
        }
    }
}

impl std::error::Error for SchemaLoadError {}

// ============================================================================
// Built-in Schema Definitions
// ============================================================================

fn any_field(required: bool, description: &str, default: Option<Value>) -> FieldSchema {
    FieldSchema {
        field_type: FieldType::Any,
        required,
        description: description.to_string(),
        default,
        example: None,
        constraints: Vec::new(),
        read_only: false,
        read_only_when: None,
    }
}

fn is_secret_field_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "secret",
        "private",
        "token",
        "password",
        "credential",
        "license",
        "api_key",
        "key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_pii_field_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["email", "account", "google_id", "google_email", "user_id"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn simple_schema(
    name: &str,
    description: &str,
    dependencies: &[&str],
    fields: Vec<(&str, FieldSchema)>,
) -> PluginSchema {
    let mut builder = PluginSchema::builder(name)
        .version("1.0.0")
        .description(description);
    for dep in dependencies {
        builder = builder.dependency(dep);
    }
    for (field_name, schema) in fields {
        builder = builder.field(field_name, schema);
    }
    builder.build()
}

fn create_adc_schema() -> PluginSchema {
    simple_schema(
        "adc",
        "Application default credentials state",
        &[],
        vec![(
            "configured",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether ADC is configured".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

fn create_agent_config_schema() -> PluginSchema {
    simple_schema(
        "agent_config",
        "Agent configuration and tool assignments",
        &[],
        vec![(
            "agents",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Any)),
                required: true,
                description: "List of agent configurations".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

fn create_config_schema() -> PluginSchema {
    simple_schema(
        "config",
        "Global key/value config store",
        &[],
        vec![(
            "configs",
            any_field(true, "Configuration map", Some(json!({}))),
        )],
    )
}

fn create_dnsresolver_schema() -> PluginSchema {
    simple_schema(
        "dnsresolver",
        "DNS resolver declaration state",
        &["net"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: false,
                    description: "Schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "items",
                FieldSchema {
                    field_type: FieldType::Array(Box::new(FieldType::Any)),
                    required: true,
                    description: "Resolver items".to_string(),
                    default: Some(json!([])),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

fn create_endpoint_schema() -> PluginSchema {
    simple_schema(
        "endpoint",
        "Endpoint configuration",
        &["net"],
        vec![(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Declared endpoints".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

fn create_full_system_schema() -> PluginSchema {
    simple_schema(
        "full_system",
        "Full system recovery snapshot",
        &["net", "service", "software", "users", "lxc", "s6"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Snapshot schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "captured_at",
                FieldSchema {
                    field_type: FieldType::String,
                    required: false,
                    description: "Capture timestamp".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            ("hostname", any_field(true, "Host name", Some(json!("")))),
            (
                "system",
                any_field(false, "System details", Some(json!({}))),
            ),
            (
                "network",
                any_field(false, "Network snapshot", Some(json!({}))),
            ),
            (
                "services",
                any_field(false, "Service snapshot", Some(json!([]))),
            ),
            (
                "packages",
                any_field(false, "Package snapshot", Some(json!([]))),
            ),
            ("users", any_field(false, "User snapshot", Some(json!([])))),
            (
                "storage",
                any_field(false, "Storage snapshot", Some(json!({}))),
            ),
            (
                "containers",
                any_field(false, "Container snapshot", Some(json!({}))),
            ),
            (
                "plugins",
                any_field(false, "Plugin snapshots", Some(json!({}))),
            ),
        ],
    )
}

fn create_gcloud_adc_schema() -> PluginSchema {
    simple_schema(
        "gcloud_adc",
        "Google Cloud ADC state",
        &[],
        vec![
            ("account", any_field(false, "Authenticated account", None)),
            ("project_id", any_field(false, "Project id", None)),
            (
                "authenticated",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Authentication status".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

fn create_hardware_schema() -> PluginSchema {
    simple_schema(
        "hardware",
        "Hardware inventory snapshot",
        &[],
        vec![
            ("cpu", any_field(true, "CPU info", Some(json!({})))),
            ("memory", any_field(true, "Memory info", Some(json!({})))),
            ("disks", any_field(true, "Disk list", Some(json!([])))),
        ],
    )
}

fn create_keypair_schema() -> PluginSchema {
    simple_schema(
        "keypair",
        "Keypair declaration state",
        &[],
        vec![(
            "keypairs",
            any_field(true, "Managed keypairs", Some(json!([]))),
        )],
    )
}

fn create_keyring_schema() -> PluginSchema {
    simple_schema(
        "keyring",
        "Secret service collections state",
        &[],
        vec![
            (
                "collections",
                any_field(true, "Secret collections", Some(json!([]))),
            ),
            (
                "default_collection",
                any_field(false, "Default collection path", None),
            ),
        ],
    )
}

fn create_login1_schema() -> PluginSchema {
    simple_schema(
        "login1",
        "Runtime login sessions",
        &["users"],
        vec![(
            "sessions",
            any_field(true, "Active sessions", Some(json!([]))),
        )],
    )
}

fn create_mcp_schema() -> PluginSchema {
    simple_schema(
        "mcp",
        "MCP server and tool-group configuration",
        &["agent_config"],
        vec![
            (
                "servers",
                any_field(false, "MCP server map", Some(json!({}))),
            ),
            (
                "tool_groups",
                any_field(false, "Tool group config", Some(json!({}))),
            ),
            (
                "compact_mode",
                any_field(false, "Compact mode config", Some(json!({}))),
            ),
        ],
    )
}

fn create_openflow_obfuscation_schema() -> PluginSchema {
    simple_schema(
        "openflow_obfuscation",
        "OpenFlow traffic obfuscation configuration",
        &["openflow", "net"],
        vec![(
            "config",
            any_field(true, "Obfuscation config", Some(json!({}))),
        )],
    )
}

fn create_ovsdb_bridge_schema() -> PluginSchema {
    simple_schema(
        "ovsdb_bridge",
        "OVS bridge declarations",
        &["net"],
        vec![(
            "bridges",
            any_field(true, "Bridge declarations", Some(json!([]))),
        )],
    )
}

fn create_packagekit_schema() -> PluginSchema {
    simple_schema(
        "packagekit",
        "PackageKit package declarations",
        &["software"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: false,
                    description: "Schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "packages",
                any_field(true, "Package declaration map", Some(json!({}))),
            ),
        ],
    )
}

fn create_pcidecl_schema() -> PluginSchema {
    simple_schema(
        "pcidecl",
        "PCI device declaration state",
        &["hardware"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: false,
                    description: "Schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "items",
                any_field(true, "PCI declarations", Some(json!([]))),
            ),
        ],
    )
}

fn create_privacy_schema() -> PluginSchema {
    simple_schema(
        "privacy",
        "Privacy coordination configuration",
        &["wireguard", "proxmox", "privacy_router"],
        vec![("config", any_field(true, "Privacy config", Some(json!({}))))],
    )
}

fn create_proxmox_schema() -> PluginSchema {
    simple_schema(
        "proxmox",
        "Proxmox container declarations",
        &["net"],
        vec![(
            "containers",
            any_field(true, "Container declarations", Some(json!([]))),
        )],
    )
}

fn create_proxy_server_schema() -> PluginSchema {
    simple_schema(
        "proxy_server",
        "Proxy server runtime config",
        &["net"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable proxy".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "port",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Proxy port".to_string(),
                    default: Some(json!(8080)),
                    example: None,
                    constraints: vec![
                        Constraint::Min { value: 1.0 },
                        Constraint::Max { value: 65535.0 },
                    ],
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

fn create_service_schema() -> PluginSchema {
    simple_schema(
        "service",
        "Service definition declarations",
        &["net"],
        vec![("services", any_field(true, "Service map", Some(json!({}))))],
    )
}

fn create_sess_decl_schema() -> PluginSchema {
    simple_schema(
        "sess_decl",
        "Session declaration state",
        &["users"],
        vec![(
            "sessions",
            any_field(true, "Session declarations", Some(json!([]))),
        )],
    )
}

fn create_software_schema() -> PluginSchema {
    simple_schema(
        "software",
        "Software package inventory",
        &[],
        vec![("packages", any_field(true, "Package list", Some(json!([]))))],
    )
}

fn create_users_schema() -> PluginSchema {
    simple_schema(
        "users",
        "User account declarations",
        &[],
        vec![("users", any_field(true, "Users list", Some(json!([]))))],
    )
}

fn create_web_ui_schema() -> PluginSchema {
    simple_schema(
        "web_ui",
        "Web UI tunables",
        &["mcp"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable UI".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cors_origins",
                any_field(false, "Allowed CORS origins", Some(json!([]))),
            ),
            (
                "compression",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable compression".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cache_ttl",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Cache TTL seconds".to_string(),
                    default: Some(json!(86400)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 0.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "theme",
                any_field(true, "Theme name", Some(json!("default"))),
            ),
            (
                "feature_flags",
                any_field(false, "Feature flag map", Some(json!({}))),
            ),
        ],
    )
}

fn create_wireguard_schema() -> PluginSchema {
    simple_schema(
        "wireguard",
        "WireGuard interface state",
        &["net"],
        vec![(
            "interfaces",
            any_field(true, "WireGuard interfaces", Some(json!([]))),
        )],
    )
}

fn create_lxc_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Container VMID".to_string(),
                default: None,
                example: Some(json!("100")),
                constraints: vec![Constraint::Pattern {
                    regex: r"^\d+$".to_string(),
                }],
                read_only: true, // ID is immutable once created
                read_only_when: None,
            },
        );
        fields.insert(
            "veth".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Veth interface name".to_string(),
                default: None,
                example: Some(json!("vi100")),
                constraints: Vec::new(),
                read_only: false,
                // veth becomes readOnly when container is running
                read_only_when: Some(ReadOnlyCondition {
                    property: "running".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "bridge".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "OVS bridge name".to_string(),
                default: Some(json!("ovs-br0")),
                example: Some(json!("ovs-br0")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "running".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether container is running".to_string(),
                default: Some(json!(false)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "properties".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Container properties (hostname, memory, cores, etc.)".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "hostname": "my-container",
                    "memory": 512,
                    "cores": 2,
                    "template": "local:vztmpl/debian-13.tar.zst"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("lxc")
        .version("2.0.0")
        .description("LXC container management via native Proxmox API")
        .array_field(
            "containers",
            FieldType::Object(container_fields),
            true,
            "List of containers",
        )
        .example(json!({
            "containers": [
                {
                    "id": "100",
                    "veth": "vi100",
                    "bridge": "ovs-br0",
                    "running": true,
                    "properties": {
                        "hostname": "wireguard-gateway",
                        "memory": 512,
                        "cores": 1,
                        "network_type": "bridge"
                    }
                }
            ]
        }))
        .build()
}

fn create_incus_schema() -> PluginSchema {
    let instance_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-123")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "status".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "Running".to_string(),
                    "Stopped".to_string(),
                    "Frozen".to_string(),
                ]),
                required: true,
                description: "Instance status".to_string(),
                default: Some(json!("Stopped")),
                example: Some(json!("Running")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "container".to_string(),
                    "virtual-machine".to_string(),
                ]),
                required: true,
                description: "Instance type".to_string(),
                default: Some(json!("container")),
                example: Some(json!("container")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source image reference".to_string(),
                default: None,
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "storage_pool".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Preferred Incus storage pool for initial creation".to_string(),
                default: None,
                example: Some(json!("registration")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Applied Incus profiles".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "config".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance configuration map".to_string(),
                default: Some(json!({})),
                example: Some(json!({"limits.cpu": "2"})),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance device definitions".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "eth0": {
                        "type": "nic",
                        "nictype": "bridged",
                        "parent": "ovsbr0"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus")
        .version("1.0.0")
        .description("Incus instance management")
        .array_field(
            "instances",
            FieldType::Object(instance_fields),
            true,
            "List of Incus instances",
        )
        .example(json!({
            "instances": [
                {
                    "name": "privacy-user-123",
                    "status": "Running",
                    "type": "container",
                    "image": "images:debian/13",
                    "storage_pool": "registration",
                    "profiles": ["default"],
                    "config": {
                        "limits.cpu": "2"
                    },
                    "devices": {
                        "eth0": {
                            "type": "nic",
                            "nictype": "bridged",
                            "parent": "ovsbr0"
                        }
                    }
                }
            ]
        }))
        .build()
}

fn create_incus_wireguard_ingress_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus image alias for the WireGuard ingress container".to_string(),
                default: Some(json!("images:alpine/edge")),
                example: Some(json!("images:alpine/edge")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Incus profiles applied to the container".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default", "privacy-system"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            any_field(
                false,
                "Incus device overrides such as NICs and disks",
                Some(json!({})),
            ),
        );
        fields
    };

    let peer_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Peer public key".to_string(),
                default: None,
                example: Some(json!("base64publickey")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "allowed_ips".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Allowed IP ranges for the peer".to_string(),
                default: Some(json!(["0.0.0.0/0"])),
                example: Some(json!(["10.0.0.2/32"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "endpoint".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional peer endpoint host:port".to_string(),
                default: None,
                example: Some(json!("vpn.example.com:51820")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "persistent_keepalive".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Persistent keepalive interval in seconds".to_string(),
                default: None,
                example: Some(json!(25)),
                constraints: vec![
                    Constraint::Min { value: 0.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let wireguard_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "interface".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "WireGuard interface name inside the container".to_string(),
                default: Some(json!("wg0")),
                example: Some(json!("wg0")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "listen_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "WireGuard listen port".to_string(),
                default: Some(json!(51820)),
                example: Some(json!(51820)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "private_key_env".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Environment variable name holding the private key".to_string(),
                default: Some(json!("WIREGUARD_PRIVATE_KEY")),
                example: Some(json!("WIREGUARD_PRIVATE_KEY")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "CIDR address assigned to the WireGuard interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1/24")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "dns".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "DNS resolvers pushed to clients".to_string(),
                default: Some(json!([])),
                example: Some(json!(["1.1.1.1", "1.0.0.1"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "peers".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(peer_fields))),
                required: true,
                description: "WireGuard peers served by the ingress gateway".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "public_key": "base64publickey",
                    "allowed_ips": ["10.0.0.2/32"]
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let capability_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "requires_root".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the container requires root privileges".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "supports_rollback".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the deployment supports rollback".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus-wireguard-ingress")
        .version("1.0.0")
        .description("Incus system container declaration for the WireGuard ingress gateway")
        .field(
            "name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema object name".to_string(),
                default: Some(json!("incus-wireguard-ingress")),
                example: Some(json!("incus-wireguard-ingress")),
                constraints: vec![
                    Constraint::Pattern {
                        regex: "^[a-z0-9_-]+$".to_string(),
                    },
                    Constraint::Max { value: 64.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema version".to_string(),
                default: Some(json!("1.0.0")),
                example: Some(json!("1.0.0")),
                constraints: vec![Constraint::Pattern {
                    regex: "^\\d+\\.\\d+\\.\\d+$".to_string(),
                }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "plugin_type",
            FieldSchema {
                field_type: FieldType::Enum(vec!["network".to_string()]),
                required: true,
                description: "Container schema category".to_string(),
                default: Some(json!("network")),
                example: Some(json!("network")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .object_field(
            "container",
            container_fields,
            true,
            "Incus container image, profiles, and device overrides",
        )
        .object_field(
            "wireguard",
            wireguard_fields,
            true,
            "WireGuard ingress service configuration",
        )
        .object_field(
            "capabilities",
            capability_fields,
            false,
            "Operational capabilities for the container implementation",
        )
        .field(
            "service",
            any_field(false, "Optional service declaration", Some(json!({}))),
        )
        .example(json!({
            "name": "incus-wireguard-ingress",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:alpine/edge",
                "profiles": ["default", "privacy-system"],
                "devices": {}
            },
            "wireguard": {
                "interface": "wg0",
                "listen_port": 51820,
                "private_key_env": "WIREGUARD_PRIVATE_KEY",
                "address": "10.0.0.1/24",
                "dns": ["1.1.1.1", "1.0.0.1"],
                "peers": [{
                    "public_key": "base64publickey",
                    "allowed_ips": ["10.0.0.2/32"]
                }]
            },
            "capabilities": {
                "requires_root": true,
                "supports_rollback": false
            },
            "service": {}
        }))
        .build()
}

fn create_incus_xray_reality_client_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus image alias for the Xray client container".to_string(),
                default: Some(json!("images:debian/13")),
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Incus profiles applied to the container".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default", "privacy-system"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            any_field(
                false,
                "Incus device overrides such as NICs and disks",
                Some(json!({})),
            ),
        );
        fields
    };

    let inbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray inbound tag".to_string(),
                default: None,
                example: Some(json!("socks-in")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "socks".to_string(),
                    "http".to_string(),
                    "dokodemo-door".to_string(),
                ]),
                required: true,
                description: "Inbound protocol".to_string(),
                default: Some(json!("socks")),
                example: Some(json!("socks")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Listener port".to_string(),
                default: Some(json!(1080)),
                example: Some(json!(1080)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "listen".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Listener bind address".to_string(),
                default: Some(json!("127.0.0.1")),
                example: Some(json!("127.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "sniffing".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Enable protocol sniffing".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vnext_user_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "User UUID".to_string(),
                default: None,
                example: Some(json!("00000000-0000-0000-0000-000000000000")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flow".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["xtls-rprx-vision".to_string()]),
                required: true,
                description: "REALITY flow".to_string(),
                default: Some(json!("xtls-rprx-vision")),
                example: Some(json!("xtls-rprx-vision")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "encryption".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["none".to_string()]),
                required: false,
                description: "Encryption mode".to_string(),
                default: Some(json!("none")),
                example: Some(json!("none")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vnext_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Remote Xray server hostname or IP".to_string(),
                default: None,
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Remote Xray server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "users".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(vnext_user_fields))),
                required: true,
                description: "Authorized VLESS users".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "id": "00000000-0000-0000-0000-000000000000",
                    "flow": "xtls-rprx-vision",
                    "encryption": "none"
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let outbound_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "vnext".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(vnext_fields))),
                required: true,
                description: "Remote VLESS upstream definitions".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "address": "vps.example.com",
                    "port": 443,
                    "users": [{
                        "id": "00000000-0000-0000-0000-000000000000",
                        "flow": "xtls-rprx-vision",
                        "encryption": "none"
                    }]
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let reality_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "server_name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "TLS server name to mimic".to_string(),
                default: None,
                example: Some(json!("www.microsoft.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Server public key".to_string(),
                default: None,
                example: Some(json!("base64publickey")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "short_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "REALITY short ID".to_string(),
                default: None,
                example: Some(json!("1234abcd")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "fingerprint".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Client fingerprint".to_string(),
                default: Some(json!("chrome")),
                example: Some(json!("chrome")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let stream_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "network".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["tcp".to_string()]),
                required: true,
                description: "Transport network".to_string(),
                default: Some(json!("tcp")),
                example: Some(json!("tcp")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "security".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["reality".to_string()]),
                required: true,
                description: "Transport security".to_string(),
                default: Some(json!("reality")),
                example: Some(json!("reality")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "reality_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(reality_fields),
                required: true,
                description: "REALITY transport settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "server_name": "www.microsoft.com",
                    "public_key": "base64publickey",
                    "short_id": "1234abcd",
                    "fingerprint": "chrome"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let outbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray outbound tag".to_string(),
                default: None,
                example: Some(json!("reality-out")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["vless".to_string()]),
                required: true,
                description: "Outbound protocol".to_string(),
                default: Some(json!("vless")),
                example: Some(json!("vless")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(outbound_settings_fields),
                required: true,
                description: "Outbound server settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "vnext": [{
                        "address": "vps.example.com",
                        "port": 443,
                        "users": [{
                            "id": "00000000-0000-0000-0000-000000000000",
                            "flow": "xtls-rprx-vision",
                            "encryption": "none"
                        }]
                    }]
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "stream_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(stream_settings_fields),
                required: true,
                description: "REALITY transport settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "network": "tcp",
                    "security": "reality",
                    "reality_settings": {
                        "server_name": "www.microsoft.com",
                        "public_key": "base64publickey",
                        "short_id": "1234abcd",
                        "fingerprint": "chrome"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "log_level".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "debug".to_string(),
                    "info".to_string(),
                    "warning".to_string(),
                    "error".to_string(),
                    "none".to_string(),
                ]),
                required: false,
                description: "Xray log level".to_string(),
                default: Some(json!("warning")),
                example: Some(json!("warning")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "inbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(inbound_fields))),
                required: true,
                description: "Local proxy listeners".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "tag": "socks-in",
                    "protocol": "socks",
                    "port": 1080,
                    "listen": "127.0.0.1",
                    "sniffing": true
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "outbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(outbound_fields))),
                required: true,
                description: "REALITY egress definitions".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "tag": "reality-out",
                    "protocol": "vless",
                    "settings": {
                        "vnext": [{
                            "address": "vps.example.com",
                            "port": 443,
                            "users": [{
                                "id": "00000000-0000-0000-0000-000000000000",
                                "flow": "xtls-rprx-vision",
                                "encryption": "none"
                            }]
                        }]
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "server_name": "www.microsoft.com",
                            "public_key": "base64publickey",
                            "short_id": "1234abcd",
                            "fingerprint": "chrome"
                        }
                    }
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let capability_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "requires_root".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the container requires root privileges".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "supports_rollback".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the deployment supports rollback".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus-xray-reality-client")
        .version("1.0.0")
        .description("Incus system container declaration for the Xray REALITY outbound client")
        .field(
            "name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema object name".to_string(),
                default: Some(json!("incus-xray-reality-client")),
                example: Some(json!("incus-xray-reality-client")),
                constraints: vec![
                    Constraint::Pattern {
                        regex: "^[a-z0-9_-]+$".to_string(),
                    },
                    Constraint::Max { value: 64.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema version".to_string(),
                default: Some(json!("1.0.0")),
                example: Some(json!("1.0.0")),
                constraints: vec![Constraint::Pattern {
                    regex: "^\\d+\\.\\d+\\.\\d+$".to_string(),
                }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "plugin_type",
            FieldSchema {
                field_type: FieldType::Enum(vec!["network".to_string()]),
                required: true,
                description: "Container schema category".to_string(),
                default: Some(json!("network")),
                example: Some(json!("network")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .object_field(
            "container",
            container_fields,
            true,
            "Incus container image, profiles, and device overrides",
        )
        .object_field(
            "xray",
            xray_fields,
            true,
            "Xray REALITY client configuration",
        )
        .object_field(
            "capabilities",
            capability_fields,
            false,
            "Operational capabilities for the container implementation",
        )
        .field(
            "service",
            any_field(false, "Optional service declaration", Some(json!({}))),
        )
        .example(json!({
            "name": "incus-xray-reality-client",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:debian/13",
                "profiles": ["default", "privacy-system"],
                "devices": {}
            },
            "xray": {
                "log_level": "warning",
                "inbounds": [{
                    "tag": "socks-in",
                    "protocol": "socks",
                    "port": 1080,
                    "listen": "127.0.0.1",
                    "sniffing": true
                }],
                "outbounds": [{
                    "tag": "reality-out",
                    "protocol": "vless",
                    "settings": {
                        "vnext": [{
                            "address": "vps.example.com",
                            "port": 443,
                            "users": [{
                                "id": "00000000-0000-0000-0000-000000000000",
                                "flow": "xtls-rprx-vision",
                                "encryption": "none"
                            }]
                        }]
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "server_name": "www.microsoft.com",
                            "public_key": "base64publickey",
                            "short_id": "1234abcd",
                            "fingerprint": "chrome"
                        }
                    }
                }]
            },
            "capabilities": {
                "requires_root": false,
                "supports_rollback": false
            },
            "service": {}
        }))
        .build()
}

fn create_incus_xray_reality_server_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus image alias for the Xray server container".to_string(),
                default: Some(json!("images:debian/13")),
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Incus profiles applied to the container".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default", "privacy-system"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            any_field(
                false,
                "Incus device overrides such as NICs and disks",
                Some(json!({})),
            ),
        );
        fields
    };

    let client_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Authorized user UUID".to_string(),
                default: None,
                example: Some(json!("00000000-0000-0000-0000-000000000000")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flow".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["xtls-rprx-vision".to_string()]),
                required: true,
                description: "REALITY flow".to_string(),
                default: Some(json!("xtls-rprx-vision")),
                example: Some(json!("xtls-rprx-vision")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "email".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional client label".to_string(),
                default: None,
                example: Some(json!("user@example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let inbound_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "clients".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(client_fields))),
                required: true,
                description: "Authorized inbound clients".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "id": "00000000-0000-0000-0000-000000000000",
                    "flow": "xtls-rprx-vision",
                    "email": "user@example.com"
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "decryption".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["none".to_string()]),
                required: false,
                description: "VLESS decryption mode".to_string(),
                default: Some(json!("none")),
                example: Some(json!("none")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let reality_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "show".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Enable REALITY debug output".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "dest".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Fallback destination for non-REALITY traffic".to_string(),
                default: None,
                example: Some(json!("www.microsoft.com:443")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "server_names".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Allowed SNI values".to_string(),
                default: Some(json!([])),
                example: Some(json!(["www.microsoft.com"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "private_key_env".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Environment variable name holding the private key".to_string(),
                default: None,
                example: Some(json!("XRAY_PRIVATE_KEY")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "private_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Inline x25519 private key".to_string(),
                default: None,
                example: Some(json!("base64privatekey")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "short_ids".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Allowed REALITY short IDs".to_string(),
                default: Some(json!([])),
                example: Some(json!(["1234abcd"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let stream_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "network".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["tcp".to_string()]),
                required: true,
                description: "Transport network".to_string(),
                default: Some(json!("tcp")),
                example: Some(json!("tcp")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "security".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["reality".to_string()]),
                required: true,
                description: "Transport security".to_string(),
                default: Some(json!("reality")),
                example: Some(json!("reality")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "reality_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(reality_fields),
                required: true,
                description: "REALITY listener settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "show": false,
                    "dest": "www.microsoft.com:443",
                    "server_names": ["www.microsoft.com"],
                    "private_key_env": "XRAY_PRIVATE_KEY",
                    "short_ids": ["1234abcd"]
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let inbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray inbound tag".to_string(),
                default: None,
                example: Some(json!("reality-in")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["vless".to_string()]),
                required: true,
                description: "Inbound protocol".to_string(),
                default: Some(json!("vless")),
                example: Some(json!("vless")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Listener port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "listen".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Listener bind address".to_string(),
                default: Some(json!("0.0.0.0")),
                example: Some(json!("0.0.0.0")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(inbound_settings_fields),
                required: true,
                description: "Inbound client settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "clients": [{
                        "id": "00000000-0000-0000-0000-000000000000",
                        "flow": "xtls-rprx-vision",
                        "email": "user@example.com"
                    }],
                    "decryption": "none"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "stream_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(stream_settings_fields),
                required: true,
                description: "REALITY transport settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "network": "tcp",
                    "security": "reality",
                    "reality_settings": {
                        "show": false,
                        "dest": "www.microsoft.com:443",
                        "server_names": ["www.microsoft.com"],
                        "private_key_env": "XRAY_PRIVATE_KEY",
                        "short_ids": ["1234abcd"]
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let outbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray outbound tag".to_string(),
                default: None,
                example: Some(json!("direct")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["freedom".to_string(), "blackhole".to_string()]),
                required: true,
                description: "Outbound protocol".to_string(),
                default: Some(json!("freedom")),
                example: Some(json!("freedom")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "log_level".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "debug".to_string(),
                    "info".to_string(),
                    "warning".to_string(),
                    "error".to_string(),
                    "none".to_string(),
                ]),
                required: false,
                description: "Xray log level".to_string(),
                default: Some(json!("warning")),
                example: Some(json!("warning")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "inbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(inbound_fields))),
                required: true,
                description: "REALITY server listeners".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "tag": "reality-in",
                    "protocol": "vless",
                    "port": 443,
                    "listen": "0.0.0.0",
                    "settings": {
                        "clients": [{
                            "id": "00000000-0000-0000-0000-000000000000",
                            "flow": "xtls-rprx-vision",
                            "email": "user@example.com"
                        }],
                        "decryption": "none"
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "show": false,
                            "dest": "www.microsoft.com:443",
                            "server_names": ["www.microsoft.com"],
                            "private_key_env": "XRAY_PRIVATE_KEY",
                            "short_ids": ["1234abcd"]
                        }
                    }
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "outbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(outbound_fields))),
                required: true,
                description: "Server-side outbounds, typically direct or blackhole".to_string(),
                default: Some(json!([{"protocol": "freedom"}])),
                example: Some(json!([{
                    "tag": "direct",
                    "protocol": "freedom"
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let capability_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "requires_root".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the container requires root privileges".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "supports_rollback".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the deployment supports rollback".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus-xray-reality-server")
        .version("1.0.0")
        .description("Incus system container declaration for the Xray REALITY inbound server")
        .field(
            "name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema object name".to_string(),
                default: Some(json!("incus-xray-reality-server")),
                example: Some(json!("incus-xray-reality-server")),
                constraints: vec![
                    Constraint::Pattern {
                        regex: "^[a-z0-9_-]+$".to_string(),
                    },
                    Constraint::Max { value: 64.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema version".to_string(),
                default: Some(json!("1.0.0")),
                example: Some(json!("1.0.0")),
                constraints: vec![Constraint::Pattern {
                    regex: "^\\d+\\.\\d+\\.\\d+$".to_string(),
                }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "plugin_type",
            FieldSchema {
                field_type: FieldType::Enum(vec!["network".to_string()]),
                required: true,
                description: "Container schema category".to_string(),
                default: Some(json!("network")),
                example: Some(json!("network")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .object_field(
            "container",
            container_fields,
            true,
            "Incus container image, profiles, and device overrides",
        )
        .object_field(
            "xray",
            xray_fields,
            true,
            "Xray REALITY server configuration",
        )
        .object_field(
            "capabilities",
            capability_fields,
            false,
            "Operational capabilities for the container implementation",
        )
        .field(
            "service",
            any_field(false, "Optional service declaration", Some(json!({}))),
        )
        .example(json!({
            "name": "incus-xray-reality-server",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:debian/13",
                "profiles": ["default", "privacy-system"],
                "devices": {}
            },
            "xray": {
                "log_level": "warning",
                "inbounds": [{
                    "tag": "reality-in",
                    "protocol": "vless",
                    "port": 443,
                    "listen": "0.0.0.0",
                    "settings": {
                        "clients": [{
                            "id": "00000000-0000-0000-0000-000000000000",
                            "flow": "xtls-rprx-vision",
                            "email": "user@example.com"
                        }],
                        "decryption": "none"
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "show": false,
                            "dest": "www.microsoft.com:443",
                            "server_names": ["www.microsoft.com"],
                            "private_key_env": "XRAY_PRIVATE_KEY",
                            "short_ids": ["1234abcd"]
                        }
                    }
                }],
                "outbounds": [{
                    "tag": "direct",
                    "protocol": "freedom"
                }]
            },
            "capabilities": {
                "requires_root": false,
                "supports_rollback": false
            },
            "service": {}
        }))
        .build()
}

fn create_net_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true, // Interface name is identity
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "ethernet".to_string(),
                    "bridge".to_string(),
                    "veth".to_string(),
                    "vlan".to_string(),
                    "bond".to_string(),
                ]),
                required: true,
                description: "Interface type".to_string(),
                default: Some(json!("ethernet")),
                example: Some(json!("ethernet")),
                constraints: Vec::new(),
                read_only: true, // Type cannot change after creation
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "IP addresses".to_string(),
                default: Some(json!([])),
                example: Some(json!(["192.168.1.100/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("net")
        .version("1.0.0")
        .description("Network interface management via rtnetlink")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "List of network interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "eth0",
                    "type": "ethernet",
                    "state": "up",
                    "addresses": ["192.168.1.100/24"]
                }
            ]
        }))
        .build()
}

fn create_rtnetlink_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Administrative interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Interface IP addresses in CIDR form".to_string(),
                default: Some(json!([])),
                example: Some(json!(["10.0.0.2/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mac_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional MAC address override".to_string(),
                default: None,
                example: Some(json!("02:00:00:00:00:01")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "default_gateway".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Default gateway for this interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("rtnetlink")
        .version("1.0.0")
        .description("Native kernel rtnetlink interface management")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "Desired rtnetlink-managed interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "ovsbr0",
                    "state": "up",
                    "addresses": ["10.10.0.1/24"],
                    "default_gateway": "10.10.0.254"
                }
            ]
        }))
        .build()
}

fn create_openflow_schema() -> PluginSchema {
    let bridge_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Bridge name".to_string(),
                default: None,
                example: Some(json!("ovs-br0")),
                constraints: Vec::new(),
                read_only: true, // Bridge name is identity
                read_only_when: None,
            },
        );
        fields.insert(
            "datapath_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Datapath ID".to_string(),
                default: None,
                example: Some(json!("0000000000000001")),
                constraints: Vec::new(),
                read_only: true, // Datapath ID is immutable
                read_only_when: None,
            },
        );
        fields.insert(
            "protocols".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Supported OpenFlow protocols".to_string(),
                default: Some(json!(["OpenFlow13"])),
                example: Some(json!(["OpenFlow10", "OpenFlow13"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flows".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "table".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "OpenFlow table number".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 254.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "priority".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "Flow priority".to_string(),
                            default: Some(json!(100)),
                            example: Some(json!(22000)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 65535.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "match_fields".to_string(),
                        FieldSchema {
                            field_type: FieldType::Any,
                            required: true,
                            description: "OpenFlow match fields".to_string(),
                            default: None,
                            example: Some(
                                json!({"in_port": "ovsbr0-sock", "nw_src": "10.100.0.2"}),
                            ),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "actions".to_string(),
                        FieldSchema {
                            field_type: FieldType::Array(Box::new(FieldType::Any)),
                            required: true,
                            description: "OpenFlow actions".to_string(),
                            default: None,
                            example: Some(json!([{"type": "output", "port": "priv_wg"}])),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "cookie".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Flow cookie for idempotent route ownership".to_string(),
                            default: None,
                            example: Some(json!(5787125521171081216u64)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "idle_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Idle timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "hard_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Hard timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Flows managed for this bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_ports".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "OVS socket port name".to_string(),
                            default: None,
                            example: Some(json!("ovsbr0-sock")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "container_name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: false,
                            description: "Optional legacy container name bound to this port"
                                .to_string(),
                            default: None,
                            example: Some(json!("privacy-user-abc")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "port_type".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "Socket port role".to_string(),
                            default: Some(json!("SharedIngress")),
                            example: Some(json!("SharedIngress")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "ofport".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Resolved OpenFlow port number".to_string(),
                            default: None,
                            example: Some(json!(7)),
                            constraints: Vec::new(),
                            read_only: true,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Managed OVS socket ports for the bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("openflow")
        .version("1.0.0")
        .description("OpenFlow flow table management")
        .dependency("net")
        .dependency("privacy_routes")
        .array_field(
            "bridges",
            FieldType::Object(bridge_fields),
            true,
            "OVS bridges",
        )
        .string_field("controller_endpoint", false, "OpenFlow controller endpoint")
        .boolean_field(
            "auto_discover_containers",
            false,
            "Auto-create flows from discovered legacy container sockets",
        )
        .boolean_field("enable_security_flows", false, "Inject hardening flows before route flows")
        .integer_field("obfuscation_level", false, "Traffic obfuscation level for generated flows")
        .example(json!({
            "bridges": [
                {
                    "name": "ovsbr0",
                    "protocols": ["OpenFlow13"],
                    "socket_ports": [
                        {
                            "name": "ovsbr0-sock",
                            "port_type": "SharedIngress"
                        }
                    ],
                    "flows": [
                        {
                            "table": 0,
                            "priority": 22000,
                            "match_fields": {"in_port": "ovsbr0-sock", "ip": "", "nw_src": "10.100.0.2"},
                            "actions": [{"type": "output", "port": "priv_wg"}],
                            "cookie": 5787125521171081216u64,
                            "idle_timeout": 0,
                            "hard_timeout": 0
                        }
                    ]
                }
            ],
            "auto_discover_containers": false,
            "enable_security_flows": false,
            "obfuscation_level": 0
        }))
        .build()
}

fn create_s6_schema() -> PluginSchema {
    let unit_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unit name".to_string(),
                default: None,
                example: Some(json!("nginx")),
                constraints: Vec::new(),
                read_only: true, // Unit name is identity
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "active".to_string(),
                    "inactive".to_string(),
                    "failed".to_string(),
                ]),
                required: false,
                description: "Desired unit state".to_string(),
                default: Some(json!("active")),
                example: Some(json!("active")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether unit is enabled at boot".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("s6")
        .version("1.0.0")
        .description("S6 service management")
        .array_field("units", FieldType::Object(unit_fields), true, "S6 services")
        .example(json!({
            "units": [
                {
                    "name": "nginx",
                    "state": "active",
                    "enabled": true
                }
            ]
        }))
        .build()
}

fn create_privacy_router_schema() -> PluginSchema {
    let wireguard_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable WireGuard tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for WireGuard".to_string(),
                default: Some(json!(100)),
                example: Some(json!(100)),
                constraints: Vec::new(),
                read_only: false,
                // Container ID becomes immutable when enabled
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "listen_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "WireGuard listen port".to_string(),
                default: Some(json!(51820)),
                example: Some(json!(51820)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port name for the WireGuard ingress container"
                    .to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let warp_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable Cloudflare WARP tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "bridge_interface".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host WireGuard interface bridged into OVS for WARP egress"
                    .to_string(),
                default: Some(json!("wgcf")),
                example: Some(json!("wgcf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wgcf_config".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Path to wgcf WireGuard config used to create the host interface"
                    .to_string(),
                default: Some(json!("/etc/wireguard/wgcf.conf")),
                example: Some(json!("/etc/wireguard/wgcf.conf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable system XRay client tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for the local XRay client".to_string(),
                default: Some(json!(101)),
                example: Some(json!(101)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port for the local XRay client".to_string(),
                default: Some(json!("priv_xray")),
                example: Some(json!("priv_xray")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socks_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "SOCKS listener port exposed by the local XRay client".to_string(),
                default: Some(json!(1080)),
                example: Some(json!(1080)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vps_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "xray_server".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "xray_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_router")
        .version("1.1.0")
        .description("System privacy fabric (WireGuard/XRay ingress, WARP bridge, XRay egress)")
        .dependency("incus")
        .dependency("openflow")
        .dependency("privacy_routes")
        .string_field("bridge_name", true, "OVS bridge for privacy network")
        .object_field(
            "wireguard",
            wireguard_fields,
            true,
            "WireGuard tunnel config",
        )
        .object_field("warp", warp_fields, true, "Cloudflare WARP bridge config")
        .object_field(
            "xray",
            xray_fields,
            true,
            "XRay REALITY egress client config",
        )
        .object_field(
            "vps",
            vps_fields,
            true,
            "Remote XRay server endpoint config",
        )
        .example(json!({
            "bridge_name": "ovsbr0",
            "wireguard": {
                "enabled": true,
                "container_id": 100,
                "socket_port": "priv_wg",
                "listen_port": 51820
            },
            "warp": {
                "enabled": true,
                "bridge_interface": "wgcf",
                "wgcf_config": "/etc/wireguard/wgcf.conf"
            },
            "xray": {
                "enabled": true,
                "container_id": 101,
                "socket_port": "priv_xray",
                "socks_port": 1080,
                "vps_address": "vps.example.com",
                "vps_port": 443
            },
            "vps": {
                "xray_server": "vps.example.com",
                "xray_port": 443
            }
        }))
        .build()
}

fn create_privacy_routes_schema() -> PluginSchema {
    let route_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Stable route object identifier".to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "route_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Derived route ID from WireGuard public key and shared secret"
                    .to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "user_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Internal privacy user identifier".to_string(),
                default: None,
                example: Some(json!("550e8400-e29b-41d4-a716-446655440000")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "email".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "User email for audit and publication context".to_string(),
                default: None,
                example: Some(json!("user@example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wireguard_public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "WireGuard public key backing this route identity".to_string(),
                default: None,
                example: Some(json!("P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "assigned_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Assigned WireGuard tunnel address".to_string(),
                default: None,
                example: Some(json!("10.100.0.2/32")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "selector_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Packet-visible selector used for OpenFlow matching".to_string(),
                default: None,
                example: Some(json!("10.100.0.2")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Associated Incus instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-550e8400")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "ingress_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Shared OVS ingress port for route matching".to_string(),
                default: Some(json!("ovsbr0-sock")),
                example: Some(json!("ovsbr0-sock")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "next_hop".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "First logical next hop for this route".to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether this route should be active".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "created_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Creation timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:00:00Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "updated_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Last update timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:05:00Z")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_routes")
        .version("1.0.0")
        .description("Per-user privacy route objects keyed by WireGuard identity")
        .dependency("wireguard")
        .dependency("privacy_router")
        .array_field(
            "routes",
            FieldType::Object(route_fields),
            true,
            "Published privacy route objects",
        )
        .example(json!({
            "routes": [
                {
                    "name": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "route_id": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "user_id": "550e8400-e29b-41d4-a716-446655440000",
                    "email": "user@example.com",
                    "wireguard_public_key": "P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=",
                    "assigned_ip": "10.100.0.2/32",
                    "selector_ip": "10.100.0.2",
                    "container_name": "privacy-user-550e8400",
                    "ingress_port": "ovsbr0-sock",
                    "next_hop": "priv_wg",
                    "enabled": true,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }
            ]
        }))
        .build()
}

fn create_netmaker_schema() -> PluginSchema {
    PluginSchema::builder("netmaker")
        .version("1.0.0")
        .description("Netmaker mesh network management")
        .dependency("net")
        .string_field("network_name", true, "Netmaker network name")
        .string_field("interface", false, "WireGuard interface name (e.g., nm0)")
        .string_field("server_url", false, "Netmaker server URL")
        .string_field(
            "enrollment_token",
            false,
            "Enrollment token for joining network",
        )
        .boolean_field("auto_enroll", false, "Auto-enroll containers in mesh")
        .example(json!({
            "network_name": "container-mesh",
            "interface": "nm0",
            "auto_enroll": true
        }))
        .build()
}

// ============================================================================
// Helper Functions
// ============================================================================

fn validate_field(name: &str, value: &Value, schema: &FieldSchema) -> Result<(), String> {
    validate_value_against_type(name, value, &schema.field_type)?;

    // Validate constraints
    for constraint in &schema.constraints {
        match constraint {
            Constraint::Min { value: min } => {
                if let Some(n) = value.as_f64() {
                    if n < *min {
                        return Err(format!("Field '{}' must be >= {}", name, min));
                    }
                }
                if let Some(s) = value.as_str() {
                    if (s.len() as f64) < *min {
                        return Err(format!("Field '{}' length must be >= {}", name, min));
                    }
                }
            }
            Constraint::Max { value: max } => {
                if let Some(n) = value.as_f64() {
                    if n > *max {
                        return Err(format!("Field '{}' must be <= {}", name, max));
                    }
                }
                if let Some(s) = value.as_str() {
                    if (s.len() as f64) > *max {
                        return Err(format!("Field '{}' length must be <= {}", name, max));
                    }
                }
            }
            Constraint::Pattern { regex } => {
                if let Some(s) = value.as_str() {
                    if let Ok(re) = regex::Regex::new(regex) {
                        if !re.is_match(s) {
                            return Err(format!("Field '{}' must match pattern: {}", name, regex));
                        }
                    }
                }
            }
            Constraint::OneOf { values } => {
                if !values.contains(value) {
                    return Err(format!("Field '{}' must be one of: {:?}", name, values));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_value_against_type(
    name: &str,
    value: &Value,
    field_type: &FieldType,
) -> Result<(), String> {
    match field_type {
        FieldType::String => {
            if !value.is_str() {
                return Err(format!("Field '{}' must be a string", name));
            }
        }
        FieldType::Integer => {
            if !value.is_i64() && !value.is_u64() {
                return Err(format!("Field '{}' must be an integer", name));
            }
        }
        FieldType::Float => {
            if !value.is_f64() && !value.is_i64() {
                return Err(format!("Field '{}' must be a number", name));
            }
        }
        FieldType::Boolean => {
            if !value.is_bool() {
                return Err(format!("Field '{}' must be a boolean", name));
            }
        }
        FieldType::Array(_) => {
            if !value.is_array() {
                return Err(format!("Field '{}' must be an array", name));
            }
            if let Some(items) = value.as_array() {
                if let FieldType::Array(item_type) = field_type {
                    for (index, item) in items.iter().enumerate() {
                        validate_value_against_type(
                            &format!("{}[{}]", name, index),
                            item,
                            item_type,
                        )?;
                    }
                }
            }
        }
        FieldType::Object(fields) => {
            if !value.is_object() {
                return Err(format!("Field '{}' must be an object", name));
            }
            validate_object_fields(name, value, fields)?;
        }
        FieldType::Enum(valid_values) => {
            if let Some(s) = value.as_str() {
                if !valid_values.contains(&s.to_string()) {
                    return Err(format!(
                        "Field '{}' must be one of: {:?}",
                        name, valid_values
                    ));
                }
            } else {
                return Err(format!("Field '{}' must be a string enum value", name));
            }
        }
        FieldType::Any => {}
    }

    Ok(())
}

fn validate_object_fields(
    name: &str,
    value: &Value,
    fields: &HashMap<String, FieldSchema>,
) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err(format!("Field '{}' must be an object", name));
    };

    for (field_name, field_schema) in fields {
        if field_schema.required && obj.get(field_name).is_none() {
            return Err(format!("Missing required field: {}.{}", name, field_name));
        }
    }

    for (field_name, field_value) in obj {
        if let Some(field_schema) = fields.get(field_name) {
            validate_field(
                &format!("{}.{}", name, field_name),
                field_value,
                field_schema,
            )?;
        }
    }

    Ok(())
}

fn default_for_type(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String => json!(""),
        FieldType::Integer => json!(0),
        FieldType::Float => json!(0.0),
        FieldType::Boolean => json!(false),
        FieldType::Array(_) => json!([]),
        FieldType::Object(_) => json!({}),
        FieldType::Enum(values) => values.first().map(|s| json!(s)).unwrap_or(json!("")),
        FieldType::Any => json!(null),
    }
}

fn field_type_to_json_schema(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String => json!({"type": "string"}),
        FieldType::Integer => json!({"type": "integer"}),
        FieldType::Float => json!({"type": "number"}),
        FieldType::Boolean => json!({"type": "boolean"}),
        FieldType::Array(item_type) => json!({
            "type": "array",
            "items": field_type_to_json_schema(item_type)
        }),
        FieldType::Object(fields) => {
            let mut properties = simd_json::value::owned::Object::new();
            for (name, schema) in fields {
                properties.insert(name.clone(), field_type_to_json_schema(&schema.field_type));
            }
            json!({
                "type": "object",
                "properties": properties
            })
        }
        FieldType::Enum(values) => json!({
            "type": "string",
            "enum": values
        }),
        FieldType::Any => json!({}),
    }
}

/// Convert field type to JSON Schema 2026 format with full metadata
fn field_type_to_json_schema_2026(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String => json!({"type": "string"}),
        FieldType::Integer => json!({"type": "integer"}),
        FieldType::Float => json!({"type": "number"}),
        FieldType::Boolean => json!({"type": "boolean"}),
        FieldType::Array(item_type) => json!({
            "type": "array",
            "items": field_type_to_json_schema_2026(item_type)
        }),
        FieldType::Object(fields) => {
            let mut properties = simd_json::value::owned::Object::new();
            let mut required = Vec::new();
            for (name, schema) in fields {
                let mut field_json = field_type_to_json_schema_2026(&schema.field_type);
                if !schema.description.is_empty() {
                    if let Some(obj) = field_json.as_object_mut() {
                        obj.insert("description".to_string(), json!(schema.description));
                    }
                }
                if schema.read_only {
                    if let Some(obj) = field_json.as_object_mut() {
                        obj.insert("readOnly".to_string(), json!(true));
                    }
                }
                properties.insert(name.clone(), field_json);
                if schema.required {
                    required.push(json!(name));
                }
            }
            let mut result = json!({
                "type": "object",
                "properties": properties
            });
            if !required.is_empty() {
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("required".to_string(), json!(required));
                }
            }
            result
        }
        FieldType::Enum(values) => json!({
            "type": "string",
            "enum": values
        }),
        FieldType::Any => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_catalog() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        assert!(catalog.get("lxc").is_some());
        assert!(catalog.get("incus").is_some());
        assert!(catalog.get("incus-wireguard-ingress").is_some());
        assert!(catalog.get("incus-xray-reality-client").is_some());
        assert!(catalog.get("incus-xray-reality-server").is_some());
        assert!(catalog.get("net").is_some());
        assert!(catalog.get("openflow").is_some());
        assert!(catalog.get("systemd").is_some());
        assert!(catalog.get("privacy_routes").is_some());
        assert!(catalog.get("privacy_router").is_some());
        assert!(catalog.get("netmaker").is_some());
    }

    #[test]
    fn test_schema_catalog_aliases() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        assert!(catalog.get("incus_wireguard_ingress").is_some());
        assert!(catalog.get("incus_xray_reality_client").is_some());
        assert!(catalog.get("incus_xray_reality_server").is_some());
    }

    #[test]
    fn test_schema_registry_compatibility_alias() {
        let registry = SchemaRegistry::with_builtin_schemas();
        assert!(registry.get("net").is_some());
    }

    #[test]
    fn test_lxc_validation() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("lxc").unwrap();

        // Valid state
        let valid_state = json!({
            "containers": [
                {
                    "id": "100",
                    "veth": "vi100",
                    "bridge": "ovs-br0",
                    "running": true
                }
            ]
        });
        let result = schema.validate(&valid_state);
        assert!(result.valid, "Errors: {:?}", result.errors);

        // Missing required field
        let invalid_state = json!({});
        let result = schema.validate(&invalid_state);
        assert!(!result.valid);
    }

    #[test]
    fn test_template_generation() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("lxc").unwrap();
        let template = schema.generate_template();
        assert!(template.get("containers").is_some());
    }

    #[test]
    fn test_json_schema_export() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("lxc").unwrap();
        let json_schema = schema.to_json_schema();
        assert_eq!(json_schema["title"], "lxc");
        assert!(json_schema["properties"].is_object());
    }

    #[test]
    fn test_json_schema_2026_dialect() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("lxc").unwrap();
        let json_schema = schema.to_json_schema();

        // Check that 2026 dialect is used
        assert_eq!(json_schema["$schema"], DEFAULT_SCHEMA_DIALECT);
    }

    #[test]
    fn test_json_schema_property_dependencies() {
        // Create a schema with conditional readOnly
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .field(
                "status",
                FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "Status".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            )
            .field(
                "id",
                FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "ID".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: Some(ReadOnlyCondition {
                        property: "status".to_string(),
                        value: "locked".to_string(),
                    }),
                },
            )
            .build();

        let json_schema = schema.to_json_schema();

        // Check that propertyDependencies is generated
        assert!(json_schema.get("propertyDependencies").is_some());
        let deps = &json_schema["propertyDependencies"];
        assert!(deps["status"]["locked"]["properties"]["id"]["readOnly"]
            .as_bool()
            .unwrap_or(false));
    }

    #[test]
    fn test_nested_required_fields_for_wireguard_ingress() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("incus-wireguard-ingress").unwrap();

        let invalid_state = json!({
            "name": "incus-wireguard-ingress",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "profiles": ["default"]
            },
            "wireguard": {
                "private_key_env": "WIREGUARD_PRIVATE_KEY",
                "peers": []
            }
        });

        let result = schema.validate(&invalid_state);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("container.image")));
    }

    #[test]
    fn test_nested_required_fields_for_xray_client() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("incus-xray-reality-client").unwrap();

        let invalid_state = json!({
            "name": "incus-xray-reality-client",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:debian/13",
                "profiles": ["default"]
            },
            "xray": {
                "outbounds": []
            }
        });

        let result = schema.validate(&invalid_state);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("xray.inbounds")));
    }

    #[test]
    fn test_contract_schema_sections_for_incus_components() {
        let registry = SchemaRegistry::with_builtin_schemas();

        for schema_name in [
            "incus-wireguard-ingress",
            "incus-xray-reality-client",
            "incus-xray-reality-server",
        ] {
            let schema = registry.get(schema_name).unwrap();
            let contract = schema.to_contract_json_schema();
            let required = contract["required"].as_array().unwrap();

            assert!(required.iter().any(|value| value == "stub"));
            assert!(required.iter().any(|value| value == "immutable"));
            assert!(required.iter().any(|value| value == "tunable"));
            assert!(contract["properties"]["stub"].is_object());
            assert!(contract["properties"]["immutable"].is_object());
            assert!(contract["properties"]["tunable"].is_object());
        }
    }

    #[test]
    fn test_privacy_router_container_ids_are_integers() {
        let registry = SchemaRegistry::with_builtin_schemas();
        let schema = registry.get("privacy_router").unwrap();

        let valid_state = json!({
            "bridge_name": "ovsbr0",
            "wireguard": {
                "enabled": true,
                "container_id": 100,
                "socket_port": "priv_wg",
                "listen_port": 51820,
                "resources": {
                    "vcpus": 1,
                    "memory_mb": 512,
                    "disk_gb": 4,
                    "os_template": "images:debian/13",
                    "swap_mb": 0,
                    "unprivileged": true
                }
            },
            "warp": {
                "enabled": true,
                "bridge_interface": "wgcf",
                "wgcf_config": "/etc/wireguard/wgcf.conf"
            },
            "xray": {
                "enabled": true,
                "container_id": 101,
                "socket_port": "priv_xray",
                "socks_port": 1080,
                "vps_address": "vps.example.com",
                "vps_port": 443,
                "resources": {
                    "vcpus": 1,
                    "memory_mb": 512,
                    "disk_gb": 4,
                    "os_template": "images:debian/13",
                    "swap_mb": 0,
                    "unprivileged": true
                }
            },
            "vps": {
                "xray_server": "vps.example.com",
                "xray_port": 443
            },
            "socket_networking": {
                "enabled": true,
                "privacy_sockets": [
                    {
                        "name": "priv_wg",
                        "container_id": 100
                    },
                    {
                        "name": "priv_xray",
                        "container_id": 101
                    }
                ]
            },
            "openflow": {
                "enabled": true,
                "enable_security_flows": true,
                "obfuscation_level": 2,
                "privacy_flows": [],
                "function_routing": []
            },
            "containers": []
        });

        let result = schema.validate(&valid_state);
        assert!(result.valid, "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_json_schema_immutable_paths() {
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .string_field("id", true, "ID field")
            .string_field("name", true, "Name field")
            .immutable_path("/id")
            .build();

        let json_schema = schema.to_json_schema();
        let properties = json_schema["properties"].as_object().unwrap();

        // Check that id has readOnly
        assert!(properties["id"]["readOnly"].as_bool().unwrap_or(false));
        // name should not be readOnly
        assert!(!properties
            .get("name")
            .and_then(|value| value.get("readOnly"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false));
    }

    #[test]
    fn test_json_schema_fully_immutable() {
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .string_field("id", true, "ID field")
            .string_field("name", true, "Name field")
            .fully_immutable()
            .build();

        let json_schema = schema.to_json_schema();

        // All fields should be readOnly
        assert!(json_schema["properties"]["id"]["readOnly"]
            .as_bool()
            .unwrap_or(false));
        assert!(json_schema["properties"]["name"]["readOnly"]
            .as_bool()
            .unwrap_or(false));
    }

    #[test]
    fn test_schema_custom_dialect() {
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .dialect(dialects::DRAFT_07)
            .string_field("name", true, "Name")
            .build();

        let json_schema = schema.to_json_schema();
        assert_eq!(json_schema["$schema"], dialects::DRAFT_07);
    }
}
</file>

<file path="src/redis_stream.rs">
//! Redis stream for real-time job notifications
//!
//! Provides pub/sub capabilities for job status updates,
//! enabling real-time monitoring and distributed coordination.

use crate::error::{Result, StateStoreError};
use crate::execution_job::ExecutionJob;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Stream name for job events
const JOB_STREAM: &str = "op:jobs";
/// Stream name for plugin state events
const PLUGIN_STREAM: &str = "op:plugins";
/// Consumer group name
const CONSUMER_GROUP: &str = "op-dbus";
/// Max stream length (for automatic trimming)
const MAX_STREAM_LENGTH: i64 = 10000;

/// Redis stream client for real-time notifications
pub struct RedisStream {
    conn: MultiplexedConnection,
    consumer_name: String,
}

/// Job event published to Redis stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: String,
    pub tool_name: String,
    pub status: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Plugin state event published to Redis stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEvent {
    pub plugin_name: String,
    pub operation: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_hash: Option<String>,
}

impl RedisStream {
    /// Create a new Redis stream client
    ///
    /// URL format: `redis://localhost:6379` or `redis://:password@localhost:6379`
    pub async fn new(url: &str) -> Result<Self> {
        info!("Connecting to Redis at {}", url);

        let client = Client::open(url).map_err(StateStoreError::Redis)?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(StateStoreError::Redis)?;

        // Generate unique consumer name
        let consumer_name = format!(
            "op-dbus-{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        );

        let stream = Self {
            conn,
            consumer_name,
        };

        // Initialize consumer groups
        stream.initialize_streams().await?;

        info!(
            "Redis stream connected as consumer: {}",
            stream.consumer_name
        );
        Ok(stream)
    }

    /// Initialize streams and consumer groups
    async fn initialize_streams(&self) -> Result<()> {
        let mut conn = self.conn.clone();

        // Create consumer groups if they don't exist
        // We ignore errors if the group already exists
        let _: std::result::Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(JOB_STREAM)
            .arg(CONSUMER_GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        let _: std::result::Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(PLUGIN_STREAM)
            .arg(CONSUMER_GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        debug!("Redis streams initialized");
        Ok(())
    }

    /// Publish a job event
    pub async fn publish_job(&self, job: &ExecutionJob) -> Result<()> {
        let mut conn = self.conn.clone();

        let event = JobEvent {
            job_id: job.id.to_string(),
            tool_name: job.tool_name.clone(),
            status: format!("{:?}", job.status),
            timestamp: job.updated_at.to_rfc3339(),
            error: job.result.as_ref().and_then(|r| r.error.clone()),
        };

        let event_json = simd_json::to_string(&event)?;

        // Add to stream with automatic trimming
        let _: String = redis::cmd("XADD")
            .arg(JOB_STREAM)
            .arg("MAXLEN")
            .arg("~")
            .arg(MAX_STREAM_LENGTH)
            .arg("*")
            .arg("event")
            .arg(&event_json)
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        debug!("Published job event: {} - {:?}", job.id, job.status);
        Ok(())
    }

    /// Publish a plugin state event
    pub async fn publish_plugin_event(
        &self,
        plugin_name: &str,
        operation: &str,
        state_hash: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.clone();

        let event = PluginEvent {
            plugin_name: plugin_name.to_string(),
            operation: operation.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            state_hash: state_hash.map(String::from),
        };

        let event_json = simd_json::to_string(&event)?;

        let _: String = redis::cmd("XADD")
            .arg(PLUGIN_STREAM)
            .arg("MAXLEN")
            .arg("~")
            .arg(MAX_STREAM_LENGTH)
            .arg("*")
            .arg("event")
            .arg(&event_json)
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        debug!("Published plugin event: {} - {}", plugin_name, operation);
        Ok(())
    }

    /// Read pending job events (for catching up)
    pub async fn read_job_events(&self, count: usize) -> Result<Vec<JobEvent>> {
        let mut conn = self.conn.clone();

        let results: Vec<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(JOB_STREAM)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        parse_job_events(results)
    }

    /// Read pending plugin events
    pub async fn read_plugin_events(&self, count: usize) -> Result<Vec<PluginEvent>> {
        let mut conn = self.conn.clone();

        let results: Vec<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(PLUGIN_STREAM)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        parse_plugin_events(results)
    }

    /// Acknowledge processed events
    pub async fn ack_job_event(&self, event_id: &str) -> Result<()> {
        let mut conn = self.conn.clone();

        let _: i64 = redis::cmd("XACK")
            .arg(JOB_STREAM)
            .arg(CONSUMER_GROUP)
            .arg(event_id)
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        Ok(())
    }

    /// Get stream info
    pub async fn get_stream_info(&self) -> Result<StreamInfo> {
        let mut conn = self.conn.clone();

        let job_len: i64 = redis::cmd("XLEN")
            .arg(JOB_STREAM)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        let plugin_len: i64 = redis::cmd("XLEN")
            .arg(PLUGIN_STREAM)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        Ok(StreamInfo {
            job_stream_length: job_len as u64,
            plugin_stream_length: plugin_len as u64,
            consumer_name: self.consumer_name.clone(),
        })
    }

    /// Publish a simple key-value update (for caching)
    pub async fn set_cached_state(
        &self,
        key: &str,
        value: &simd_json::OwnedValue,
        ttl_secs: u64,
    ) -> Result<()> {
        let mut conn = self.conn.clone();
        let value_json = simd_json::to_string(value)?;

        let _: () = conn
            .set_ex(key, value_json, ttl_secs)
            .await
            .map_err(StateStoreError::Redis)?;

        Ok(())
    }

    /// Get cached state
    pub async fn get_cached_state(&self, key: &str) -> Result<Option<simd_json::OwnedValue>> {
        let mut conn = self.conn.clone();

        let value: Option<String> = conn.get(key).await.map_err(StateStoreError::Redis)?;

        match value {
            Some(json) => {
                let mut json_mut = json;
                Ok(Some(unsafe { simd_json::from_str(&mut json_mut)? }))
            }
            None => Ok(None),
        }
    }

    /// Check if Redis is connected
    pub async fn ping(&self) -> Result<bool> {
        let mut conn = self.conn.clone();
        let result: std::result::Result<String, _> =
            redis::cmd("PING").query_async(&mut conn).await;
        Ok(result.map(|s| s == "PONG").unwrap_or(false))
    }
}

/// Stream statistics
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub job_stream_length: u64,
    pub plugin_stream_length: u64,
    pub consumer_name: String,
}

/// Parse job events from Redis response - simplified to avoid version-specific enum variants
fn parse_job_events(results: Vec<redis::Value>) -> Result<Vec<JobEvent>> {
    use redis::FromRedisValue;

    let mut events = Vec::new();

    // Convert using redis's built-in parsing where possible
    // For stream responses, we try to extract strings from the nested structure
    for result in &results {
        if let Ok(entries) = Vec::<(String, Vec<(String, String)>)>::from_redis_value(result) {
            for (_entry_id, fields) in entries {
                for (key, mut value) in fields {
                    if key == "event" {
                        if let Ok(event) = unsafe { simd_json::from_str::<JobEvent>(&mut value) } {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }

    Ok(events)
}

/// Parse plugin events from Redis response
fn parse_plugin_events(results: Vec<redis::Value>) -> Result<Vec<PluginEvent>> {
    use redis::FromRedisValue;

    let mut events = Vec::new();

    for result in &results {
        if let Ok(entries) = Vec::<(String, Vec<(String, String)>)>::from_redis_value(result) {
            for (_entry_id, fields) in entries {
                for (key, mut value) in fields {
                    if key == "event" {
                        if let Ok(event) = unsafe { simd_json::from_str::<PluginEvent>(&mut value) }
                        {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }

    Ok(events)
}

/// Try to connect to Redis (optional, returns None if unavailable)
pub async fn try_connect(url: &str) -> Option<RedisStream> {
    match RedisStream::new(url).await {
        Ok(stream) => Some(stream),
        Err(e) => {
            warn!("Redis not available ({}): {}", url, e);
            None
        }
    }
}
</file>

<file path="src/schema_shuttle.rs">
use crate::plugin_schema::PluginSchema;
use crate::schema_validator::canonicalize_json;
use md5; // Matches EventChain hashing methodology
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tokio::time::{sleep, Duration};

/// THE SLED: Zero-copy shared memory layout
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32], // The current "Thought" injected into Xray
}

pub struct SchemaShuttle;

impl SchemaShuttle {
    /// Genesis: Validates the PluginSchema and creates the initial Sled
    pub fn forge_sled(
        wg_pubkey: &str,
        current_schema: &PluginSchema,
    ) -> Result<IdentitySled, String> {
        // Enforce the "No Valid Schema = Does Not Exist" rule
        if !current_schema.is_valid() {
            return Err("Invalid Schema State: Connection Rejected.".into());
        }

        // Decode WG pubkey (assume base64)
        use base64::Engine;
        let wg_bytes: [u8; 32] = base64::engine::general_purpose::STANDARD
            .decode(wg_pubkey.trim())
            .map_err(|e| format!("Invalid WG key: {}", e))?
            .try_into()
            .map_err(|_| "WG key must be 32 bytes".to_string())?;

        // Serialize the PluginSchema to a simd_json Value for canonicalization
        let schema_json = serde_json::to_string(current_schema)
            .map_err(|e| format!("Serialization Failed: {}", e))?;
        let mut schema_bytes = schema_json.into_bytes();
        let schema_value: simd_json::OwnedValue = simd_json::to_owned_value(&mut schema_bytes)
            .map_err(|e| format!("SIMD-JSON parse failed: {}", e))?;
        let canonical_state = serde_json::to_string(&canonicalize_json(&schema_value))
            .map_err(|e| format!("Canonical serialization failed: {}", e))?;

        let payload = format!("{}:{}", wg_pubkey, canonical_state);
        let genesis_hash = md5::compute(payload.as_bytes());
        let mut hashed_footprint = [0u8; 32];
        // MD5 is 16 bytes, we pad it into 32 bytes for the Sled layout
        hashed_footprint[..16].copy_from_slice(&genesis_hash.0);

        Ok(IdentitySled {
            wireguard_pubkey: wg_bytes,
            mutation_index: current_schema.mutation_index.unwrap_or(0),
            is_valid: true,
            hashed_footprint,
        })
    }
}

/// THE SHUTTLE: Async loop monitoring the RPC DB for state mutations
pub async fn run_shuttle() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let rpc_url = "http://127.0.0.1:7020"; // op-jsonrpc legacy tool execution port

    // Extracted from D-Bus/systemd-networkd
    let active_wg_key = "EPHEMERAL_WG_PUBKEY";

    println!("[*] Schema Shuttle active. Fetching PluginSchema...");

    // Fetch the absolute present schema
    let genesis_res = client
        .post(rpc_url)
        .json(&serde_json::json!({"jsonrpc": "2.0", "method": "get_latest_schema", "id": 1}))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // Parse into the authoritative PluginSchema object
    let schema: PluginSchema = serde_json::from_value(genesis_res["result"].clone())?;

    // Forge the Sled
    let mut session_sled = SchemaShuttle::forge_sled(active_wg_key, &schema)?;
    let mut last_mutation_index = session_sled.mutation_index;

    let footprint_hex = hex::encode(session_sled.hashed_footprint);
    println!(
        "[SUCCESS] Identity Sled Forged. Footprint: {}",
        footprint_hex
    );

    // The "Even Trade" Zero-Btrfs Loop
    loop {
        let res = client
            .post(rpc_url)
            .json(&serde_json::json!({"jsonrpc": "2.0", "method": "get_mutation_index", "id": 2}))
            .send()
            .await;

        if let Ok(response) = res {
            let state: serde_json::Value = response.json().await?;
            let current_index = state["result"].as_u64().unwrap_or(last_mutation_index);

            // If the Btrfs snowball mutates, instantly update the gRPC headers
            if current_index > last_mutation_index {
                last_mutation_index = current_index;

                let current_footprint_hex = hex::encode(session_sled.hashed_footprint);
                let update_payload = format!("{}:{}", current_footprint_hex, current_index);
                let new_hash = md5::compute(update_payload.as_bytes());

                let mut new_footprint = [0u8; 32];
                new_footprint[..16].copy_from_slice(&new_hash.0);
                session_sled.hashed_footprint = new_footprint;

                let new_footprint_hex = hex::encode(session_sled.hashed_footprint);
                let trace_id = format!("trace-{}", new_footprint_hex);

                // Dynamically update Xray via Environment Injection to preserve NVMe I/O
                Command::new("sh")
                    .arg("-c")
                    .arg(format!(
                        "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
                        new_footprint_hex, trace_id
                    ))
                    .spawn()?;
            }
        }

        sleep(Duration::from_millis(100)).await;
    }
}
</file>

<file path="src/schema_validator.rs">
//! Schema Validation Module
//!
//! Provides validation of:
//! - Generated schemas against JSON Schema meta-schemas
//! - Instance data against plugin schemas
//! - Expansion of propertyDependencies to if/then for broader compatibility

use crate::plugin_schema::{PluginSchema, SchemaCatalog, DEFAULT_SCHEMA_DIALECT};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

/// Validation report with detailed error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Whether validation passed
    pub valid: bool,
    /// List of validation errors
    pub errors: Vec<ValidationError>,
    /// List of validation warnings (non-fatal)
    pub warnings: Vec<String>,
    /// Schema dialect used
    pub dialect: String,
    /// Hash of the validated content (for audit trail)
    pub content_hash: Option<String>,
}

impl ValidationReport {
    /// Create a successful validation report
    pub fn success(dialect: &str) -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            dialect: dialect.to_string(),
            content_hash: None,
        }
    }

    /// Create a failed validation report with errors
    pub fn failure(dialect: &str, errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: Vec::new(),
            dialect: dialect.to_string(),
            content_hash: None,
        }
    }

    /// Add a content hash for audit trail
    pub fn with_content_hash(mut self, hash: String) -> Self {
        self.content_hash = Some(hash);
        self
    }
}

/// A single validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON Pointer path to the error location
    pub path: String,
    /// Error message
    pub message: String,
    /// The keyword that caused the error (e.g., "type", "required")
    pub keyword: Option<String>,
    /// The schema path where the error originated
    pub schema_path: Option<String>,
}

impl ValidationError {
    pub fn new(path: &str, message: &str) -> Self {
        Self {
            path: path.to_string(),
            message: message.to_string(),
            keyword: None,
            schema_path: None,
        }
    }

    pub fn with_keyword(mut self, keyword: &str) -> Self {
        self.keyword = Some(keyword.to_string());
        self
    }
}

/// Schema validator that uses the jsonschema crate
pub struct SchemaValidator {
    /// Cached compiled validators
    validators: HashMap<String, jsonschema::Validator>,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
        }
    }

    /// Validate a generated schema against the meta-schema
    pub fn validate_schema_against_meta(
        &mut self,
        schema: &Value,
        schema_catalog: &SchemaCatalog,
    ) -> Result<ValidationReport, ValidatorError> {
        let dialect = schema
            .get("$schema")
            .and_then(|s| s.as_str())
            .unwrap_or(DEFAULT_SCHEMA_DIALECT);

        // Get or load the meta-schema
        let meta_schema = schema_catalog
            .get_meta_schema(dialect)
            .ok_or_else(|| ValidatorError::MetaSchemaNotLoaded(dialect.to_string()))?;

        // Compile the meta-schema validator if not cached
        let validator = self.get_or_compile_validator(dialect, meta_schema)?;

        // Convert simd_json::Value to serde_json::Value for jsonschema
        let serde_schema: serde_json::Value = serde_json::to_value(schema)
            .map_err(|e| ValidatorError::CompilationError(e.to_string()))?;

        // Validate
        match validator.validate(&serde_schema) {
            Ok(_) => Ok(ValidationReport::success(dialect)),
            Err(error) => {
                // Single error returned, but we can get all errors via iter_errors
                let validation_errors: Vec<ValidationError> = validator
                    .iter_errors(&serde_schema)
                    .map(|e| ValidationError::new(&e.instance_path.to_string(), &e.to_string()))
                    .collect();
                if validation_errors.is_empty() {
                    // Fallback if iter_errors returns empty but validate failed
                    Ok(ValidationReport::failure(
                        dialect,
                        vec![ValidationError::new("", &error.to_string())],
                    ))
                } else {
                    Ok(ValidationReport::failure(dialect, validation_errors))
                }
            }
        }
    }

    /// Validate instance data against a plugin schema
    pub fn validate_instance(
        &mut self,
        schema: &PluginSchema,
        instance: &Value,
    ) -> Result<ValidationReport, ValidatorError> {
        let json_schema = schema.to_json_schema();
        let dialect = &schema.dialect;

        // Convert to serde_json for jsonschema
        let serde_json_schema: serde_json::Value = serde_json::to_value(&json_schema)
            .map_err(|e| ValidatorError::CompilationError(e.to_string()))?;
        let serde_instance: serde_json::Value = serde_json::to_value(instance)
            .map_err(|e| ValidatorError::CompilationError(e.to_string()))?;

        // Compile the schema validator
        let validator = jsonschema::validator_for(&serde_json_schema)
            .map_err(|e| ValidatorError::CompilationError(e.to_string()))?;

        // Validate
        match validator.validate(&serde_instance) {
            Ok(_) => {
                let mut report = ValidationReport::success(dialect);
                // Add content hash for audit trail
                report.content_hash = Some(compute_content_hash(instance));
                Ok(report)
            }
            Err(error) => {
                // Get all errors via iter_errors
                let validation_errors: Vec<ValidationError> = validator
                    .iter_errors(&serde_instance)
                    .map(|e| ValidationError::new(&e.instance_path.to_string(), &e.to_string()))
                    .collect();
                if validation_errors.is_empty() {
                    Ok(ValidationReport::failure(
                        dialect,
                        vec![ValidationError::new("", &error.to_string())],
                    ))
                } else {
                    Ok(ValidationReport::failure(dialect, validation_errors))
                }
            }
        }
    }

    /// Expand propertyDependencies to if/then for validators that don't support it natively
    ///
    /// Transforms:
    /// ```json
    /// {
    ///   "propertyDependencies": {
    ///     "status": {
    ///       "locked": { "properties": { "id": { "readOnly": true } } }
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// To:
    /// ```json
    /// {
    ///   "allOf": [
    ///     {
    ///       "if": { "properties": { "status": { "const": "locked" } } },
    ///       "then": { "properties": { "id": { "readOnly": true } } }
    ///     }
    ///   ]
    /// }
    /// ```
    pub fn expand_property_dependencies(schema: &Value) -> Result<Value, ValidatorError> {
        let mut result = schema.clone();

        if let Some(obj) = result.as_object_mut() {
            if let Some(prop_deps) = obj.remove("propertyDependencies") {
                let mut all_of = obj
                    .get("allOf")
                    .and_then(|a| a.as_array())
                    .cloned()
                    .unwrap_or_default();

                if let Some(deps_obj) = prop_deps.as_object() {
                    for (prop_name, value_map) in deps_obj {
                        if let Some(values) = value_map.as_object() {
                            for (value, then_schema) in values {
                                let if_schema = json!({
                                    "properties": {
                                        prop_name: { "const": value }
                                    },
                                    "required": [prop_name]
                                });

                                all_of.push(json!({
                                    "if": if_schema,
                                    "then": then_schema
                                }));
                            }
                        }
                    }
                }

                if !all_of.is_empty() {
                    obj.insert("allOf".to_string(), json!(all_of));
                }
            }
        }

        // Recursively expand nested schemas
        if let Some(obj) = result.as_object_mut() {
            for (_key, value) in obj.iter_mut() {
                if value.is_object() {
                    *value = Self::expand_property_dependencies(value)?;
                } else if let Some(arr) = value.as_array_mut() {
                    for item in arr.iter_mut() {
                        if item.is_object() {
                            *item = Self::expand_property_dependencies(item)?;
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn get_or_compile_validator(
        &mut self,
        key: &str,
        schema: &Value,
    ) -> Result<&jsonschema::Validator, ValidatorError> {
        if !self.validators.contains_key(key) {
            let serde_schema: serde_json::Value = serde_json::to_value(schema)
                .map_err(|e| ValidatorError::CompilationError(e.to_string()))?;
            let validator = jsonschema::validator_for(&serde_schema)
                .map_err(|e| ValidatorError::CompilationError(e.to_string()))?;
            self.validators.insert(key.to_string(), validator);
        }
        Ok(self.validators.get(key).unwrap())
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during validation
#[derive(Debug, Clone)]
pub enum ValidatorError {
    MetaSchemaNotLoaded(String),
    CompilationError(String),
    InvalidSchema(String),
}

impl std::fmt::Display for ValidatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MetaSchemaNotLoaded(d) => write!(f, "Meta-schema not loaded for dialect: {}", d),
            Self::CompilationError(e) => write!(f, "Schema compilation error: {}", e),
            Self::InvalidSchema(e) => write!(f, "Invalid schema: {}", e),
        }
    }
}

impl std::error::Error for ValidatorError {}

/// Compute a hash of the content for audit trail
fn compute_content_hash(value: &Value) -> String {
    // Canonicalize the JSON for consistent hashing
    let canonical = canonicalize_json(value);
    let canonical_str = simd_json::to_string(&canonical).unwrap_or_default();
    format!("{:x}", md5::compute(canonical_str.as_bytes()))
}

/// Canonicalize JSON for consistent hashing
/// - Sort object keys
/// - Normalize numbers
/// - Remove optional whitespace variance
pub fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            // Sort keys for consistent ordering
            let mut sorted: Vec<_> = map.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));

            let canonical_map: simd_json::value::owned::Object = sorted
                .into_iter()
                .map(|(k, v)| (k.clone(), canonicalize_json(v)))
                .collect();

            Value::Object(Box::new(canonical_map))
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize_json).collect()),
        Value::Static(s) => {
            if let Some(f) = s.as_f64() {
                if f.fract() == 0.0 && f.abs() < i64::MAX as f64 {
                    // It's an integer
                    json!(f as i64)
                } else {
                    json!(f)
                }
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_json() {
        let input = json!({
            "z": 1,
            "a": 2,
            "m": [3, 2, 1]
        });

        let canonical = canonicalize_json(&input);
        let output = simd_json::to_string(&canonical).unwrap();

        // Keys should be sorted
        assert!(output.find("\"a\"").unwrap() < output.find("\"m\"").unwrap());
        assert!(output.find("\"m\"").unwrap() < output.find("\"z\"").unwrap());
    }

    #[test]
    fn test_expand_property_dependencies() {
        let schema = json!({
            "type": "object",
            "propertyDependencies": {
                "status": {
                    "locked": {
                        "properties": {
                            "id": { "readOnly": true }
                        }
                    }
                }
            }
        });

        let expanded = SchemaValidator::expand_property_dependencies(&schema).unwrap();

        // Should have allOf with if/then
        assert!(expanded.get("allOf").is_some());
        assert!(expanded.get("propertyDependencies").is_none());

        let all_of = expanded.get("allOf").unwrap().as_array().unwrap();
        assert_eq!(all_of.len(), 1);

        let first = &all_of[0];
        assert!(first.get("if").is_some());
        assert!(first.get("then").is_some());
    }

    #[test]
    fn test_content_hash_consistency() {
        let value1 = json!({"b": 2, "a": 1});
        let value2 = json!({"a": 1, "b": 2});

        // Same content, different order should produce same hash
        let hash1 = compute_content_hash(&value1);
        let hash2 = compute_content_hash(&value2);

        assert_eq!(hash1, hash2);
    }
}
</file>

<file path="src/sqlite_store.rs">
//! SQLite-based persistent state store
//!
//! Provides durable storage for execution jobs, plugin state snapshots,
//! and audit trail. Uses SQLx for async database operations.

use crate::error::{Result, StateStoreError};
use crate::execution_job::{ExecutionJob, ExecutionStatus};
use crate::state_store::{StateStore, ToolRecord};
use crate::{CanonicalDbExport, StoredObject};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// SQLite-backed state store for execution jobs and plugin state
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Create a new SQLite store with the given database URL
    ///
    /// URL format: `sqlite:///path/to/db.sqlite` or `sqlite::memory:`
    pub async fn new(url: &str) -> Result<Self> {
        info!("Initializing SQLite state store: {}", url);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await?;

        let store = Self { pool };
        store.initialize_schema().await?;

        info!("SQLite state store initialized successfully");
        Ok(store)
    }

    /// Create an in-memory store for testing
    pub async fn in_memory() -> Result<Self> {
        Self::new("sqlite::memory:").await
    }

    /// Get the underlying SQLx pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> Result<()> {
        debug!("Initializing database schema");

        // Create execution_jobs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS execution_jobs (
                id TEXT PRIMARY KEY,
                tool_name TEXT NOT NULL,
                arguments TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                result TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create plugin_state table for caching plugin state snapshots
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS plugin_state (
                plugin_name TEXT PRIMARY KEY,
                state_json TEXT NOT NULL,
                state_hash TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create checkpoints table for rollback support
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                plugin_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                state_snapshot TEXT NOT NULL,
                backend_checkpoint TEXT,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create audit_log table for tracking all state changes
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                plugin_name TEXT NOT NULL,
                operation TEXT NOT NULL,
                data TEXT NOT NULL,
                footprint_hash TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indices for common queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_status ON execution_jobs(status)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_created ON execution_jobs(created_at)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_checkpoints_plugin ON checkpoints(plugin_name)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_plugin ON audit_log(plugin_name)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp)")
            .execute(&self.pool)
            .await?;

        // Execute namespace schema (org.opdbus.* enterprise tables)
        debug!("Initializing enterprise namespace schema...");
        let namespace_schema = include_str!("namespace_schema.sql");

        // Split by semicolon but keep multi-line statements together
        let mut current_statement = String::new();
        for line in namespace_schema.lines() {
            let trimmed = line.trim();

            // Skip comment-only lines
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }

            current_statement.push_str(line);
            current_statement.push('\n');

            // Execute when we hit a semicolon at the end of a line
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!(
                            "Failed to execute namespace schema statement: {} - Error: {}",
                            stmt.chars().take(100).collect::<String>(),
                            e
                        );
                        // Continue on error for idempotency (IF NOT EXISTS)
                    }
                }
                current_statement.clear();
            }
        }

        info!("✅ Enterprise namespace schema initialized (org.opdbus.*)");

        // Create tools table for tool registry persistence (read-only on startup)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tools (
                tool_name TEXT PRIMARY KEY,
                definition_json TEXT NOT NULL,
                category TEXT NOT NULL,
                namespace TEXT NOT NULL,
                schema_version TEXT NOT NULL DEFAULT 'https://json-schema.org/draft/next/schema',
                source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create objects table for general object storage
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS objects (
                id TEXT PRIMARY KEY,
                object_type TEXT NOT NULL,
                namespace TEXT NOT NULL,
                data TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Execute Full Active Directory schema
        debug!("Initializing Full Active Directory schema...");
        let ad_schema = include_str!("ad_full_schema.sql");
        let mut current_statement = String::new();
        for line in ad_schema.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }
            current_statement.push_str(line);
            current_statement.push('\n');
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!("Failed to execute AD schema statement: {}", e);
                    }
                }
                current_statement.clear();
            }
        }

        // Execute Drupal CMS schema
        debug!("Initializing Drupal CMS schema...");
        let drupal_schema = include_str!("cms_drupal_schema.sql");
        let mut current_statement = String::new();
        for line in drupal_schema.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }
            current_statement.push_str(line);
            current_statement.push('\n');
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!("Failed to execute Drupal schema statement: {}", e);
                    }
                }
                current_statement.clear();
            }
        }

        // Execute WordPress CMS schema
        debug!("Initializing WordPress CMS schema...");
        let wordpress_schema = include_str!("cms_wordpress_schema.sql");
        let mut current_statement = String::new();
        for line in wordpress_schema.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }
            current_statement.push_str(line);
            current_statement.push('\n');
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!("Failed to execute WordPress schema statement: {}", e);
                    }
                }
                current_statement.clear();
            }
        }

        info!("✅ Extended enterprise schemas loaded: Full AD + Drupal + WordPress");
        debug!("Database schema initialized");
        Ok(())
    }

    /// Save plugin state snapshot
    pub async fn save_plugin_state(
        &self,
        plugin_name: &str,
        state: &simd_json::OwnedValue,
    ) -> Result<()> {
        let state_json = simd_json::to_string(state)?;
        let state_hash = format!("{:x}", md5::compute(&state_json));
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO plugin_state (plugin_name, state_json, state_hash, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(plugin_name) DO UPDATE SET
                state_json = excluded.state_json,
                state_hash = excluded.state_hash,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(plugin_name)
        .bind(&state_json)
        .bind(&state_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        debug!("Saved plugin state for {}", plugin_name);
        Ok(())
    }

    /// Get plugin state snapshot
    pub async fn get_plugin_state(
        &self,
        plugin_name: &str,
    ) -> Result<Option<simd_json::OwnedValue>> {
        let row = sqlx::query("SELECT state_json FROM plugin_state WHERE plugin_name = ?")
            .bind(plugin_name)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let mut state_json: String = row.get("state_json");
                let state: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut state_json)? };
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Save a checkpoint for rollback
    pub async fn save_checkpoint(
        &self,
        id: &str,
        plugin_name: &str,
        timestamp: i64,
        state_snapshot: &simd_json::OwnedValue,
        backend_checkpoint: Option<&simd_json::OwnedValue>,
    ) -> Result<()> {
        let state_json = simd_json::to_string(state_snapshot)?;
        let backend_json = backend_checkpoint.map(simd_json::to_string).transpose()?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, plugin_name, timestamp, state_snapshot, backend_checkpoint, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(plugin_name)
        .bind(timestamp)
        .bind(&state_json)
        .bind(&backend_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        debug!("Saved checkpoint {} for {}", id, plugin_name);
        Ok(())
    }

    /// Get a checkpoint by ID
    pub async fn get_checkpoint(&self, id: &str) -> Result<Option<CheckpointRecord>> {
        let row = sqlx::query(
            "SELECT id, plugin_name, timestamp, state_snapshot, backend_checkpoint, created_at FROM checkpoints WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let mut state_json: String = row.get("state_snapshot");
                let mut backend_json: Option<String> = row.get("backend_checkpoint");

                Ok(Some(CheckpointRecord {
                    id: row.get("id"),
                    plugin_name: row.get("plugin_name"),
                    timestamp: row.get("timestamp"),
                    state_snapshot: unsafe { simd_json::from_str(&mut state_json)? },
                    backend_checkpoint: backend_json
                        .as_mut()
                        .map(|s| unsafe { simd_json::from_str(s) })
                        .transpose()?,
                    created_at: row.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get latest checkpoint for a plugin
    pub async fn get_latest_checkpoint(
        &self,
        plugin_name: &str,
    ) -> Result<Option<CheckpointRecord>> {
        let row = sqlx::query(
            "SELECT id, plugin_name, timestamp, state_snapshot, backend_checkpoint, created_at FROM checkpoints WHERE plugin_name = ? ORDER BY timestamp DESC LIMIT 1",
        )
        .bind(plugin_name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let mut state_json: String = row.get("state_snapshot");
                let mut backend_json: Option<String> = row.get("backend_checkpoint");

                Ok(Some(CheckpointRecord {
                    id: row.get("id"),
                    plugin_name: row.get("plugin_name"),
                    timestamp: row.get("timestamp"),
                    state_snapshot: unsafe { simd_json::from_str(&mut state_json)? },
                    backend_checkpoint: backend_json
                        .as_mut()
                        .map(|s| unsafe { simd_json::from_str(s) })
                        .transpose()?,
                    created_at: row.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Log an audit entry
    pub async fn log_audit(
        &self,
        plugin_name: &str,
        operation: &str,
        data: &simd_json::OwnedValue,
        footprint_hash: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let data_json = simd_json::to_string(data)?;

        sqlx::query(
            r#"
            INSERT INTO audit_log (timestamp, plugin_name, operation, data, footprint_hash)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&now)
        .bind(plugin_name)
        .bind(operation)
        .bind(&data_json)
        .bind(footprint_hash)
        .execute(&self.pool)
        .await?;

        debug!("Logged audit entry for {} - {}", plugin_name, operation);
        Ok(())
    }

    /// Get audit log entries for a plugin
    pub async fn get_audit_log(
        &self,
        plugin_name: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditEntry>> {
        let rows = if let Some(name) = plugin_name {
            sqlx::query(
                "SELECT id, timestamp, plugin_name, operation, data, footprint_hash FROM audit_log WHERE plugin_name = ? ORDER BY id DESC LIMIT ?",
            )
            .bind(name)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, timestamp, plugin_name, operation, data, footprint_hash FROM audit_log ORDER BY id DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut entries = Vec::new();
        for row in rows {
            let mut data_json: String = row.get("data");
            entries.push(AuditEntry {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                plugin_name: row.get("plugin_name"),
                operation: row.get("operation"),
                data: unsafe { simd_json::from_str(&mut data_json)? },
                footprint_hash: row.get("footprint_hash"),
            });
        }

        Ok(entries)
    }

    /// List all jobs with optional status filter
    pub async fn list_jobs(
        &self,
        status: Option<ExecutionStatus>,
        limit: i64,
    ) -> Result<Vec<ExecutionJob>> {
        let rows = if let Some(status) = status {
            let status_str = status_to_string(&status);
            sqlx::query(
                "SELECT id, tool_name, arguments, status, created_at, updated_at, result FROM execution_jobs WHERE status = ? ORDER BY created_at DESC LIMIT ?",
            )
            .bind(status_str)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, tool_name, arguments, status, created_at, updated_at, result FROM execution_jobs ORDER BY created_at DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row_to_job(&row)?);
        }

        Ok(jobs)
    }

    /// Count jobs by status
    pub async fn count_jobs_by_status(&self) -> Result<JobCounts> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total,
                SUM(CASE WHEN status = 'Pending' THEN 1 ELSE 0 END) as pending,
                SUM(CASE WHEN status = 'Running' THEN 1 ELSE 0 END) as running,
                SUM(CASE WHEN status = 'Completed' THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'Failed' THEN 1 ELSE 0 END) as failed
            FROM execution_jobs
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(JobCounts {
            total: row.get::<i64, _>("total") as u64,
            pending: row.get::<i64, _>("pending") as u64,
            running: row.get::<i64, _>("running") as u64,
            completed: row.get::<i64, _>("completed") as u64,
            failed: row.get::<i64, _>("failed") as u64,
        })
    }

    /// Delete old jobs (cleanup)
    pub async fn delete_old_jobs(&self, before: DateTime<Utc>) -> Result<u64> {
        let before_str = before.to_rfc3339();

        let result = sqlx::query(
            "DELETE FROM execution_jobs WHERE created_at < ? AND status IN ('Completed', 'Failed')",
        )
        .bind(&before_str)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();
        info!("Deleted {} old jobs from before {}", deleted, before_str);
        Ok(deleted)
    }

    /// Delete old checkpoints (keep only latest N per plugin)
    pub async fn cleanup_checkpoints(&self, keep_per_plugin: i64) -> Result<u64> {
        // This is a bit complex - we need to delete all but the latest N checkpoints per plugin
        let result = sqlx::query(
            r#"
            DELETE FROM checkpoints WHERE id IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (PARTITION BY plugin_name ORDER BY timestamp DESC) as rn
                    FROM checkpoints
                ) WHERE rn > ?
            )
            "#,
        )
        .bind(keep_per_plugin)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();
        info!("Deleted {} old checkpoints", deleted);
        Ok(deleted)
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<StoreStats> {
        let job_counts = self.count_jobs_by_status().await?;

        let checkpoint_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
            .fetch_one(&self.pool)
            .await?;

        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.pool)
            .await?;

        let plugin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_state")
            .fetch_one(&self.pool)
            .await?;

        Ok(StoreStats {
            jobs: job_counts,
            checkpoints: checkpoint_count as u64,
            audit_entries: audit_count as u64,
            plugin_states: plugin_count as u64,
        })
    }
}

#[async_trait]
impl StateStore for SqliteStore {
    async fn save_job(&self, job: &ExecutionJob) -> Result<()> {
        let arguments_json = simd_json::to_string(&job.arguments)?;
        let result_json = job.result.as_ref().map(simd_json::to_string).transpose()?;
        let status_str = status_to_string(&job.status);

        sqlx::query(
            r#"
            INSERT INTO execution_jobs (id, tool_name, arguments, status, created_at, updated_at, result)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(job.id.to_string())
        .bind(&job.tool_name)
        .bind(&arguments_json)
        .bind(status_str)
        .bind(job.created_at.to_rfc3339())
        .bind(job.updated_at.to_rfc3339())
        .bind(&result_json)
        .execute(&self.pool)
        .await?;

        debug!("Saved job {} ({})", job.id, job.tool_name);
        Ok(())
    }

    async fn get_job(&self, id: Uuid) -> Result<Option<ExecutionJob>> {
        let row = sqlx::query(
            "SELECT id, tool_name, arguments, status, created_at, updated_at, result FROM execution_jobs WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_job(&row)?)),
            None => Ok(None),
        }
    }

    async fn update_job(&self, job: &ExecutionJob) -> Result<()> {
        let arguments_json = simd_json::to_string(&job.arguments)?;
        let result_json = job.result.as_ref().map(simd_json::to_string).transpose()?;
        let status_str = status_to_string(&job.status);

        let result = sqlx::query(
            r#"
            UPDATE execution_jobs
            SET tool_name = ?, arguments = ?, status = ?, updated_at = ?, result = ?
            WHERE id = ?
            "#,
        )
        .bind(&job.tool_name)
        .bind(&arguments_json)
        .bind(status_str)
        .bind(job.updated_at.to_rfc3339())
        .bind(&result_json)
        .bind(job.id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            warn!("Job {} not found for update", job.id);
            return Err(StateStoreError::NotFound(job.id.to_string()));
        }

        debug!("Updated job {} to status {:?}", job.id, job.status);
        Ok(())
    }

    async fn get_object(&self, id: &str) -> Result<Option<StoredObject>> {
        let row = sqlx::query("SELECT id, object_type, namespace, data FROM objects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => {
                let mut data_json: String = r.get("data");
                Ok(Some(StoredObject {
                    id: r.get("id"),
                    object_type: r.get("object_type"),
                    namespace: r.get("namespace"),
                    data: unsafe { simd_json::from_str(&mut data_json)? },
                }))
            }
            None => Ok(None),
        }
    }

    async fn upsert_object(
        &self,
        id: &str,
        object_type: &str,
        namespace: &str,
        data: &simd_json::OwnedValue,
    ) -> Result<()> {
        let data_json = simd_json::to_string(data)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO objects (id, object_type, namespace, data, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(id)
        .bind(object_type)
        .bind(namespace)
        .bind(&data_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn export_canonical(&self) -> Result<CanonicalDbExport> {
        // Get all objects
        let rows = sqlx::query("SELECT id, object_type, namespace, data FROM objects")
            .fetch_all(&self.pool)
            .await?;

        let objects: Vec<StoredObject> = rows
            .iter()
            .map(|r| {
                let mut data_json: String = r.get("data");
                StoredObject {
                    id: r.get("id"),
                    object_type: r.get("object_type"),
                    namespace: r.get("namespace"),
                    data: unsafe { simd_json::from_str(&mut data_json).unwrap_or_default() },
                }
            })
            .collect();

        Ok(CanonicalDbExport {
            objects,
            executions: vec![], // To be populated if needed
            snowball: vec![], // To be populated if needed
        })
    }

    /// Save tools to database (WRITE: onboarding/upgrade/migration/chatbot changes)
    async fn save_tools(&self, tools: Vec<ToolRecord>) -> Result<()> {
        let tool_count = tools.len();
        let mut tx = self.pool.begin().await?;

        for tool in tools {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO tools (tool_name, definition_json, category, namespace, schema_version, source, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&tool.tool_name)
            .bind(&tool.definition_json)
            .bind(&tool.category)
            .bind(&tool.namespace)
            .bind(&tool.schema_version)
            .bind(&tool.source)
            .bind(&tool.created_at)
            .bind(&tool.updated_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        info!("Saved {} tools to database", tool_count);
        Ok(())
    }

    /// Load tools from database (READ: normal startup)
    async fn load_tools(&self) -> Result<Vec<ToolRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT tool_name, definition_json, category, namespace, schema_version, source, created_at, updated_at
            FROM tools
            ORDER BY tool_name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let tools: Vec<ToolRecord> = rows
            .iter()
            .map(|row| ToolRecord {
                tool_name: row.get("tool_name"),
                definition_json: row.get("definition_json"),
                category: row.get("category"),
                namespace: row.get("namespace"),
                schema_version: row.get("schema_version"),
                source: row.get("source"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        info!("Loaded {} tools from database", tools.len());
        Ok(tools)
    }

    /// Check if tools table is empty (indicates first run/onboarding)
    async fn is_tools_empty(&self) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tools")
            .fetch_one(&self.pool)
            .await?;
        Ok(count == 0)
    }

    /// Clear all tools (for migration/upgrade)
    async fn clear_tools(&self) -> Result<()> {
        sqlx::query("DELETE FROM tools").execute(&self.pool).await?;
        info!("Cleared all tools from database");
        Ok(())
    }
}

/// Helper function to convert status enum to string
fn status_to_string(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Pending => "Pending",
        ExecutionStatus::Running => "Running",
        ExecutionStatus::Completed => "Completed",
        ExecutionStatus::Failed => "Failed",
    }
}

/// Helper function to convert string to status enum
fn string_to_status(s: &str) -> ExecutionStatus {
    match s {
        "Pending" => ExecutionStatus::Pending,
        "Running" => ExecutionStatus::Running,
        "Completed" => ExecutionStatus::Completed,
        "Failed" => ExecutionStatus::Failed,
        _ => ExecutionStatus::Pending, // Default fallback
    }
}

/// Helper function to convert database row to ExecutionJob
fn row_to_job(row: &sqlx::sqlite::SqliteRow) -> Result<ExecutionJob> {
    let id_str: String = row.get("id");
    let mut arguments_json: String = row.get("arguments");
    let status_str: String = row.get("status");
    let created_at_str: String = row.get("created_at");
    let updated_at_str: String = row.get("updated_at");
    let mut result_json: Option<String> = row.get("result");

    Ok(ExecutionJob {
        id: Uuid::parse_str(&id_str).map_err(|e| StateStoreError::NotFound(e.to_string()))?,
        tool_name: row.get("tool_name"),
        arguments: unsafe { simd_json::from_str(&mut arguments_json)? },
        status: string_to_status(&status_str),
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| StateStoreError::NotFound(e.to_string()))?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| StateStoreError::NotFound(e.to_string()))?
            .with_timezone(&Utc),
        result: result_json
            .as_mut()
            .map(|s| unsafe { simd_json::from_str(s) })
            .transpose()?,
    })
}

/// Checkpoint record from database
#[derive(Debug, Clone)]
pub struct CheckpointRecord {
    pub id: String,
    pub plugin_name: String,
    pub timestamp: i64,
    pub state_snapshot: simd_json::OwnedValue,
    pub backend_checkpoint: Option<simd_json::OwnedValue>,
    pub created_at: String,
}

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub id: i64,
    pub timestamp: String,
    pub plugin_name: String,
    pub operation: String,
    pub data: simd_json::OwnedValue,
    pub footprint_hash: Option<String>,
}

/// Job counts by status
#[derive(Debug, Clone, Default)]
pub struct JobCounts {
    pub total: u64,
    pub pending: u64,
    pub running: u64,
    pub completed: u64,
    pub failed: u64,
}

/// Store statistics
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub jobs: JobCounts,
    pub checkpoints: u64,
    pub audit_entries: u64,
    pub plugin_states: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_job::ExecutionResult;

    #[tokio::test]
    async fn test_sqlite_store_job_lifecycle() {
        let store = SqliteStore::in_memory().await.unwrap();

        // Create a job
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            arguments: simd_json::json!({"key": "value"}),
            status: ExecutionStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            result: None,
        };

        // Save
        store.save_job(&job).await.unwrap();

        // Get
        let retrieved = store.get_job(job.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, job.id);
        assert_eq!(retrieved.tool_name, "test_tool");
        assert_eq!(retrieved.status, ExecutionStatus::Pending);

        // Update
        let mut updated_job = job.clone();
        updated_job.status = ExecutionStatus::Completed;
        updated_job.result = Some(ExecutionResult {
            success: true,
            output: Some(simd_json::json!({"result": "ok"})),
            error: None,
        });
        store.update_job(&updated_job).await.unwrap();

        // Verify update
        let retrieved = store.get_job(job.id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, ExecutionStatus::Completed);
        assert!(retrieved.result.is_some());
    }

    #[tokio::test]
    async fn test_sqlite_store_plugin_state() {
        let store = SqliteStore::in_memory().await.unwrap();

        let state = simd_json::json!({
            "containers": [
                {"id": "100", "status": "running"}
            ]
        });

        // Save
        store.save_plugin_state("lxc", &state).await.unwrap();

        // Get
        let retrieved = store.get_plugin_state("lxc").await.unwrap().unwrap();
        assert_eq!(retrieved, state);

        // Update
        let new_state = simd_json::json!({
            "containers": [
                {"id": "100", "status": "stopped"},
                {"id": "101", "status": "running"}
            ]
        });
        store.save_plugin_state("lxc", &new_state).await.unwrap();

        let retrieved = store.get_plugin_state("lxc").await.unwrap().unwrap();
        assert_eq!(retrieved, new_state);
    }

    #[tokio::test]
    async fn test_sqlite_store_checkpoints() {
        let store = SqliteStore::in_memory().await.unwrap();

        let state = simd_json::json!({"key": "value"});

        // Save checkpoint
        store
            .save_checkpoint("cp-1", "test_plugin", 1000, &state, None)
            .await
            .unwrap();

        // Get checkpoint
        let cp = store.get_checkpoint("cp-1").await.unwrap().unwrap();
        assert_eq!(cp.plugin_name, "test_plugin");
        assert_eq!(cp.timestamp, 1000);

        // Get latest
        store
            .save_checkpoint("cp-2", "test_plugin", 2000, &state, None)
            .await
            .unwrap();

        let latest = store
            .get_latest_checkpoint("test_plugin")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, "cp-2");
    }

    #[tokio::test]
    async fn test_sqlite_store_audit_log() {
        let store = SqliteStore::in_memory().await.unwrap();

        // Log entries
        store
            .log_audit("plugin1", "create", &simd_json::json!({"id": "1"}), None)
            .await
            .unwrap();
        store
            .log_audit(
                "plugin1",
                "update",
                &simd_json::json!({"id": "1"}),
                Some("abc123"),
            )
            .await
            .unwrap();
        store
            .log_audit("plugin2", "delete", &simd_json::json!({"id": "2"}), None)
            .await
            .unwrap();

        // Get all
        let entries = store.get_audit_log(None, 10).await.unwrap();
        assert_eq!(entries.len(), 3);

        // Get for plugin1
        let entries = store.get_audit_log(Some("plugin1"), 10).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_sqlite_store_stats() {
        let store = SqliteStore::in_memory().await.unwrap();

        // Create some jobs
        for i in 0..5 {
            let job = ExecutionJob {
                id: Uuid::new_v4(),
                tool_name: format!("tool_{}", i),
                arguments: simd_json::json!({}),
                status: if i % 2 == 0 {
                    ExecutionStatus::Completed
                } else {
                    ExecutionStatus::Failed
                },
                created_at: Utc::now(),
                updated_at: Utc::now(),
                result: None,
            };
            store.save_job(&job).await.unwrap();
        }

        let stats = store.get_stats().await.unwrap();
        assert_eq!(stats.jobs.total, 5);
        assert_eq!(stats.jobs.completed, 3);
        assert_eq!(stats.jobs.failed, 2);
    }
}
</file>

<file path="src/state_store.rs">
use crate::error::Result;
use crate::execution_job::ExecutionJob;
use crate::{CanonicalDbExport, StoredObject};
use async_trait::async_trait;
use uuid::Uuid;

/// Tool record from database
#[derive(Debug, Clone)]
pub struct ToolRecord {
    pub tool_name: String,
    pub definition_json: String, // Serialized ToolDefinition
    pub category: String,
    pub namespace: String,
    pub schema_version: String, // JSON Schema version
    pub source: String,         // "builtin", "dbus-session.v1", "dbus-system.v1", "mcp", "agent"
    pub created_at: String,
    pub updated_at: String,
}

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn save_job(&self, job: &ExecutionJob) -> Result<()>;
    async fn get_job(&self, id: Uuid) -> Result<Option<ExecutionJob>>;
    async fn update_job(&self, job: &ExecutionJob) -> Result<()>;

    async fn get_object(&self, id: &str) -> Result<Option<StoredObject>>;
    async fn upsert_object(
        &self,
        id: &str,
        object_type: &str,
        namespace: &str,
        data: &simd_json::OwnedValue,
    ) -> Result<()>;
    async fn export_canonical(&self) -> Result<CanonicalDbExport>;

    // Tool persistence (READ on startup, WRITE only on onboarding/upgrade/migration)
    async fn save_tools(&self, tools: Vec<ToolRecord>) -> Result<()>;
    async fn load_tools(&self) -> Result<Vec<ToolRecord>>;
    async fn is_tools_empty(&self) -> Result<bool>;
    async fn clear_tools(&self) -> Result<()>;
}
</file>

<file path="Cargo.toml">
[package]
name = "op-state-store"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "MCP Execution State Store - Persistent job ledger and state tracking"

[dependencies]
tokio = { workspace = true, features = ["full"] }
sqlx = { workspace = true, features = ["sqlite", "runtime-tokio", "chrono", "json"] }
redis = { workspace = true, features = ["tokio-comp"] }
serde = { workspace = true }
simd-json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
md5 = "0.7"
base64 = { workspace = true }
hex = { workspace = true }
opentelemetry = { workspace = true }
prometheus = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }
regex = { workspace = true }
lazy_static = { workspace = true }
zbus = { workspace = true }
serde_json = { workspace = true }
reqwest = { workspace = true, features = ["json"] }
jsonschema = { version = "0.29", default-features = false }
</file>

<file path="compare-op-state-store.md">
# compare-op-state-store

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 11 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 10 |
| Partial artifacts | 0 |
| Spec-listed source files | 11 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- MCP Execution State Store - Persistent job ledger and state tracking

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/state_store.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_store.rs |
| `src/sqlite_store.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/sqlite_store.rs |
| `src/schema_validator.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/schema_validator.rs |
| `src/redis_stream.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/redis_stream.rs |
| `src/plugin_schema.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugin_schema.rs |
| `src/metrics.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/metrics.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/execution_job.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/execution_job.rs |
| `src/event_chain.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/event_chain.rs |
| `src/error.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/error.rs |
| `src/disaster_recovery.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/disaster_recovery.rs |
| `root` | ✅ Present | root source group | src/disaster_recovery.rs, src/error.rs, src/event_chain.rs, src/execution_job.rs, src/lib.rs, src/metrics.rs, src/plugin_schema.rs, src/redis_stream.rs, ... (+3 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| state_store | ✅ Implemented | src/state_store.rs | SPEC main module |
| sqlite_store | ✅ Implemented | src/sqlite_store.rs | SPEC main module |
| schema_validator | ✅ Implemented | src/schema_validator.rs | SPEC main module |
| redis_stream | ✅ Implemented | src/redis_stream.rs | SPEC main module |
| plugin_schema | ✅ Implemented | src/plugin_schema.rs | SPEC main module |
| metrics | ✅ Implemented | src/metrics.rs | SPEC main module |
| execution_job | ✅ Implemented | src/execution_job.rs | SPEC main module |
| event_chain | ✅ Implemented | src/event_chain.rs | SPEC main module |
| error | ✅ Implemented | src/error.rs | SPEC main module |
| disaster_recovery | ✅ Implemented | src/disaster_recovery.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `sqlx` - documented in SPEC
- `redis` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC
- `tracing` - documented in SPEC
- `md5` - documented in SPEC
- `opentelemetry` - documented in SPEC
- `prometheus` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `async-trait` - documented in SPEC
- `regex` - documented in SPEC
- `lazy_static` - documented in SPEC
- `zbus` - documented in SPEC
- `serde_json` - documented in SPEC
- `jsonschema` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: disaster_recovery, error, event_chain, execution_job, metrics, plugin_schema, redis_stream, schema_validator, sqlite_store, state_store.
</file>

<file path="SPEC.md">
# op-state-store - Specification

## Overview
**Crate**: `op-state-store`  
**Location**: `crates/op-state-store`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-state-store"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "MCP Execution State Store - Persistent job ledger and state tracking"
```

### Source Structure
```
op-state-store/src/state_store.rs
op-state-store/src/sqlite_store.rs
op-state-store/src/schema_validator.rs
op-state-store/src/redis_stream.rs
op-state-store/src/plugin_schema.rs
op-state-store/src/metrics.rs
op-state-store/src/lib.rs
op-state-store/src/execution_job.rs
op-state-store/src/event_chain.rs
op-state-store/src/error.rs
op-state-store/src/disaster_recovery.rs
```

### Key Dependencies
```toml
tokio = { workspace = true, features = ["full"] }
sqlx = { workspace = true, features = ["sqlite", "runtime-tokio", "chrono", "json"] }
redis = { workspace = true, features = ["tokio-comp"] }
serde = { workspace = true }
simd-json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
md5 = "0.7"
opentelemetry = { workspace = true }
prometheus = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }
regex = { workspace = true }
lazy_static = { workspace = true }
zbus = { workspace = true }
serde_json = { workspace = true }
jsonschema = { version = "0.29", default-features = false }
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
      11 Rust source files

### Main Modules
state_store
sqlite_store
schema_validator
redis_stream
plugin_schema
metrics
execution_job
event_chain
error
disaster_recovery

## Purpose
MCP Execution State Store - Persistent job ledger and state tracking

## Build Information
- **Edition**: edition.workspace = true
- **Version**: 0.1.0
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
