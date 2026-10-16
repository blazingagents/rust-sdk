//! Scheduled and on-demand tasks and their runs. Start from [`Client::tasks`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{AgentId, TaskId, TaskRunId};
use crate::op::Op;
use crate::pagination::{Cursor, Page, Paginated};
use crate::transcript::UiMessage;

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
    /// Lists tasks.
    pub fn list(&self) -> Call<ListTasks> {
        Call::new(self.client, Op::ListTasks, ListTasks::default())
    }

    /// Creates a task.
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
            },
        )
    }

    /// Gets one task.
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

    /// Changes a task. Fields you do not set keep their values.
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

    /// Deletes a task.
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

    /// Starts a run of a task now.
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

    /// Lists a task's runs.
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

    /// Gets one run.
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

    /// Lists a run's messages.
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

    /// Cancels a run.
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
    cursor: Option<Cursor>,
}
impl_request!(ListTasks => Page<Task>; encode = query; decode = json; paginate);

/// Request state of [`Tasks::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    agent_id: AgentId,
    name: String,
    prompt: String,
}
impl_request!(CreateTask => CreatedTask; encode = json; decode = json);

/// Request state of [`Tasks::get`].
#[derive(Clone, Debug)]
pub struct GetTask;
impl_request!(GetTask => Task; encode = none; decode = json);

/// Request state of [`Tasks::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTask {}
impl_request!(UpdateTask => Task; encode = json; decode = json);

/// Request state of [`Tasks::delete`].
#[derive(Clone, Debug)]
pub struct DeleteTask;
impl_request!(DeleteTask => (); encode = none; decode = none);

/// Request state of [`Tasks::submit`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRun {}
impl_request!(CreateTaskRun => CreatedTaskRun; encode = json; decode = json);

/// Request state of [`Tasks::list_runs`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTaskRuns {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListTaskRuns => Page<TaskRun>; encode = query; decode = json; paginate);

/// Request state of [`Tasks::get_run`].
#[derive(Clone, Debug)]
pub struct GetTaskRun;
impl_request!(GetTaskRun => TaskRun; encode = none; decode = json);

/// Request state of [`Tasks::run_messages`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTaskRunMessages {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListTaskRunMessages => RunMessagesPage; encode = query; decode = json; paginate);

/// Request state of [`Tasks::cancel_run`].
#[derive(Clone, Debug)]
pub struct CancelTaskRun;
impl_request!(CancelTaskRun => (); encode = none; decode = none);

/// A task.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Task {}

/// A created task and, when it was submitted at once, its first run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CreatedTask {}

/// A run of a task.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {}

/// A newly started run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CreatedTaskRun {}

/// A page of a run's messages.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct RunMessagesPage {
    /// The messages on this page.
    pub data: Vec<UiMessage>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
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
