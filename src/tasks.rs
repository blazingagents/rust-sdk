//! Saved prompts an agent runs in the background, on demand or on a schedule.
//! Start from [`Client::tasks`].
//!
//! Documentation: <https://docs.blazingagents.com/automation/tasks>.

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::agents::AgentConfig;
use crate::call::{Call, Paginate, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{AgentId, SessionId, TaskId, TaskRunId, TenantId, TurnId};
use crate::op::Op;
use crate::pagination::{Cursor, Page, Paginated};
use crate::patch::Patch;
use crate::transcript::UiMessage;
use crate::types::{Metadata, Timestamp};

/// Task operations. Returned by [`Client::tasks`].
#[derive(Clone, Copy, Debug)]
pub struct Tasks<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Task operations.
    pub fn tasks(&self) -> Tasks<'_, S> {
        Tasks { client: self }
    }
}

impl<S: Scope> Tasks<'_, S> {
    /// Lists tasks, most recently updated first. Each task's
    /// [`latest_run`](TaskListItem::latest_run) carries its most recent run's
    /// status.
    pub fn list(&self) -> Call<ListTasks> {
        Call::new(self.client, Op::ListTasks, ListTasks::default())
    }

    /// Creates a task: a saved prompt the agent runs in the background.
    /// Without a schedule the task runs only when you start it; with
    /// [`submit`](Call::submit) the first run starts at once and the
    /// response's [`run_id`](CreatedTask::run_id) is set.
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// use blazing_agents::tasks::Schedule;
    /// let created = client
    ///     .tasks()
    ///     .create(
    ///         "ag_4kP9sT2vXq7LmN3a",
    ///         "Daily support summary",
    ///         "Summarize yesterday's open support cases.",
    ///     )
    ///     .schedule(Schedule::cron("0 9 * * 1-5").timezone("Europe/London"))
    ///     .submit(true)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(
        &self,
        agent_id: impl Into<AgentId>,
        name: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Call<CreateTask> {
        Call::new(
            self.client,
            Op::CreateTask,
            CreateTask {
                agent_id: agent_id.into(),
                name: name.into(),
                prompt: prompt.into(),
                idempotency_key: None,
                schedule: None,
                enabled: None,
                submit: None,
                user_id: None,
                metadata: None,
            },
        )
    }

    /// Gets one task without running it.
    pub fn get(&self, task_id: impl Into<TaskId>) -> Call<GetTask> {
        let task_id = task_id.into();
        Call::new(
            self.client,
            Op::GetTask {
                task_id: task_id.as_str(),
            },
            GetTask,
        )
    }

    /// Changes a task. Send at least one field; fields you do not set keep
    /// their values, and a run already in progress keeps the settings it
    /// started with. The agent and `user_id` cannot change.
    pub fn update(&self, task_id: impl Into<TaskId>) -> Call<UpdateTask> {
        let task_id = task_id.into();
        Call::new(
            self.client,
            Op::UpdateTask {
                task_id: task_id.as_str(),
            },
            UpdateTask::default(),
        )
    }

    /// Deletes a task and stops its schedule. Its past runs and their
    /// transcripts are kept. A task with a queued or running run cannot be
    /// deleted.
    pub fn delete(&self, task_id: impl Into<TaskId>) -> Call<DeleteTask> {
        let task_id = task_id.into();
        Call::new(
            self.client,
            Op::DeleteTask {
                task_id: task_id.as_str(),
            },
            DeleteTask,
        )
    }

    /// Starts a run of the task now, whether or not it has a schedule. A task
    /// runs one run at a time, so this returns `409 task_active_run_exists`
    /// while another run is queued or running.
    pub fn submit(&self, task_id: impl Into<TaskId>) -> Call<CreateTaskRun> {
        let task_id = task_id.into();
        Call::new(
            self.client,
            Op::CreateTaskRun {
                task_id: task_id.as_str(),
            },
            CreateTaskRun::default(),
        )
    }

    /// Lists a task's runs, newest first.
    pub fn list_runs(&self, task_id: impl Into<TaskId>) -> Call<ListTaskRuns> {
        let task_id = task_id.into();
        Call::new(
            self.client,
            Op::ListTaskRuns {
                task_id: task_id.as_str(),
            },
            ListTaskRuns::default(),
        )
    }

    /// Gets one run's current state and the agent configuration captured when
    /// it was queued. Poll it until [`status`](TaskRun::status) is final.
    pub fn get_run(
        &self,
        task_id: impl Into<TaskId>,
        run_id: impl Into<TaskRunId>,
    ) -> Call<GetTaskRun> {
        let task_id = task_id.into();
        let run_id = run_id.into();
        Call::new(
            self.client,
            Op::GetTaskRun {
                task_id: task_id.as_str(),
                run_id: run_id.as_str(),
            },
            GetTaskRun,
        )
    }

    /// Returns a run's transcript with the run's `status`, `error` and
    /// `finished_at`, so one request says both what the agent said and whether
    /// the run is done. The page is empty until the run starts its session.
    /// Pages walk backward with `cursor`; to poll for newer messages use
    /// [`after`](Call::after) instead — never both.
    pub fn run_messages(
        &self,
        task_id: impl Into<TaskId>,
        run_id: impl Into<TaskRunId>,
    ) -> Call<ListTaskRunMessages> {
        let task_id = task_id.into();
        let run_id = run_id.into();
        Call::new(
            self.client,
            Op::ListTaskRunMessages {
                task_id: task_id.as_str(),
                run_id: run_id.as_str(),
            },
            ListTaskRunMessages::default(),
        )
    }

    /// Asks a queued or running run to stop; poll [`get_run`](Self::get_run)
    /// for its `canceled` status. It returns `204` whenever the task exists,
    /// even if the run is missing or already finished.
    pub fn cancel_run(
        &self,
        task_id: impl Into<TaskId>,
        run_id: impl Into<TaskRunId>,
    ) -> Call<CancelTaskRun> {
        let task_id = task_id.into();
        let run_id = run_id.into();
        Call::new(
            self.client,
            Op::CancelTaskRun {
                task_id: task_id.as_str(),
                run_id: run_id.as_str(),
            },
            CancelTaskRun,
        )
    }
}

/// Request state of [`Tasks::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTasks {
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListTasks => Page<TaskListItem>; encode = query; decode = json; paginate);

impl Call<ListTasks> {
    /// Only tasks for this agent.
    pub fn agent_id(self, agent_id: impl Into<AgentId>) -> Self {
        self.map(|request| request.agent_id = Some(agent_id.into()))
    }

    /// Only tasks for this end user. An empty string selects tenant-level
    /// tasks.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Tasks per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Tasks::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    agent_id: AgentId,
    name: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schedule: Option<Schedule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    submit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}
impl_request!(CreateTask => CreatedTask; encode = json; decode = json);

impl Call<CreateTask> {
    /// Retries this creation safely: a repeated request with the same key
    /// returns the task and its original initial run instead of creating
    /// another task. Changed fields or a deleted task return `409`.
    pub fn idempotency_key(self, idempotency_key: impl Into<String>) -> Self {
        self.map(|request| request.idempotency_key = Some(idempotency_key.into()))
    }

    /// When the task runs by itself. Omit it for an on-demand task: on create
    /// the API treats a missing `schedule` and `null` the same, which is why
    /// there is no `clear_schedule` here.
    pub fn schedule(self, schedule: Schedule) -> Self {
        self.map(|request| request.schedule = Some(schedule))
    }

    /// Whether the schedule fires. Defaults to `true`; a disabled task can
    /// still be started on demand.
    pub fn enabled(self, enabled: bool) -> Self {
        self.map(|request| request.enabled = Some(enabled))
    }

    /// Starts the first run as soon as the task is created: the response is
    /// `202` and its [`run_id`](CreatedTask::run_id) is set.
    pub fn submit(self, submit: bool) -> Self {
        self.map(|request| request.submit = Some(submit))
    }

    /// The end user the task belongs to, copied to every run. Fixed after
    /// creation; an empty string (the default) means a tenant-level task.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Your own data, copied to every run.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }
}

/// Request state of [`Tasks::get`].
#[derive(Clone, Debug)]
pub struct GetTask;
impl_request!(GetTask => Task; encode = none; decode = json);

/// Request state of [`Tasks::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    schedule: Patch<Schedule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}
impl_request!(UpdateTask => Task; encode = json; decode = json);

impl Call<UpdateTask> {
    /// A new display name, 1 to 80 characters.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Some(name.into()))
    }

    /// The prompt the agent receives on every run, 1 to 6,000 characters.
    pub fn prompt(self, prompt: impl Into<String>) -> Self {
        self.map(|request| request.prompt = Some(prompt.into()))
    }

    /// A new schedule, replacing the current one.
    pub fn schedule(self, schedule: Schedule) -> Self {
        self.map(|request| request.schedule = Patch::Set(schedule))
    }

    /// Removes the schedule: the task runs only when you start it.
    pub fn clear_schedule(self) -> Self {
        self.map(|request| request.schedule = Patch::Clear)
    }

    /// Whether the schedule fires. A disabled task can still be started on
    /// demand.
    pub fn enabled(self, enabled: bool) -> Self {
        self.map(|request| request.enabled = Some(enabled))
    }

    /// Your own data. Replaces the current metadata completely.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }
}

/// Request state of [`Tasks::delete`].
#[derive(Clone, Debug)]
pub struct DeleteTask;
impl_request!(DeleteTask => (); encode = none; decode = none);

/// Request state of [`Tasks::submit`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRun {
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotency_key: Option<String>,
}
impl_request!(CreateTaskRun => CreatedTaskRun; encode = json; decode = json);

impl Call<CreateTaskRun> {
    /// Makes retries safe: repeating the request with the same key returns the
    /// same run instead of starting another.
    pub fn idempotency_key(self, idempotency_key: impl Into<String>) -> Self {
        self.map(|request| request.idempotency_key = Some(idempotency_key.into()))
    }
}

/// Request state of [`Tasks::list_runs`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTaskRuns {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListTaskRuns => Page<TaskRun>; encode = query; decode = json; paginate);

impl Call<ListTaskRuns> {
    /// Runs per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Tasks::get_run`].
#[derive(Clone, Debug)]
pub struct GetTaskRun;
impl_request!(GetTaskRun => TaskRun; encode = none; decode = json);

/// Request state of [`Tasks::run_messages`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTaskRunMessages {
    #[serde(flatten)]
    position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListTaskRunMessages => RunMessagesPage; encode = query; decode = json);

/// Where a [`Tasks::run_messages`] page starts, and in which direction it
/// walks, so a page started with `after` keeps polling forward through
/// [`Call::items`]: the API sends `cursor` or `after`, never both.
#[derive(Clone, Debug)]
enum Position {
    Before(Cursor),
    After(Cursor),
}

impl Serialize for Position {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Before(cursor) => map.serialize_entry("cursor", cursor)?,
            Self::After(cursor) => map.serialize_entry("after", cursor)?,
        }
        map.end()
    }
}

impl Paginate for ListTaskRunMessages {
    fn set_cursor(&mut self, cursor: Cursor) {
        self.position = Some(match &self.position {
            Some(Position::After(_)) => Position::After(cursor),
            _ => Position::Before(cursor),
        });
    }
}

impl Call<ListTaskRunMessages> {
    /// Polls forward: returns only messages newer than `cursor`, typically the
    /// previous page's [`latest_cursor`](RunMessagesPage::latest_cursor). When
    /// a forward page has more, pass its `nextCursor` as the next `after` —
    /// the generic [`cursor`](Call::cursor) used by [`items`](Call::items)
    /// and [`pages`](Call::pages) keeps this direction. `cursor` and `after`
    /// are never sent together.
    pub fn after(self, cursor: impl Into<Cursor>) -> Self {
        self.map(|request| request.position = Some(Position::After(cursor.into())))
    }

    /// Messages per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Tasks::cancel_run`].
#[derive(Clone, Debug)]
pub struct CancelTaskRun;
impl_request!(CancelTaskRun => (); encode = none; decode = none);

crate::open_enum! {
    /// Where a task run is in its lifecycle.
    pub enum TaskRunStatus {
        /// `queued`: waiting to start.
        Queued = "queued",
        /// `running`: in progress.
        Running = "running",
        /// `blocked`: not allowed to start, for example because a quota was
        /// reached.
        Blocked = "blocked",
        /// `succeeded`: finished successfully.
        Succeeded = "succeeded",
        /// `failed`: finished with an error.
        Failed = "failed",
        /// `canceled`: stopped before finishing.
        Canceled = "canceled",
    }
}

/// When a task runs by itself. A `kind` this version of the SDK does not know
/// decodes as [`Schedule::Unrecognized`] instead of failing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Schedule {
    /// Runs once, at a set instant (`{"kind": "once", "config": {"at": "..."}}`).
    #[non_exhaustive]
    Once {
        /// When the run starts, as an RFC 3339 instant.
        at: Timestamp,
    },
    /// Runs on a fixed gap (`{"kind": "interval", "config": {"everyMs": ...}}`).
    #[non_exhaustive]
    Interval {
        /// The gap between runs in milliseconds, at least 60,000.
        every_ms: u64,
    },
    /// Runs on a cron expression
    /// (`{"kind": "cron", "config": {"expression": "...", ...}}`).
    #[non_exhaustive]
    Cron {
        /// The five-field cron expression, 1 to 120 characters.
        expression: String,
        /// The IANA timezone of the expression. The server defaults it to
        /// `UTC`.
        timezone: Option<String>,
        /// The most each run is delayed, in milliseconds. Each task gets its
        /// own fixed delay within it.
        stagger_ms: Option<u64>,
    },
    /// A kind this version of the SDK does not know, kept exactly as the
    /// server sent it.
    Unrecognized(UnrecognizedSchedule),
}

impl Schedule {
    /// Runs once at `at`.
    pub fn once(at: Timestamp) -> Self {
        Self::Once { at }
    }

    /// Runs every `every_ms` milliseconds, at least 60,000.
    pub fn interval(every_ms: u64) -> Self {
        Self::Interval { every_ms }
    }

    /// Runs on a five-field cron expression in an IANA timezone. The timezone
    /// defaults to `UTC` on the server; set it with
    /// [`timezone`](Self::timezone).
    pub fn cron(expression: impl Into<String>) -> Self {
        Self::Cron {
            expression: expression.into(),
            timezone: None,
            stagger_ms: None,
        }
    }

    /// The IANA timezone a `cron` schedule uses, such as `Europe/London`. On
    /// other kinds this does nothing.
    #[must_use]
    pub fn timezone(mut self, timezone: impl Into<String>) -> Self {
        if let Self::Cron {
            timezone: field, ..
        } = &mut self
        {
            *field = Some(timezone.into());
        }
        self
    }

    /// Delays each `cron` run by up to `stagger_ms` milliseconds. Each task
    /// gets its own fixed delay within it, so tasks that share a cron time do
    /// not all start at once. On other kinds this does nothing.
    #[must_use]
    pub fn stagger_ms(mut self, stagger_ms: u64) -> Self {
        if let Self::Cron {
            stagger_ms: field, ..
        } = &mut self
        {
            *field = Some(stagger_ms);
        }
        self
    }
}

/// A schedule kind this version of the SDK does not know, kept exactly as the
/// server sent it so sending it back — for example in [`Tasks::update`] —
/// round-trips it unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnrecognizedSchedule {
    raw: serde_json::Map<String, serde_json::Value>,
}

impl UnrecognizedSchedule {
    /// The `kind` string the server sent.
    pub fn kind(&self) -> &str {
        self.raw
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    }

    /// The whole schedule object as received.
    pub fn as_json(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.raw
    }
}

/// The `kind`-tagged wire shape of a known [`Schedule`].
#[derive(Serialize)]
struct ScheduleWire {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct OnceConfig {
    at: Timestamp,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntervalConfig {
    every_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CronConfig {
    expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stagger_ms: Option<u64>,
}

impl Serialize for Schedule {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = match self {
            Self::Once { at } => ScheduleWire {
                kind: "once".to_owned(),
                config: Some(
                    serde_json::to_value(OnceConfig { at: *at })
                        .map_err(serde::ser::Error::custom)?,
                ),
            },
            Self::Interval { every_ms } => ScheduleWire {
                kind: "interval".to_owned(),
                config: Some(
                    serde_json::to_value(IntervalConfig {
                        every_ms: *every_ms,
                    })
                    .map_err(serde::ser::Error::custom)?,
                ),
            },
            Self::Cron {
                expression,
                timezone,
                stagger_ms,
            } => ScheduleWire {
                kind: "cron".to_owned(),
                config: Some(
                    serde_json::to_value(CronConfig {
                        expression: expression.clone(),
                        timezone: timezone.clone(),
                        stagger_ms: *stagger_ms,
                    })
                    .map_err(serde::ser::Error::custom)?,
                ),
            },
            Self::Unrecognized(other) => return other.raw.serialize(serializer),
        };
        wire.serialize(serializer)
    }
}

/// The `config` of a known schedule kind, required on the wire.
fn schedule_config<E: serde::de::Error>(
    raw: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, E> {
    raw.get("config")
        .cloned()
        .ok_or_else(|| E::missing_field("config"))
}

impl<'de> Deserialize<'de> for Schedule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::types::decode_tagged(
            deserializer,
            "kind",
            &["once", "interval", "cron"],
            |kind, raw| {
                let config = schedule_config(&raw)?;
                match kind {
                    "once" => serde_json::from_value::<OnceConfig>(config)
                        .map(|config| Self::Once { at: config.at }),
                    "interval" => serde_json::from_value::<IntervalConfig>(config).map(|config| {
                        Self::Interval {
                            every_ms: config.every_ms,
                        }
                    }),
                    "cron" => {
                        serde_json::from_value::<CronConfig>(config).map(|config| Self::Cron {
                            expression: config.expression,
                            timezone: config.timezone,
                            stagger_ms: config.stagger_ms,
                        })
                    }
                    _ => unreachable!("`known` names every tag this closure sees"),
                }
            },
            |raw| Self::Unrecognized(UnrecognizedSchedule { raw }),
        )
    }
}

/// A task: a saved prompt an agent runs in the background.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// The task's ID.
    pub id: TaskId,
    /// The tenant that owns the task.
    pub tenant_id: TenantId,
    /// The agent that runs the task.
    pub agent_id: AgentId,
    /// The display name.
    pub name: String,
    /// The prompt the agent receives on every run.
    pub prompt: String,
    /// When the task runs by itself, or `None` for on demand only.
    pub schedule: Option<Schedule>,
    /// Whether the schedule fires. A disabled task can still be started on
    /// demand.
    pub enabled: bool,
    /// The run in progress, if any.
    pub active_run_id: Option<TaskRunId>,
    /// The most recent run, if any.
    pub latest_run_id: Option<TaskRunId>,
    /// The end user, or an empty string for a tenant-level task.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the task was deleted, if it was.
    pub deleted_at: Option<Timestamp>,
    /// When the task was created.
    pub created_at: Timestamp,
    /// When the task last changed.
    pub updated_at: Timestamp,
    /// When the schedule next starts a run, or `None` when nothing is
    /// scheduled.
    pub next_fire_at: Option<Timestamp>,
}

/// A task in a [`Tasks::list`] page: the task plus the status of its most
/// recent run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskListItem {
    /// The task's ID.
    pub id: TaskId,
    /// The tenant that owns the task.
    pub tenant_id: TenantId,
    /// The agent that runs the task.
    pub agent_id: AgentId,
    /// The display name.
    pub name: String,
    /// The prompt the agent receives on every run.
    pub prompt: String,
    /// When the task runs by itself, or `None` for on demand only.
    pub schedule: Option<Schedule>,
    /// Whether the schedule fires. A disabled task can still be started on
    /// demand.
    pub enabled: bool,
    /// The run in progress, if any.
    pub active_run_id: Option<TaskRunId>,
    /// The most recent run, if any.
    pub latest_run_id: Option<TaskRunId>,
    /// The end user, or an empty string for a tenant-level task.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the task was deleted, if it was.
    pub deleted_at: Option<Timestamp>,
    /// When the task was created.
    pub created_at: Timestamp,
    /// When the task last changed.
    pub updated_at: Timestamp,
    /// When the schedule next starts a run, or `None` when nothing is
    /// scheduled.
    pub next_fire_at: Option<Timestamp>,
    /// The most recent run, or `None` if the task never ran.
    pub latest_run: Option<LatestRun>,
}

/// The most recent run of a listed task.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct LatestRun {
    /// The run's ID.
    pub id: TaskRunId,
    /// The run's status.
    pub status: TaskRunStatus,
    /// When the run finished, if it did.
    pub finished_at: Option<Timestamp>,
}

/// A created task and, when it was submitted at once, its first run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CreatedTask {
    /// The task.
    pub task: Task,
    /// The first run's ID, set only when the create request submitted one.
    pub run_id: Option<TaskRunId>,
}

/// A run of a task.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    /// The run's ID.
    pub id: TaskRunId,
    /// The task it runs.
    pub task_id: TaskId,
    /// The tenant that owns the run.
    pub tenant_id: TenantId,
    /// The agent that runs it.
    pub agent_id: AgentId,
    /// The agent configuration captured when the run was queued. Later
    /// changes to the agent do not change it.
    pub agent_config: AgentConfig,
    /// The run's session, `None` until it starts one.
    pub session_id: Option<SessionId>,
    /// The run's metered turn, `None` until it starts.
    pub turn_id: Option<TurnId>,
    /// Where the run is in its lifecycle.
    pub status: TaskRunStatus,
    /// Why a `failed` or `blocked` run did not complete, if known.
    pub error: Option<String>,
    /// The end user, copied from the task.
    pub user_id: String,
    /// Your own data, copied from the task.
    pub metadata: Metadata,
    /// When the run started, if it did.
    pub started_at: Option<Timestamp>,
    /// When the run reached a final status, if it did.
    pub finished_at: Option<Timestamp>,
    /// When cancellation was requested, if it was.
    pub cancel_requested_at: Option<Timestamp>,
    /// When the run was canceled, if it was.
    pub canceled_at: Option<Timestamp>,
    /// When the run was queued.
    pub created_at: Timestamp,
    /// When the run last changed.
    pub updated_at: Timestamp,
}

/// A newly started run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CreatedTaskRun {
    /// The queued run's ID.
    pub run_id: TaskRunId,
}

/// A page of a run's messages, with the run's own state so one request tells
/// you both what the agent said and whether the run is done.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct RunMessagesPage {
    /// The messages on this page, in chronological order.
    pub data: Vec<UiMessage>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
    /// The cursor of the newest message on the page, for polling forward with
    /// [`after`](Call::after).
    pub latest_cursor: Option<Cursor>,
    /// The run's status.
    pub status: TaskRunStatus,
    /// Why the run failed or was blocked, if known.
    pub error: Option<String>,
    /// When the run reached a final status, if it did.
    pub finished_at: Option<Timestamp>,
}

impl Paginated for RunMessagesPage {
    type Item = UiMessage;

    fn next_cursor(&self) -> Option<&Cursor> {
        self.next_cursor.as_ref()
    }

    fn into_items(self) -> Vec<UiMessage> {
        self.data
    }
}
