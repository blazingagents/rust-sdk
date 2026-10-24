use blazing_agents::agent_tools::{
    AGENT_TOOL_GROUPS, AGENT_TOOLS, AgentToolFileAccessTag, AgentToolGroupTag,
};
use serde_json::{Value, json};

fn spec_catalog() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/openapi.json"))
        .expect("the vendored spec is readable");
    let spec: Value = serde_json::from_str(&text).expect("the vendored spec is JSON");
    spec["x-agent-tools"].clone()
}

#[test]
fn catalog_matches_the_vendored_spec() {
    let catalog = spec_catalog();
    let groups: Vec<Value> = AGENT_TOOL_GROUPS
        .iter()
        .map(|group| {
            json!({ "tag": group.tag, "name": group.name, "description": group.description })
        })
        .collect();
    assert_eq!(Value::from(groups), catalog["groups"]);
    let tools: Vec<Value> = AGENT_TOOLS
        .iter()
        .map(|tool| {
            let mut tags = vec![json!(tool.group)];
            tags.extend(tool.file_access.as_ref().map(|access| json!(access)));
            json!({
                "name": tool.name,
                "description": tool.description,
                "requiresWorkspace": tool.requires_workspace,
                "tags": tags,
            })
        })
        .collect();
    assert_eq!(Value::from(tools), catalog["tools"]);
    let tags = [
        json!(AgentToolGroupTag::Files),
        json!(AgentToolGroupTag::Planning),
        json!(AgentToolGroupTag::Memory),
        json!(AgentToolGroupTag::Web),
        json!(AgentToolFileAccessTag::FileRead),
        json!(AgentToolFileAccessTag::FileWrite),
        json!(AgentToolFileAccessTag::Execute),
    ];
    assert_eq!(json!(tags), catalog["tags"]);
}

#[test]
fn read_only_file_tools_are_the_file_read_tagged_ones() {
    let read_only: Vec<&str> = AGENT_TOOLS
        .iter()
        .filter(|tool| tool.file_access == Some(AgentToolFileAccessTag::FileRead))
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(read_only, ["read", "grep", "glob"]);
}

#[test]
fn web_tools_need_no_workspace() {
    let web: Vec<_> = AGENT_TOOLS
        .iter()
        .filter(|tool| tool.group == AgentToolGroupTag::Web)
        .collect();
    let names: Vec<&str> = web.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["web_search", "web_fetch"]);
    assert!(web.iter().all(|tool| !tool.requires_workspace));
}
