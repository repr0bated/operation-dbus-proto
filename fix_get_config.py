import re

with open('crates/op-web/src/handlers/privacy.rs', 'r') as f:
    content = f.read()

new_get_config = """pub async fn get_config(
    headers: axum::http::HeaderMap,
    axum::extract::Extension(state): axum::extract::Extension<std::sync::Arc<crate::AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> (axum::http::StatusCode, axum::Json<VerifyResponse>) {
    let auth_token = crate::middleware::security::extract_auth_token(&headers);
    let mut is_authorized = false;
    
    if let Some(token) = auth_token {
        if crate::middleware::security::check_bypass_api_key(&headers).is_some() {
            is_authorized = true;
        } else if let Some(user) = state.user_store.get_user(&user_id).await {
            if let Some(creds) = &user.api_credentials {
                if creds.token == token {
                    is_authorized = true;
                }
            }
        }
    }

    if !is_authorized {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(VerifyResponse {
                success: false,
                user_id: None,
                config: None,
                qr_code: None,
                message: "Unauthorized".to_string(),
            }),
        );
    }

    match state.user_store.get_user(&user_id).await {
"""

content = re.sub(r'pub async fn get_config\(\n\s+Extension\(state\): Extension<Arc<AppState>>,\n\s+Path\(user_id\): Path<String>,\n\) -> \(StatusCode, Json<VerifyResponse>\) {\n\s+match state\.user_store\.get_user\(&user_id\)\.await {', new_get_config, content)

with open('crates/op-web/src/handlers/privacy.rs', 'w') as f:
    f.write(content)
