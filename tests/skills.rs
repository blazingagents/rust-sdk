mod support;

use blazing_agents::error::ErrorCode;
use blazing_agents::files::Upload;
use blazing_agents::skills::{ArchiveType, SkillCopyResult};
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn skill(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "name": "refund-policy",
        "description": "Answer refund questions using the current refund policy.",
        "metadata": { "team": "support" },
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z",
        "files": [
            { "path": "SKILL.md", "sizeBytes": 240 },
            { "path": "scripts/deploy.sh", "sizeBytes": 58 }
        ]
    })
}

fn skill_summary(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "name": "refund-policy",
        "description": "Answer refund questions using the current refund policy.",
        "metadata": { "team": "support" },
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    })
}

fn document() -> &'static str {
    "---\nname: refund-policy\ndescription: Answer refund questions.\n---\n\nQuote the matching rule.\n"
}

#[tokio::test]
async fn create_sends_the_document_and_decodes_the_skill() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/skills"))
        .respond_with(json(201, skill("skill_5Wn3Hc7TbK2xQv9F")))
        .mount(&server)
        .await;

    let created = client
        .agent("ag_1")
        .skills()
        .create("SKILL.md", document())
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "path": "SKILL.md", "content": document() })
    );
    assert_eq!(created.id.as_str(), "skill_5Wn3Hc7TbK2xQv9F");
    assert_eq!(created.tenant_id.as_str(), "ten_8Hq2Zr5WcY1bJt6D");
    assert_eq!(created.agent_id.as_str(), "ag_4kP9sT2vXq7LmN3a");
    assert_eq!(created.name, "refund-policy");
    assert_eq!(
        created.description,
        "Answer refund questions using the current refund policy."
    );
    assert_eq!(
        created.metadata.as_ref().and_then(|m| m.get("team")),
        Some(&"support".to_owned())
    );
    assert_eq!(created.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(created.files.len(), 2);
    assert_eq!(created.files[0].path, "SKILL.md");
    assert_eq!(created.files[0].size_bytes, 240);
    assert_eq!(created.files[1].path, "scripts/deploy.sh");
}

#[tokio::test]
async fn upload_sends_a_multipart_archive() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/skills/upload"))
        .respond_with(json(201, skill("skill_1")))
        .mount(&server)
        .await;

    client
        .agent("ag_1")
        .skills()
        .upload(
            ArchiveType::TarGz,
            Upload::bytes(b"archive-bytes".as_slice()).file_name("skill.tar.gz"),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert!(
        sent[0]
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let multipart = String::from_utf8_lossy(&sent[0].body);
    assert!(multipart.contains("name=\"type\""), "{multipart}");
    assert!(multipart.contains("tar.gz"), "{multipart}");
    assert!(
        multipart.contains("name=\"file\"; filename=\"skill.tar.gz\""),
        "{multipart}"
    );
    assert!(multipart.contains("archive-bytes"), "{multipart}");
}

#[tokio::test]
async fn list_sends_limit_and_walks_every_page() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/skills"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [skill_summary("skill_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/skills"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [skill_summary("skill_2")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let page = client
        .agent("ag_1")
        .skills()
        .list()
        .limit(25)
        .await
        .unwrap();
    assert_eq!(page.data.len(), 1);
    assert_eq!(
        page.next_cursor.map(|cursor| cursor.to_string()),
        Some("c2".to_owned())
    );

    let ids: Vec<String> = client
        .agent("ag_1")
        .skills()
        .list()
        .limit(25)
        .items()
        .map_ok(|skill| skill.id.to_string())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(ids, ["skill_1", "skill_2"]);
}

#[tokio::test]
async fn get_decodes_unknown_fields() {
    let (server, client) = mock().await;
    let mut body = skill("skill_1");
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/skills/skill_1"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client.agent("ag_1").skills().get("skill_1").await.unwrap();

    assert_eq!(fetched.name, "refund-policy");
    assert_eq!(fetched.files.len(), 2);
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/agents/ag_1/skills/skill_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client
        .agent("ag_1")
        .skills()
        .delete("skill_1")
        .await
        .unwrap();
}

#[tokio::test]
async fn read_file_streams_the_body() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "scripts/deploy.sh"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(b"#!/bin/sh\ndeploy\n".to_vec(), "application/octet-stream"),
        )
        .mount(&server)
        .await;

    let download = client
        .agent("ag_1")
        .skills()
        .read_file("skill_1", "scripts/deploy.sh")
        .await
        .unwrap();

    assert_eq!(download.content_type(), Some("application/octet-stream"));
    assert_eq!(
        download.bytes().await.unwrap().as_ref(),
        b"#!/bin/sh\ndeploy\n"
    );
}

#[tokio::test]
async fn replace_file_sends_the_raw_body() {
    let (server, client) = mock().await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "scripts/deploy.sh"))
        .respond_with(json(200, skill("skill_1")))
        .mount(&server)
        .await;

    let updated = client
        .agent("ag_1")
        .skills()
        .replace_file(
            "skill_1",
            "scripts/deploy.sh",
            Upload::bytes(b"#!/bin/sh\ndeploy --all\n".as_slice()),
        )
        .await
        .unwrap();

    assert_eq!(updated.id.as_str(), "skill_1");
    let sent = requests(&server).await;
    assert_eq!(
        sent[0]
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/octet-stream")
    );
    assert_eq!(sent[0].body, b"#!/bin/sh\ndeploy --all\n");
}

#[tokio::test]
async fn delete_file_sends_the_path_and_decodes_the_skill() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "old.md"))
        .respond_with(json(200, skill("skill_1")))
        .mount(&server)
        .await;

    let updated = client
        .agent("ag_1")
        .skills()
        .delete_file("skill_1", "old.md")
        .await
        .unwrap();

    assert_eq!(updated.id.as_str(), "skill_1");
}

#[tokio::test]
async fn copy_decodes_each_result_in_order() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/skills/skill_1/copies"))
        .respond_with(json(
            200,
            json!([
                {
                    "agentId": "ag_9Xm3Fp7KdR2vHs5T",
                    "status": "created",
                    "skill": skill("skill_6Qw8Er2Ty4Ui7Ok1")
                },
                {
                    "agentId": "ag_2Bt6Yh4MsW8kLc1V",
                    "status": "failed",
                    "error": {
                        "code": "skill_name_conflict",
                        "message": "The agent already has a skill named refund-policy.",
                        "details": { "name": "refund-policy" }
                    }
                },
                {
                    "agentId": "ag_7Zn4Xc6Vb1Nm3Qw8",
                    "status": "skipped",
                    "reason": "same_skill"
                }
            ]),
        ))
        .mount(&server)
        .await;

    let results = client
        .agent("ag_1")
        .skills()
        .copy(
            "skill_1",
            [
                "ag_9Xm3Fp7KdR2vHs5T",
                "ag_2Bt6Yh4MsW8kLc1V",
                "ag_7Zn4Xc6Vb1Nm3Qw8",
            ],
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "agentIds": ["ag_9Xm3Fp7KdR2vHs5T", "ag_2Bt6Yh4MsW8kLc1V", "ag_7Zn4Xc6Vb1Nm3Qw8"]
        })
    );
    assert_eq!(results.len(), 3);
    match &results[0] {
        SkillCopyResult::Created {
            agent_id, skill, ..
        } => {
            assert_eq!(agent_id.as_str(), "ag_9Xm3Fp7KdR2vHs5T");
            assert_eq!(skill.id.as_str(), "skill_6Qw8Er2Ty4Ui7Ok1");
            assert_eq!(skill.files.len(), 2);
        }
        other => panic!("expected a created result, got {other:?}"),
    }
    match &results[1] {
        SkillCopyResult::Failed {
            agent_id, error, ..
        } => {
            assert_eq!(agent_id.as_str(), "ag_2Bt6Yh4MsW8kLc1V");
            assert_eq!(error.code, ErrorCode::SkillNameConflict);
            assert_eq!(
                error.message,
                "The agent already has a skill named refund-policy."
            );
            assert_eq!(error.details, Some(json!({ "name": "refund-policy" })));
        }
        other => panic!("expected a failed result, got {other:?}"),
    }
    match &results[2] {
        SkillCopyResult::Unrecognized(value) => {
            assert_eq!(value["status"], json!("skipped"));
            assert_eq!(value["agentId"], json!("ag_7Zn4Xc6Vb1Nm3Qw8"));
        }
        other => panic!("expected an unrecognized result, got {other:?}"),
    }
}
