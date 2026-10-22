//! Every property of every JSON response schema in `spec/openapi.json` maps to
//! a field of the SDK's model, unless `tests/spec_field_omissions.txt` names it
//! with a reason.
//!
//! For each operation, the test builds a response holding every property the
//! spec declares (one response per `oneOf`/`anyOf` branch), sends it through
//! the SDK method from the mock server, and expects it to decode. Then it
//! replaces one property at a time with a value of a type the spec does not
//! allow. A modeled field refuses that value, so the call fails; a property no
//! field reads is ignored, so the call still succeeds, and the property is
//! reported as missing. Going through the public method covers envelopes,
//! custom decoders and the credential check exactly as a caller sees them.

mod support;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use blazing_agents::call::{Call, Request};
use blazing_agents::chat_connections::{ChatCredentials, NewChatConnection, TelegramCredentials};
use blazing_agents::files::Upload;
use blazing_agents::mcp_connections::{McpReconnect, NewMcpConnection};
use blazing_agents::merchant::{MerchantEnvironment, MerchantProvider};
use blazing_agents::messages::UserMessage;
use blazing_agents::providers::ProviderType;
use blazing_agents::skills::ArchiveType;
use blazing_agents::{Client, Error};
use serde_json::{Value, json};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

type Sent = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;
type Method = fn(&Client) -> Sent;

fn send<R: Request>(call: Call<R>) -> Sent {
    Box::pin(async move { call.await.map(drop) })
}

/// The SDK method that sends each operation with a JSON response.
fn methods() -> Vec<(&'static str, Method)> {
    vec![
        ("getTenantSettings", |c| send(c.tenant().get())),
        ("updateTenantSettings", |c| send(c.tenant().update())),
        ("getTenantSpendingLimit", |c| {
            send(c.tenant().get_spending_limit())
        }),
        ("updateTenantSpendingLimit", |c| {
            send(c.tenant().update_spending_limit(None))
        }),
        ("createChatConnection", |c| {
            send(c.chat_connections().create(NewChatConnection::telegram(
                "ag_1",
                "Bot",
                TelegramCredentials::new("7123456789:AAFtoken"),
            )))
        }),
        ("listChatConnections", |c| send(c.chat_connections().list())),
        ("getChatConnection", |c| {
            send(c.chat_connections().get("cc_1"))
        }),
        ("renameChatConnection", |c| {
            send(c.chat_connections().update("cc_1"))
        }),
        ("rotateChatCredentials", |c| {
            send(c.chat_connections().rotate_credentials(
                "cc_1",
                ChatCredentials::Telegram(TelegramCredentials::new("7123456789:AAFtoken")),
            ))
        }),
        ("checkChatHealth", |c| {
            send(c.chat_connections().check_health("cc_1"))
        }),
        ("disableChatConnection", |c| {
            send(c.chat_connections().disable("cc_1"))
        }),
        ("enableChatConnection", |c| {
            send(c.chat_connections().enable("cc_1"))
        }),
        ("getAgentSpendingLimit", |c| {
            send(c.agents().get_spending_limit("ag_1"))
        }),
        ("updateAgentSpendingLimit", |c| {
            send(c.agents().update_spending_limit("ag_1", None))
        }),
        ("createProvider", |c| {
            send(
                c.providers()
                    .create("OpenAI", ProviderType::OpenAi, "sk-test-0123456789"),
            )
        }),
        ("listProviders", |c| send(c.providers().list())),
        ("listProviderModels", |c| {
            send(c.providers().list_models("prv_1"))
        }),
        ("listThinkingLevels", |c| {
            send(c.providers().get_thinking_levels("prv_1", "gpt-5"))
        }),
        ("getProvider", |c| send(c.providers().get("prv_1"))),
        ("updateProvider", |c| {
            send(c.providers().update("prv_1", "OpenAI"))
        }),
        ("createWorkspace", |c| send(c.workspaces().create())),
        ("listWorkspaces", |c| send(c.workspaces().list())),
        ("getWorkspace", |c| send(c.workspaces().get("ws_1"))),
        ("updateWorkspace", |c| send(c.workspaces().update("ws_1"))),
        ("createMcpConnection", |c| {
            send(c.mcp_connections().create(NewMcpConnection::None {
                name: "Docs".to_owned(),
                url: "https://mcp.example.com/mcp".to_owned(),
            }))
        }),
        ("listMcpConnections", |c| send(c.mcp_connections().list())),
        ("testMcpConnection", |c| {
            send(c.mcp_connections().test("mcp_1"))
        }),
        ("reconnectMcpConnection", |c| {
            send(c.mcp_connections().reconnect(
                "mcp_1",
                McpReconnect::None {
                    url: "https://mcp.example.com/mcp".to_owned(),
                },
            ))
        }),
        ("getMcpConnection", |c| {
            send(c.mcp_connections().get("mcp_1"))
        }),
        ("updateMcpConnection", |c| {
            send(c.mcp_connections().update("mcp_1", "Docs"))
        }),
        ("createAgent", |c| send(c.agents().create("Support"))),
        ("listAgents", |c| send(c.agents().list())),
        ("listAgentMcpAttachments", |c| {
            send(c.agents().list_mcp_attachments("ag_1"))
        }),
        ("updateAgentMcpAttachment", |c| {
            send(
                c.agents()
                    .update_mcp_attachment("ag_1", "mcp_1")
                    .forward_user_id(true),
            )
        }),
        ("getAgent", |c| send(c.agents().get("ag_1"))),
        ("updateAgent", |c| send(c.agents().update("ag_1"))),
        ("disableAgent", |c| send(c.agents().disable("ag_1"))),
        ("enableAgent", |c| send(c.agents().enable("ag_1"))),
        ("uploadAgentAvatar", |c| {
            send(
                c.agents()
                    .upload_avatar("ag_1", Upload::bytes(b"png".as_slice()).file_name("a.png")),
            )
        }),
        ("deleteAgentAvatar", |c| {
            send(c.agents().remove_avatar("ag_1"))
        }),
        ("createSkill", |c| {
            send(c.agent("ag_1").skills().create("SKILL.md", "# Hi"))
        }),
        ("listSkills", |c| send(c.agent("ag_1").skills().list())),
        ("uploadSkill", |c| {
            send(c.agent("ag_1").skills().upload(
                ArchiveType::Zip,
                Upload::bytes(b"zip".as_slice()).file_name("s.zip"),
            ))
        }),
        ("getSkill", |c| {
            send(c.agent("ag_1").skills().get("skill_1"))
        }),
        ("putSkillFile", |c| {
            send(c.agent("ag_1").skills().replace_file(
                "skill_1",
                "SKILL.md",
                Upload::bytes(b"# Hi".as_slice()),
            ))
        }),
        ("deleteSkillFile", |c| {
            send(c.agent("ag_1").skills().delete_file("skill_1", "notes.md"))
        }),
        ("copySkill", |c| {
            send(c.agent("ag_1").skills().copy("skill_1", ["ag_2"]))
        }),
        ("listSessions", |c| send(c.sessions().list("ag_1"))),
        ("listLatestSessions", |c| send(c.sessions().list_latest())),
        ("getSession", |c| send(c.sessions().get("ag_1", "ss_1"))),
        ("forkSession", |c| {
            send(c.sessions().fork("ag_1", "ss_1", "m1", "fork-1"))
        }),
        ("listSessionMessages", |c| {
            send(c.sessions().messages("ag_1", "ss_1"))
        }),
        ("listToolApprovals", |c| {
            send(c.sessions().tool_approvals("ag_1", "ss_1"))
        }),
        ("submitSessionInput", |c| {
            send(
                c.sessions()
                    .submit_input("ag_1", "ss_1", "req-1", UserMessage::text("hi")),
            )
        }),
        ("listSessionInputs", |c| {
            send(c.sessions().inputs("ag_1", "ss_1"))
        }),
        ("stopSessionTurn", |c| {
            send(c.sessions().stop("ag_1", "ss_1", "turn_1"))
        }),
        ("listArtifacts", |c| send(c.artifacts().list())),
        ("createArtifactDownloadUrl", |c| {
            send(c.artifacts().create_download_url("at_1"))
        }),
        ("getArtifact", |c| send(c.artifacts().get("at_1"))),
        ("createMemory", |c| {
            send(c.memories().create("ag_1", "note"))
        }),
        ("listMemories", |c| send(c.memories().list("ag_1"))),
        ("getMemory", |c| send(c.memories().get("ag_1", "mem_1"))),
        ("updateMemory", |c| {
            send(c.memories().update("ag_1", "mem_1", "note"))
        }),
        ("createPrompt", |c| send(c.prompts().create("Reply", "Hi"))),
        ("listPrompts", |c| send(c.prompts().list())),
        ("getPrompt", |c| send(c.prompts().get("prompt_1"))),
        ("updatePrompt", |c| send(c.prompts().update("prompt_1"))),
        ("getMerchantConnection", |c| {
            send(c.merchant_connection().get())
        }),
        ("createMerchantConnection", |c| {
            send(c.merchant_connection().create(
                MerchantProvider::Polar,
                MerchantEnvironment::Sandbox,
                "polar_oat_0123456789",
            ))
        }),
        ("updateMerchantConnection", |c| {
            send(c.merchant_connection().update())
        }),
        ("listMerchantBindings", |c| {
            send(c.merchant_bindings().list())
        }),
        ("setMerchantBinding", |c| {
            send(c.merchant_bindings().put("user-1", "cus_1"))
        }),
        ("listMerchantUsageEvents", |c| {
            send(c.merchant_usage_events().list())
        }),
        ("getMerchantUsageSummary", |c| {
            send(c.merchant_usage_events().summary())
        }),
        ("getMerchantUsageEvent", |c| {
            send(c.merchant_usage_events().get("mev_1"))
        }),
        ("retryMerchantUsageEvent", |c| {
            send(c.merchant_usage_events().retry("mev_1"))
        }),
        ("discardMerchantUsageEvent", |c| {
            send(c.merchant_usage_events().discard("mev_1"))
        }),
        ("getUsage", |c| send(c.usage().get())),
        ("getUsageOverview", |c| send(c.usage().overview())),
        ("getAgentUsage", |c| send(c.usage().get_for_agent("ag_1"))),
        ("getSessionUsage", |c| send(c.usage().sessions(["ss_1"]))),
        ("createTask", |c| {
            send(c.tasks().create("ag_1", "Nightly", "Summarize"))
        }),
        ("listTasks", |c| send(c.tasks().list())),
        ("getTask", |c| send(c.tasks().get("tk_1"))),
        ("updateTask", |c| send(c.tasks().update("tk_1"))),
        ("createTaskRun", |c| send(c.tasks().submit("tk_1"))),
        ("listTaskRuns", |c| send(c.tasks().list_runs("tk_1"))),
        ("getTaskRun", |c| send(c.tasks().get_run("tk_1", "tr_1"))),
        ("listTaskRunMessages", |c| {
            send(c.tasks().run_messages("tk_1", "tr_1"))
        }),
    ]
}

/// A property the generated response sets, and the value that replaces it.
struct Site {
    pointer: String,
    key: String,
    poison: Value,
}

/// A response the spec allows, with every property it can poison.
struct Instance {
    value: Value,
    sites: Vec<Site>,
}

/// Properties the spec leaves free-form. Any JSON fits them, so no value can
/// show whether a field reads them; the test checks this list instead.
const FREE_FORM: &[&str] = &[
    "SessionMessageList.data[].metadata",
    "SkillCopyResultList[].error.details",
    "TaskRunMessageList.data[].metadata",
    "listSessionInputs:response.data[].message.metadata",
    "submitSessionInput:response.data.message.metadata",
];

struct Spec<'a> {
    schemas: &'a serde_json::Map<String, Value>,
    free_form: RefCell<BTreeSet<String>>,
}

impl<'a> Spec<'a> {
    /// Follows `$ref`s, returning the schema and the last component name.
    fn resolve(&self, mut schema: &'a Value) -> (&'a Value, Option<&'a str>) {
        let mut name = None;
        while let Some(reference) = schema["$ref"].as_str() {
            let component = reference
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("unsupported $ref {reference}"));
            name = Some(component);
            schema = &self.schemas[component];
        }
        (schema, name)
    }

    fn branches(&self, schema: &'a Value) -> Option<Vec<&'a Value>> {
        let list = schema["oneOf"]
            .as_array()
            .or_else(|| schema["anyOf"].as_array())?;
        Some(
            list.iter()
                .filter(|branch| self.resolve(branch).0["type"] != "null")
                .collect(),
        )
    }

    /// The properties of an object schema, with `allOf` members merged.
    fn properties(&self, schema: &'a Value, into: &mut Vec<(&'a str, &'a Value)>) {
        if let Some(members) = schema["allOf"].as_array() {
            for member in members {
                self.properties(self.resolve(member).0, into);
            }
        }
        if let Some(properties) = schema["properties"].as_object() {
            for (name, property) in properties {
                into.retain(|(existing, _)| *existing != name.as_str());
                into.push((name, property));
            }
        }
    }

    /// The JSON types a schema accepts; `None` when it accepts anything.
    fn types(&self, schema: &'a Value) -> Option<BTreeSet<&'static str>> {
        let (schema, _) = self.resolve(schema);
        if let Some(list) = schema["oneOf"]
            .as_array()
            .or_else(|| schema["anyOf"].as_array())
        {
            let mut all = BTreeSet::new();
            for branch in list {
                all.extend(self.types(branch)?);
            }
            return Some(all);
        }
        let values: Vec<&Value> = match (&schema["const"], schema["enum"].as_array()) {
            (Value::Null, None) => Vec::new(),
            (Value::Null, Some(values)) => values.iter().collect(),
            (value, _) => vec![value],
        };
        if !values.is_empty() {
            return Some(values.iter().map(|value| json_type(value)).collect());
        }
        let named: Vec<&str> = match &schema["type"] {
            Value::String(name) => vec![name.as_str()],
            Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
            _ if schema.get("properties").is_some() || schema.get("allOf").is_some() => {
                vec!["object"]
            }
            _ => return None,
        };
        Some(
            named
                .into_iter()
                .map(|name| match name {
                    "integer" | "number" => "number",
                    "string" => "string",
                    "boolean" => "boolean",
                    "array" => "array",
                    "object" => "object",
                    "null" => "null",
                    other => panic!("unknown JSON type {other}"),
                })
                .collect(),
        )
    }

    /// A value of a type the property's schema does not accept.
    fn poison(&self, schema: &'a Value) -> Option<Value> {
        let types = self.types(schema)?;
        [
            ("boolean", json!(true)),
            ("string", json!("spec-field-poison")),
            ("array", json!([true])),
        ]
        .into_iter()
        .find(|(name, _)| !types.contains(name))
        .map(|(_, value)| value)
    }

    /// Responses covering every branch, each with its poisonable properties.
    fn instances(&self, schema: &'a Value, owner: &str, stack: &mut Vec<&'a str>) -> Vec<Instance> {
        let (schema, name) = self.resolve(schema);
        let owner = name.map_or_else(|| owner.to_owned(), str::to_owned);
        if let Some(name) = name {
            assert!(
                !stack.contains(&name),
                "recursive schema {name} is not supported"
            );
            stack.push(name);
        }
        let instances = self.instances_of(schema, &owner, stack);
        if name.is_some() {
            stack.pop();
        }
        instances
    }

    fn instances_of(
        &self,
        schema: &'a Value,
        owner: &str,
        stack: &mut Vec<&'a str>,
    ) -> Vec<Instance> {
        let leaf = |value| {
            vec![Instance {
                value,
                sites: Vec::new(),
            }]
        };
        if let Some(branches) = self.branches(schema) {
            return branches
                .into_iter()
                .flat_map(|branch| self.instances(branch, owner, stack))
                .collect();
        }
        if !schema["const"].is_null() {
            return leaf(schema["const"].clone());
        }
        if let Some(values) = schema["enum"].as_array() {
            return leaf(
                values
                    .iter()
                    .find(|value| !value.is_null())
                    .cloned()
                    .unwrap_or(Value::Null),
            );
        }
        let kind = match &schema["type"] {
            Value::String(kind) => kind.as_str(),
            Value::Array(kinds) => kinds
                .iter()
                .filter_map(Value::as_str)
                .find(|kind| *kind != "null")
                .unwrap_or("null"),
            _ if schema.get("properties").is_some() || schema.get("allOf").is_some() => "object",
            _ => return leaf(json!({})),
        };
        match kind {
            "object" => self.object(schema, owner, stack),
            "array" => self
                .instances(&schema["items"], &format!("{owner}[]"), stack)
                .into_iter()
                .map(|item| Instance {
                    value: json!([item.value]),
                    sites: prefixed("/0", item.sites),
                })
                .collect(),
            "string" => leaf(json!(match schema["format"].as_str() {
                Some("date") => "2026-01-01",
                Some("uri" | "url") => "https://example.com/x",
                Some("email") => "user@example.com",
                Some("uuid") => "00000000-0000-4000-8000-000000000000",
                // The spec leaves many timestamps without a format, and any
                // other string field accepts a timestamp too.
                _ => "2026-01-01T00:00:00.000Z",
            })),
            "integer" | "number" => leaf(
                schema["minimum"]
                    .as_f64()
                    .or_else(|| {
                        schema["exclusiveMinimum"]
                            .as_f64()
                            .map(|minimum| minimum + 1.0)
                    })
                    .map_or(json!(1), |minimum| json!(minimum.max(1.0) as u64)),
            ),
            "boolean" => leaf(json!(true)),
            "null" => leaf(Value::Null),
            other => panic!("unknown JSON type {other}"),
        }
    }

    fn object(&self, schema: &'a Value, owner: &str, stack: &mut Vec<&'a str>) -> Vec<Instance> {
        let mut properties = Vec::new();
        self.properties(schema, &mut properties);
        let fields: Vec<(&str, Vec<Instance>, Option<Value>)> = properties
            .into_iter()
            .map(|(name, property)| {
                let key = format!("{owner}.{name}");
                let poison = self.poison(property);
                if poison.is_none() {
                    self.free_form.borrow_mut().insert(key.clone());
                }
                (name, self.instances(property, &key, stack), poison)
            })
            .collect();
        let count = fields
            .iter()
            .map(|(_, choices, _)| choices.len())
            .max()
            .unwrap_or(1);
        (0..count)
            .map(|index| {
                let mut value = serde_json::Map::new();
                let mut sites = Vec::new();
                for (name, choices, poison) in &fields {
                    let choice = &choices[index.min(choices.len() - 1)];
                    value.insert((*name).to_owned(), choice.value.clone());
                    if let Some(poison) = poison {
                        sites.push(Site {
                            pointer: format!("/{name}"),
                            key: format!("{owner}.{name}"),
                            poison: poison.clone(),
                        });
                    }
                    if index < choices.len() {
                        sites.extend(prefixed(&format!("/{name}"), clone_sites(&choice.sites)));
                    }
                }
                Instance {
                    value: Value::Object(value),
                    sites,
                }
            })
            .collect()
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn clone_sites(sites: &[Site]) -> Vec<Site> {
    sites
        .iter()
        .map(|site| Site {
            pointer: site.pointer.clone(),
            key: site.key.clone(),
            poison: site.poison.clone(),
        })
        .collect()
}

fn prefixed(prefix: &str, sites: Vec<Site>) -> Vec<Site> {
    sites
        .into_iter()
        .map(|site| Site {
            pointer: format!("{prefix}{}", site.pointer),
            ..site
        })
        .collect()
}

/// Serves whatever response the test last set.
#[derive(Clone, Default)]
struct Current(Arc<Mutex<(u16, Value)>>);

impl Respond for Current {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let (status, body) = self.0.lock().unwrap().clone();
        ResponseTemplate::new(status).set_body_json(body)
    }
}

/// `operationId` or `Schema.property` mapped to the reason it is not modeled.
fn omissions() -> BTreeMap<String, String> {
    let text = include_str!("spec_field_omissions.txt");
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (key, reason) = line
                .split_once(": ")
                .unwrap_or_else(|| panic!("omission without a reason: {line}"));
            (key.to_owned(), reason.to_owned())
        })
        .collect()
}

#[tokio::test]
async fn every_response_property_maps_to_a_model_field() {
    let spec: Value =
        serde_json::from_str(include_str!("../spec/openapi.json")).expect("the spec is JSON");
    let walker = Spec {
        schemas: spec["components"]["schemas"]
            .as_object()
            .expect("the spec has component schemas"),
        free_form: RefCell::default(),
    };
    let server = MockServer::start().await;
    let current = Current::default();
    Mock::given(any())
        .respond_with(current.clone())
        .mount(&server)
        .await;
    let observed: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .on_response({
            let observed = observed.clone();
            move |observation| observed.lock().unwrap().push(observation.operation_id)
        })
        .build()
        .unwrap();

    let omissions = omissions();
    let mut methods: BTreeMap<&str, Method> = methods().into_iter().collect();
    let mut problems = Vec::new();
    let mut missing: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut checked = 0;

    for item in spec["paths"].as_object().unwrap().values() {
        for operation in item.as_object().unwrap().values() {
            let Some(id) = operation["operationId"].as_str() else {
                continue;
            };
            let Some((status, schema)) =
                operation["responses"].as_object().unwrap().iter().find_map(
                    |(status, response)| {
                        let schema = response["content"]["application/json"].get("schema")?;
                        status
                            .starts_with('2')
                            .then(|| (status.parse::<u16>().unwrap(), schema))
                    },
                )
            else {
                continue;
            };
            let method = methods.remove(id);
            if omissions.contains_key(id) {
                if method.is_some() {
                    problems.push(format!("{id} is listed as an omission but has a method"));
                }
                continue;
            }
            let Some(method) = method else {
                problems.push(format!(
                    "{id} has a JSON response but no entry in methods()"
                ));
                continue;
            };
            for instance in walker.instances(schema, &format!("{id}:response"), &mut Vec::new()) {
                *current.0.lock().unwrap() = (status, instance.value.clone());
                observed.lock().unwrap().clear();
                if let Err(error) = method(&client).await {
                    problems.push(format!(
                        "{id}: a response with every spec property does not decode: {error:?}\n{}",
                        instance.value
                    ));
                    continue;
                }
                if *observed.lock().unwrap() != [id] {
                    problems.push(format!(
                        "{id}: methods() sends {:?} instead",
                        observed.lock().unwrap()
                    ));
                    continue;
                }
                for site in &instance.sites {
                    let mut poisoned = instance.value.clone();
                    *poisoned.pointer_mut(&site.pointer).unwrap() = site.poison.clone();
                    *current.0.lock().unwrap() = (status, poisoned);
                    checked += 1;
                    if method(&client).await.is_ok() {
                        missing
                            .entry(site.key.clone())
                            .or_default()
                            .insert(id.to_owned());
                    }
                }
            }
        }
    }

    problems.extend(
        methods
            .keys()
            .map(|id| format!("{id} is in methods() but has no JSON response in the spec")),
    );
    for (key, operations) in &missing {
        if !omissions.contains_key(key) {
            problems.push(format!(
                "{key} has no model field (seen in {}). Add the field, or add it to tests/spec_field_omissions.txt with a reason.",
                operations.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    for key in omissions.keys() {
        if key.contains('.') && !missing.contains_key(key) {
            problems.push(format!(
                "{key} is listed in tests/spec_field_omissions.txt but the model reads it; remove the entry"
            ));
        }
    }
    assert!(checked > 0, "no property was checked");
    assert_eq!(
        walker.free_form.into_inner(),
        FREE_FORM.iter().map(|key| (*key).to_owned()).collect(),
        "the spec's free-form response properties changed; update FREE_FORM"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
