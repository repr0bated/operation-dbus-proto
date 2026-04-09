use sqlx::SqlitePool;
use anyhow::Result;

pub async fn create_pre_canned_schemas(pool: &SqlitePool) -> Result<()> {
    // Active Directory
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO plugins (name, service_name, base_object)
        VALUES (?, ?, ?)
        "#,
    )
    .bind("active-directory")
    .bind("org.opdbus.activedirectory.v1")
    .bind(r#"{
        "type": "DirectoryEntry",
        "description": "Active Directory integration",
        "replaces": [],
        "common_properties": {
            "id": {"type": "uuid", "required": true},
            "created_at": {"type": "timestamp", "required": true},
            "updated_at": {"type": "timestamp", "required": true},
            "object_type": {"type": "string", "required": true}
        },
        "object_types": {
            "User": {
                "description": "Active Directory user account",
                "base_path": "/org/opdbus/activedirectory/users",
                "interface": "org.opdbus.activedirectory.v1.User"
            },
            "Group": {
                "description": "Active Directory group",
                "base_path": "/org/opdbus/activedirectory/groups",
                "interface": "org.opdbus.activedirectory.v1.Group"
            },
            "Computer": {
                "description": "Active Directory computer account",
                "base_path": "/org/opdbus/activedirectory/computers",
                "interface": "org.opdbus.activedirectory.v1.Computer"
            }
        }
    }"#)
    .execute(pool)
    .await?;

    // Slack
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO plugins (name, service_name, base_object)
        VALUES (?, ?, ?)
        "#,
    )
    .bind("slack")
    .bind("org.opdbus.slack.v1")
    .bind(r#"{
        "type": "SlackObject",
        "description": "Slack integration",
        "replaces": [],
        "common_properties": {
            "id": {"type": "string", "required": true},
            "created_at": {"type": "timestamp", "required": true},
            "updated_at": {"type": "timestamp", "required": true},
            "object_type": {"type": "string", "required": true}
        },
        "object_types": {
            "User": {
                "description": "Slack user account",
                "base_path": "/org/opdbus/slack/users",
                "interface": "org.opdbus.slack.v1.User"
            },
            "Channel": {
                "description": "Slack channel",
                "base_path": "/org/opdbus/slack/channels",
                "interface": "org.opdbus.slack.v1.Channel"
            },
            "Message": {
                "description": "Slack message",
                "base_path": "/org/opdbus/slack/messages",
                "interface": "org.opdbus.slack.v1.Message"
            }
        }
    }"#)
    .execute(pool)
    .await?;

    Ok(())
}
