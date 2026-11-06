//! Servidor MCP local con las mismas 9 tools que Granola:
//! query_granola_meetings, list_meetings, list_meeting_folders, get_meetings,
//! get_meeting_attachment, get_meeting_transcript, get_account_info,
//! switch_workspace, sign_out.

pub const TOOLS: &[&str] = &[
    "query_granola_meetings",
    "list_meetings",
    "list_meeting_folders",
    "get_meetings",
    "get_meeting_attachment",
    "get_meeting_transcript",
    "get_account_info",
    "switch_workspace",
    "sign_out",
];
