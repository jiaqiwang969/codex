mod pkce;
mod server;

pub use server::{LoginServer, ServerOptions, ShutdownHandle, run_login_server};

// Re-export commonly used auth types and helpers from codex-core for compatibility
pub use codex_core::{
    AuthManager, CodexAuth,
    auth::{
        AuthDotJson, CLIENT_ID, OPENAI_API_KEY_ENV_VAR, get_auth_file, login_with_api_key, logout,
        try_read_auth_json, write_auth_json,
    },
    token_data::TokenData,
};
pub use codex_protocol::mcp_protocol::AuthMode;
