mod support;

use blazing_agents::spending_limits::{ResetInterval, SpendingLimitInput};
use blazing_agents::tenant::QuotaInput;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::Mock;
use wiremock::matchers::{method, path};

fn settings() -> serde_json::Value {
    json!({
        "name": "Acme",
        "quota": {
            "monthlyTokenLimit": 5000000,
            "monthlyRequestLimit": 10000,
            "resetDay": 1
        },
        "deletion": null,
        "monetizationEnabled": false
    })
}

fn spending_limit_status() -> serde_json::Value {
    json!({
        "spendingLimit": {
            "amountUsd": 10.0,
            "resetStartDate": "2026-10-01",
            "resetInterval": "monthly"
        },
        "period": {
            "startsAt": "2026-10-01T00:00:00.000Z",
            "endsAt": "2026-11-01T00:00:00.000Z",
            "spentUsd": 1.5,
            "reservedUsd": 0.5,
            "availableUsd": 8.0
        },
        "nextResetAt": "2026-11-01T00:00:00.000Z"
    })
}

#[tokio::test]
async fn get_decodes_the_settings_and_ignores_unknown_fields() {
    let (server, client) = mock().await;
    let mut body = settings();
    body["deletion"] = json!({
        "deletesAt": "2026-08-10T10:00:00.000Z",
        "requestedAt": "2026-07-10T10:00:00.000Z"
    });
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/tenant"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let settings = client.tenant().get().await.unwrap();

    assert_eq!(settings.name, "Acme");
    assert!(!settings.monetization_enabled);
    let quota = settings.quota.unwrap();
    assert_eq!(quota.monthly_token_limit, Some(5000000));
    assert_eq!(quota.monthly_request_limit, Some(10000));
    assert_eq!(quota.reset_day, 1);
    let deletion = settings.deletion.unwrap();
    assert_eq!(deletion.deletes_at.to_string(), "2026-08-10T10:00:00Z");
    assert_eq!(deletion.requested_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn update_omits_unset_fields_and_sends_null_to_clear_quota() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/tenant"))
        .respond_with(json(200, settings()))
        .mount(&server)
        .await;

    client.tenant().update().name("Acme II").await.unwrap();
    client
        .tenant()
        .update()
        .monetization_enabled(true)
        .await
        .unwrap();
    client.tenant().update().clear_quota().await.unwrap();
    client
        .tenant()
        .update()
        .quota(QuotaInput::new(15).monthly_token_limit(5_000_000))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Acme II" }));
    assert_eq!(json_body(&sent[1]), json!({ "monetizationEnabled": true }));
    assert_eq!(json_body(&sent[2]), json!({ "quota": null }));
    assert_eq!(
        json_body(&sent[3]),
        json!({
            "quota": {
                "monthlyTokenLimit": 5000000,
                "monthlyRequestLimit": null,
                "resetDay": 15
            }
        })
    );
}

#[tokio::test]
async fn get_spending_limit_decodes_the_status_with_and_without_a_limit() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/tenant/spending-limit"))
        .respond_with(json(200, spending_limit_status()))
        .expect(1)
        .mount(&server)
        .await;

    let status = client.tenant().get_spending_limit().await.unwrap();

    let limit = status.spending_limit.unwrap();
    assert_eq!(limit.amount_usd, 10.0);
    assert_eq!(limit.reset_start_date, "2026-10-01");
    assert_eq!(limit.reset_interval, ResetInterval::Monthly);
    let period = status.period.unwrap();
    assert_eq!(period.starts_at.to_string(), "2026-10-01T00:00:00Z");
    assert_eq!(period.ends_at.to_string(), "2026-11-01T00:00:00Z");
    assert_eq!(period.spent_usd, 1.5);
    assert_eq!(period.reserved_usd, 0.5);
    assert_eq!(period.available_usd, 8.0);
    assert_eq!(
        status.next_reset_at.unwrap().to_string(),
        "2026-11-01T00:00:00Z"
    );
}

#[tokio::test]
async fn get_spending_limit_decodes_an_all_null_status() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/tenant/spending-limit"))
        .respond_with(json(
            200,
            json!({ "spendingLimit": null, "period": null, "nextResetAt": null }),
        ))
        .mount(&server)
        .await;

    let status = client.tenant().get_spending_limit().await.unwrap();

    assert!(status.spending_limit.is_none());
    assert!(status.period.is_none());
    assert!(status.next_reset_at.is_none());
}

#[tokio::test]
async fn update_spending_limit_sends_null_or_the_limit() {
    let (server, client) = mock().await;
    Mock::given(method("PUT"))
        .and(path("/v1/tenant/spending-limit"))
        .respond_with(json(200, spending_limit_status()))
        .mount(&server)
        .await;

    client
        .tenant()
        .update_spending_limit(Some(SpendingLimitInput::new(
            10.0,
            "2026-10-01",
            ResetInterval::Monthly,
        )))
        .await
        .unwrap();
    client.tenant().update_spending_limit(None).await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "spendingLimit": {
                "amountUsd": 10.0,
                "resetStartDate": "2026-10-01",
                "resetInterval": "monthly"
            }
        })
    );
    assert_eq!(json_body(&sent[1]), json!({ "spendingLimit": null }));
}

#[tokio::test]
async fn an_unknown_reset_interval_decodes_as_unrecognized() {
    let (server, client) = mock().await;
    let mut body = spending_limit_status();
    body["spendingLimit"]["resetInterval"] = json!("hourly");
    Mock::given(method("GET"))
        .and(path("/v1/tenant/spending-limit"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let status = client.tenant().get_spending_limit().await.unwrap();

    match status.spending_limit.unwrap().reset_interval {
        ResetInterval::Unrecognized(other) => assert_eq!(other.as_str(), "hourly"),
        other => panic!("expected an unrecognized reset interval, got {other:?}"),
    }
}
