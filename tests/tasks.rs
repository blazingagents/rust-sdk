mod support;

use blazing_agents::ids::{AgentId, SessionId, TaskId, TaskRunId, TenantId, TurnId};
use blazing_agents::tasks::{Schedule, TaskRunStatus};
use blazing_agents::types::Metadata;
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn task(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "name": "Daily support summary",
        "prompt": "Summarize yesterday's open support cases and flag any that are overdue.",
        "schedule": {
            "kind": "cron",
            "config": { "expression": "0 9 * * 1-5", "timezone": "Europe/London" }
        },
        "enabled": true,
        "nextFireAt": "2026-07-13T08:00:00.000Z",
        "activeRunId": null,
        "latestRunId": null,
        "userId": "",
        "metadata": { "team": "support" },
        "deletedAt": null,
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:00:00.000Z"
    })
}

fn task_run(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "taskId": "tk_6Wq3Hn8ZpL2vRt5C",
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "agentConfig": {
            "approvalInChat": { "default": "full", "overrides": [] },
            "approvalInTasks": { "default": "full", "overrides": [] },
            "name": "Support Agent",
            "model": "openai/gpt-6-luna",
            "thinkingLevel": null,
            "providerId": "prv_7Tn4Kd9QwE2sLx5R",
            "autoCompaction": true,
            "compactionReserveTokens": 16384,
            "memoryInjectionEnabled": false,
            "tools": ["read", "grep", "glob", "write_todos"],
            "instructions": "Answer billing questions clearly and briefly.",
            "metadata": { "team": "support" },
            "mcpConnectionIds": []
        },
        "sessionId": "ss_5Ty8Lr2GhW4nZc7F",
        "turnId": "turn_3Xp6Mv9QdB1sKe4H",
        "status": "succeeded",
        "error": null,
        "userId": "",
        "metadata": { "team": "support" },
        "startedAt": "2026-07-10T10:00:01.000Z",
        "finishedAt": "2026-07-10T10:03:00.000Z",
        "cancelRequestedAt": null,
        "canceledAt": null,
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:03:00.000Z"
    })
}

fn message(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "role": "assistant",
        "branchable": false,
        "parts": [{ "type": "text", "text": "Hello." }]
    })
}

fn messages_page(data: Vec<serde_json::Value>, next: serde_json::Value) -> serde_json::Value {
    json!({
        "data": data,
        "nextCursor": next,
        "latestCursor": "eyJzZXEiOjEyfQ",
        "status": "succeeded",
        "error": null,
        "finishedAt": "2026-07-10T10:03:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_the_body_and_decodes_the_task() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/tasks"))
        .respond_with(json(
            201,
            json!({ "task": task("tk_6Wq3Hn8ZpL2vRt5C"), "runId": null }),
        ))
        .mount(&server)
        .await;

    let created = client
        .tasks()
        .create(
            "ag_4kP9sT2vXq7LmN3a",
            "Daily support summary",
            "Summarize yesterday's open support cases and flag any that are overdue.",
        )
        .idempotency_key("daily-summary:2026-10-01")
        .schedule(
            Schedule::cron("0 9 * * 1-5")
                .timezone("Europe/London")
                .stagger_ms(30000),
        )
        .enabled(true)
        .user_id("customer_123")
        .metadata(Metadata::new().insert("team", "support"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "agentId": "ag_4kP9sT2vXq7LmN3a",
            "name": "Daily support summary",
            "prompt": "Summarize yesterday's open support cases and flag any that are overdue.",
            "idempotencyKey": "daily-summary:2026-10-01",
            "schedule": {
                "kind": "cron",
                "config": {
                    "expression": "0 9 * * 1-5",
                    "timezone": "Europe/London",
                    "staggerMs": 30000
                }
            },
            "enabled": true,
            "userId": "customer_123",
            "metadata": { "team": "support" }
        })
    );
    let created_task = created.task;
    assert_eq!(created_task.id, TaskId::from("tk_6Wq3Hn8ZpL2vRt5C"));
    assert_eq!(
        created_task.tenant_id,
        TenantId::from("ten_8Hq2Zr5WcY1bJt6D")
    );
    assert_eq!(created_task.agent_id, AgentId::from("ag_4kP9sT2vXq7LmN3a"));
    assert_eq!(created_task.name, "Daily support summary");
    assert_eq!(
        created_task.schedule,
        Some(Schedule::cron("0 9 * * 1-5").timezone("Europe/London"))
    );
    assert!(created_task.enabled);
    assert_eq!(created_task.active_run_id, None);
    assert_eq!(created_task.latest_run_id, None);
    assert_eq!(
        created_task.next_fire_at.map(|at| at.to_string()),
        Some("2026-07-13T08:00:00Z".to_owned())
    );
    assert_eq!(created_task.user_id, "");
    assert_eq!(created_task.metadata.get("team"), Some(&json!("support")));
    assert_eq!(created_task.deleted_at, None);
    assert_eq!(created_task.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(created_task.updated_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(created.run_id, None);
}

#[tokio::test]
async fn create_with_submit_decodes_the_accepted_response() {
    let (server, client) = mock().await;
    let mut submitted = task("tk_6Wq3Hn8ZpL2vRt5C");
    submitted["schedule"] = json!(null);
    submitted["nextFireAt"] = json!(null);
    submitted["activeRunId"] = json!("tr_9Jd4Ks7NbV2xQm6P");
    submitted["latestRunId"] = json!("tr_9Jd4Ks7NbV2xQm6P");
    Mock::given(method("POST"))
        .and(path("/v1/tasks"))
        .respond_with(json(
            202,
            json!({ "task": submitted, "runId": "tr_9Jd4Ks7NbV2xQm6P" }),
        ))
        .mount(&server)
        .await;

    let created = client
        .tasks()
        .create(
            "ag_4kP9sT2vXq7LmN3a",
            "Daily support summary",
            "Summarize yesterday's open support cases and flag any that are overdue.",
        )
        .submit(true)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "agentId": "ag_4kP9sT2vXq7LmN3a",
            "name": "Daily support summary",
            "prompt": "Summarize yesterday's open support cases and flag any that are overdue.",
            "submit": true
        })
    );
    assert_eq!(created.run_id, Some(TaskRunId::from("tr_9Jd4Ks7NbV2xQm6P")));
    assert_eq!(
        created.task.active_run_id,
        Some(TaskRunId::from("tr_9Jd4Ks7NbV2xQm6P"))
    );
    assert_eq!(created.task.schedule, None);
}

#[tokio::test]
async fn list_sends_filters_and_decodes_latest_run() {
    let (server, client) = mock().await;
    let mut item = task("tk_6Wq3Hn8ZpL2vRt5C");
    item["latestRunId"] = json!("tr_9Jd4Ks7NbV2xQm6P");
    item["latestRun"] = json!({
        "id": "tr_9Jd4Ks7NbV2xQm6P",
        "status": "succeeded",
        "finishedAt": "2026-07-10T10:03:00.000Z"
    });
    Mock::given(method("GET"))
        .and(path("/v1/tasks"))
        .and(query_param("agentId", "ag_4kP9sT2vXq7LmN3a"))
        .and(query_param("userId", "customer_123"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(200, json!({ "data": [item], "nextCursor": "c2" })))
        .mount(&server)
        .await;

    let page = client
        .tasks()
        .list()
        .agent_id("ag_4kP9sT2vXq7LmN3a")
        .user_id("customer_123")
        .limit(25)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(
        page.next_cursor.map(|cursor| cursor.to_string()),
        Some("c2".to_owned())
    );
    let latest = page.data[0].latest_run.as_ref().unwrap();
    assert_eq!(latest.id, TaskRunId::from("tr_9Jd4Ks7NbV2xQm6P"));
    assert_eq!(latest.status, TaskRunStatus::Succeeded);
    assert_eq!(
        latest.finished_at.map(|at| at.to_string()),
        Some("2026-07-10T10:03:00Z".to_owned())
    );
}

#[tokio::test]
async fn get_ignores_unknown_response_fields() {
    let (server, client) = mock().await;
    let mut body = task("tk_6Wq3Hn8ZpL2vRt5C");
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client.tasks().get("tk_6Wq3Hn8ZpL2vRt5C").await.unwrap();

    assert_eq!(fetched.name, "Daily support summary");
    assert_eq!(
        fetched.prompt,
        "Summarize yesterday's open support cases and flag any that are overdue."
    );
}

#[tokio::test]
async fn unknown_enums_decode_as_unrecognized() {
    let (server, client) = mock().await;
    let mut body = task("tk_6Wq3Hn8ZpL2vRt5C");
    body["schedule"] = json!({ "kind": "weekly", "config": { "day": "monday" } });
    Mock::given(method("GET"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;
    let mut run_body = task_run("tr_9Jd4Ks7NbV2xQm6P");
    run_body["status"] = json!("paused");
    Mock::given(method("GET"))
        .and(path(
            "/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs/tr_9Jd4Ks7NbV2xQm6P",
        ))
        .respond_with(json(200, run_body))
        .mount(&server)
        .await;

    let fetched = client.tasks().get("tk_6Wq3Hn8ZpL2vRt5C").await.unwrap();
    match fetched.schedule {
        Some(Schedule::Unrecognized(schedule)) => assert_eq!(schedule.kind(), "weekly"),
        other => panic!("expected an unrecognized schedule kind, got {other:?}"),
    }

    let run = client
        .tasks()
        .get_run("tk_6Wq3Hn8ZpL2vRt5C", "tr_9Jd4Ks7NbV2xQm6P")
        .await
        .unwrap();
    match &run.status {
        TaskRunStatus::Unrecognized(status) => assert_eq!(status.as_str(), "paused"),
        other => panic!("expected an unrecognized status, got {other:?}"),
    }
    assert_eq!(run.status.as_str(), "paused");
}

#[tokio::test]
async fn update_sends_back_an_unknown_schedule_unchanged() {
    let (server, client) = mock().await;
    let mut body = task("tk_6Wq3Hn8ZpL2vRt5C");
    body["schedule"] = json!({ "kind": "weekly", "config": { "day": "mon" }, "extra": 1 });
    Mock::given(method("GET"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C"))
        .respond_with(json(200, task("tk_6Wq3Hn8ZpL2vRt5C")))
        .mount(&server)
        .await;

    let fetched = client.tasks().get("tk_6Wq3Hn8ZpL2vRt5C").await.unwrap();
    client
        .tasks()
        .update("tk_6Wq3Hn8ZpL2vRt5C")
        .schedule(fetched.schedule.unwrap())
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[1]),
        json!({ "schedule": { "kind": "weekly", "config": { "day": "mon" }, "extra": 1 } })
    );
}

#[tokio::test]
async fn update_omits_unset_fields_and_sends_null_to_clear() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C"))
        .respond_with(json(200, task("tk_6Wq3Hn8ZpL2vRt5C")))
        .mount(&server)
        .await;

    client
        .tasks()
        .update("tk_6Wq3Hn8ZpL2vRt5C")
        .name("Renamed")
        .await
        .unwrap();
    client
        .tasks()
        .update("tk_6Wq3Hn8ZpL2vRt5C")
        .clear_schedule()
        .await
        .unwrap();
    client
        .tasks()
        .update("tk_6Wq3Hn8ZpL2vRt5C")
        .schedule(Schedule::interval(3_600_000))
        .enabled(false)
        .prompt("New prompt")
        .metadata(Metadata::new().insert("paused_by", "ops"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Renamed" }));
    assert_eq!(json_body(&sent[1]), json!({ "schedule": null }));
    assert_eq!(
        json_body(&sent[2]),
        json!({
            "schedule": { "kind": "interval", "config": { "everyMs": 3600000 } },
            "enabled": false,
            "prompt": "New prompt",
            "metadata": { "paused_by": "ops" }
        })
    );
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.tasks().delete("tk_6Wq3Hn8ZpL2vRt5C").await.unwrap();
}

#[tokio::test]
async fn submit_sends_an_empty_body_or_the_key() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs"))
        .respond_with(json(202, json!({ "runId": "tr_9Jd4Ks7NbV2xQm6P" })))
        .mount(&server)
        .await;

    client.tasks().submit("tk_6Wq3Hn8ZpL2vRt5C").await.unwrap();
    let created = client
        .tasks()
        .submit("tk_6Wq3Hn8ZpL2vRt5C")
        .idempotency_key("daily-summary:2026-10-01")
        .await
        .unwrap();

    assert_eq!(created.run_id, TaskRunId::from("tr_9Jd4Ks7NbV2xQm6P"));
    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({}));
    assert_eq!(
        json_body(&sent[1]),
        json!({ "idempotencyKey": "daily-summary:2026-10-01" })
    );
}

#[tokio::test]
async fn get_run_decodes_every_field() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs/tr_9Jd4Ks7NbV2xQm6P",
        ))
        .respond_with(json(200, task_run("tr_9Jd4Ks7NbV2xQm6P")))
        .mount(&server)
        .await;

    let run = client
        .tasks()
        .get_run("tk_6Wq3Hn8ZpL2vRt5C", "tr_9Jd4Ks7NbV2xQm6P")
        .await
        .unwrap();

    assert_eq!(run.id, TaskRunId::from("tr_9Jd4Ks7NbV2xQm6P"));
    assert_eq!(run.task_id, TaskId::from("tk_6Wq3Hn8ZpL2vRt5C"));
    assert_eq!(run.tenant_id, TenantId::from("ten_8Hq2Zr5WcY1bJt6D"));
    assert_eq!(run.agent_id, AgentId::from("ag_4kP9sT2vXq7LmN3a"));
    assert_eq!(run.session_id, Some(SessionId::from("ss_5Ty8Lr2GhW4nZc7F")));
    assert_eq!(run.turn_id, Some(TurnId::from("turn_3Xp6Mv9QdB1sKe4H")));
    assert_eq!(run.status, TaskRunStatus::Succeeded);
    assert_eq!(run.error, None);
    assert_eq!(run.user_id, "");
    assert_eq!(run.metadata.get("team"), Some(&json!("support")));
    assert_eq!(
        run.started_at.map(|at| at.to_string()),
        Some("2026-07-10T10:00:01Z".to_owned())
    );
    assert_eq!(
        run.finished_at.map(|at| at.to_string()),
        Some("2026-07-10T10:03:00Z".to_owned())
    );
    assert_eq!(run.cancel_requested_at, None);
    assert_eq!(run.canceled_at, None);
    assert_eq!(run.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(run.updated_at.to_string(), "2026-07-10T10:03:00Z");
}

#[tokio::test]
async fn run_messages_decodes_the_page_and_run_state() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs/tr_9Jd4Ks7NbV2xQm6P/messages",
        ))
        .and(query_param_is_missing("cursor"))
        .and(query_param_is_missing("after"))
        .and(query_param("limit", "20"))
        .respond_with(json(
            200,
            messages_page(vec![message("msg_1")], json!(null)),
        ))
        .mount(&server)
        .await;

    let page = client
        .tasks()
        .run_messages("tk_6Wq3Hn8ZpL2vRt5C", "tr_9Jd4Ks7NbV2xQm6P")
        .limit(20)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.next_cursor, None);
    assert_eq!(
        page.latest_cursor.map(|cursor| cursor.to_string()),
        Some("eyJzZXEiOjEyfQ".to_owned())
    );
    assert_eq!(page.status, TaskRunStatus::Succeeded);
    assert_eq!(page.error, None);
    assert_eq!(
        page.finished_at.map(|at| at.to_string()),
        Some("2026-07-10T10:03:00Z".to_owned())
    );
}

#[tokio::test]
async fn cancel_run_posts_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(
            "/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs/tr_9Jd4Ks7NbV2xQm6P/cancel",
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client
        .tasks()
        .cancel_run("tk_6Wq3Hn8ZpL2vRt5C", "tr_9Jd4Ks7NbV2xQm6P")
        .await
        .unwrap();
}

#[tokio::test]
async fn items_walks_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/tasks"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [task("tk_1"), task("tk_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/tasks"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [task("tk_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.tasks().list().limit(2).items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|task| task.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["tk_1", "tk_2", "tk_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2&limit=2"));
}

#[tokio::test]
async fn run_items_walk_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [task_run("tr_1"), task_run("tr_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [task_run("tr_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client
        .tasks()
        .list_runs("tk_6Wq3Hn8ZpL2vRt5C")
        .limit(2)
        .items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|run| run.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["tr_1", "tr_2", "tr_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2&limit=2"));
}

#[tokio::test]
async fn run_messages_items_walk_backward_with_cursor() {
    let (server, client) = mock().await;
    let messages_path = "/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs/tr_9Jd4Ks7NbV2xQm6P/messages";
    Mock::given(method("GET"))
        .and(path(messages_path))
        .and(query_param_is_missing("cursor"))
        .and(query_param_is_missing("after"))
        .respond_with(json(
            200,
            messages_page(vec![message("msg_1")], json!("c2")),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(messages_path))
        .and(query_param("cursor", "c2"))
        .and(query_param_is_missing("after"))
        .respond_with(json(
            200,
            messages_page(vec![message("msg_2")], json!(null)),
        ))
        .mount(&server)
        .await;

    let items = client
        .tasks()
        .run_messages("tk_6Wq3Hn8ZpL2vRt5C", "tr_9Jd4Ks7NbV2xQm6P")
        .items();
    assert!(requests(&server).await.is_empty());
    let count = items.try_collect::<Vec<_>>().await.unwrap().len();

    assert_eq!(count, 2);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].url.query(), None);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn run_messages_items_poll_forward_with_after() {
    let (server, client) = mock().await;
    let messages_path = "/v1/tasks/tk_6Wq3Hn8ZpL2vRt5C/runs/tr_9Jd4Ks7NbV2xQm6P/messages";
    Mock::given(method("GET"))
        .and(path(messages_path))
        .and(query_param("after", "c1"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            messages_page(vec![message("msg_2")], json!("c2")),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(messages_path))
        .and(query_param("after", "c2"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            messages_page(vec![message("msg_3")], json!(null)),
        ))
        .mount(&server)
        .await;

    let items = client
        .tasks()
        .run_messages("tk_6Wq3Hn8ZpL2vRt5C", "tr_9Jd4Ks7NbV2xQm6P")
        .after("c1")
        .items();
    assert!(requests(&server).await.is_empty());
    let count = items.try_collect::<Vec<_>>().await.unwrap().len();

    assert_eq!(count, 2);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].url.query(), Some("after=c1"));
    assert_eq!(sent[1].url.query(), Some("after=c2"));
}

#[test]
fn schedule_serializes_to_the_wire_shape() {
    assert_eq!(
        serde_json::to_value(Schedule::once("2026-10-01T09:00:00Z".parse().unwrap())).unwrap(),
        json!({ "kind": "once", "config": { "at": "2026-10-01T09:00:00Z" } })
    );
    assert_eq!(
        serde_json::to_value(Schedule::interval(3_600_000)).unwrap(),
        json!({ "kind": "interval", "config": { "everyMs": 3600000 } })
    );
    assert_eq!(
        serde_json::to_value(Schedule::cron("0 9 * * 1-5")).unwrap(),
        json!({ "kind": "cron", "config": { "expression": "0 9 * * 1-5" } })
    );
    assert_eq!(
        serde_json::to_value(
            Schedule::cron("0 9 * * 1-5")
                .timezone("Europe/London")
                .stagger_ms(30000)
        )
        .unwrap(),
        json!({
            "kind": "cron",
            "config": {
                "expression": "0 9 * * 1-5",
                "timezone": "Europe/London",
                "staggerMs": 30000
            }
        })
    );
}

#[test]
fn schedule_decodes_each_kind() {
    assert_eq!(
        serde_json::from_value::<Schedule>(
            json!({ "kind": "once", "config": { "at": "2026-10-01T09:00:00Z" } })
        )
        .unwrap(),
        Schedule::once("2026-10-01T09:00:00Z".parse().unwrap())
    );
    assert_eq!(
        serde_json::from_value::<Schedule>(
            json!({ "kind": "interval", "config": { "everyMs": 3600000 } })
        )
        .unwrap(),
        Schedule::interval(3_600_000)
    );
    // Response cron configs always carry the timezone; extra fields a newer
    // config adds are ignored.
    assert_eq!(
        serde_json::from_value::<Schedule>(json!({
            "kind": "cron",
            "config": {
                "expression": "0 9 * * 1-5",
                "timezone": "Europe/London",
                "addedNextYear": true
            }
        }))
        .unwrap(),
        Schedule::cron("0 9 * * 1-5").timezone("Europe/London")
    );
}

#[test]
fn unrecognized_schedule_round_trips() {
    let raw = json!({ "kind": "weekly", "config": { "day": "mon" }, "extra": 1 });
    let schedule = serde_json::from_value::<Schedule>(raw.clone()).unwrap();

    let Schedule::Unrecognized(inner) = &schedule else {
        panic!("expected an unrecognized schedule, got {schedule:?}");
    };
    assert_eq!(inner.kind(), "weekly");
    assert_eq!(inner.as_json()["extra"], json!(1));
    assert_eq!(serde_json::to_value(&schedule).unwrap(), raw);
}

#[test]
fn schedule_rejects_a_missing_or_non_string_kind() {
    assert!(serde_json::from_value::<Schedule>(json!({ "config": {} })).is_err());
    assert!(serde_json::from_value::<Schedule>(json!({ "kind": 7 })).is_err());
}

#[test]
fn schedule_rejects_malformed_known_kinds() {
    assert!(serde_json::from_value::<Schedule>(json!({ "kind": "once" })).is_err());
    assert!(serde_json::from_value::<Schedule>(json!({ "kind": "cron", "config": {} })).is_err());
    assert!(
        serde_json::from_value::<Schedule>(
            json!({ "kind": "interval", "config": { "everyMs": "x" } })
        )
        .is_err()
    );
}

#[test]
fn cron_only_setters_leave_other_kinds_alone() {
    assert_eq!(
        Schedule::interval(3_600_000)
            .timezone("Europe/London")
            .stagger_ms(30000),
        Schedule::interval(3_600_000)
    );
}
