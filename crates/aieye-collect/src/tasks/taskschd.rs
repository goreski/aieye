//! Windows Task Scheduler, read through its COM interface.

// Every COM call is a foreign call; each unsafe block says why it is sound.
#![allow(unsafe_code)]

use anyhow::{Context as _, Result};
use jiff::tz::TimeZone;
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, VARIANT_BOOL};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::TaskScheduler::{
    IActionCollection, IComHandlerAction, IDailyTrigger, IExecAction, IMonthlyTrigger,
    IRegisteredTask, ITaskFolder, ITaskService, ITriggerCollection, IWeeklyTrigger,
    TASK_ACTION_COM_HANDLER, TASK_ACTION_EXEC, TASK_ACTION_TYPE, TASK_ENUM_HIDDEN,
    TASK_TRIGGER_BOOT, TASK_TRIGGER_DAILY, TASK_TRIGGER_EVENT, TASK_TRIGGER_IDLE,
    TASK_TRIGGER_LOGON, TASK_TRIGGER_MONTHLY, TASK_TRIGGER_MONTHLYDOW, TASK_TRIGGER_REGISTRATION,
    TASK_TRIGGER_SESSION_STATE_CHANGE, TASK_TRIGGER_TIME, TASK_TRIGGER_TYPE2, TASK_TRIGGER_WEEKLY,
    TaskScheduler,
};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::core::{BSTR, Interface};

use super::{RawAction, RawTask, Trigger, TriggerKind, is_windows_own, ole_time, unquote};

/// Reads every task the signed-in user can see, outside Windows' own folders.
pub(super) fn read() -> Result<Vec<RawTask>> {
    let _com = Com::init()?;
    // SAFETY: COM is initialised on this thread while `_com` lives, and every
    // interface below is dropped before it.
    let root = unsafe {
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)
            .context("cannot open Task Scheduler")?;
        let empty = VARIANT::default();
        service
            .Connect(&empty, &empty, &empty, &empty)
            .context("cannot connect to Task Scheduler")?;
        service
            .GetFolder(&BSTR::from("\\"))
            .context("cannot open Task Scheduler's root folder")?
    };
    let zone = TimeZone::system();
    let mut tasks = Vec::new();
    walk(&root, &zone, &mut tasks);
    Ok(tasks)
}

/// COM for the current thread, released when dropped.
struct Com {
    initialized: bool,
}

impl Com {
    fn init() -> Result<Self> {
        // SAFETY: initialising COM has no preconditions; `Drop` balances it.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            // The thread already uses COM in another mode, which works as well.
            return Ok(Self { initialized: false });
        }
        result.ok().context("cannot initialise COM")?;
        Ok(Self { initialized: true })
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: balances the successful `CoInitializeEx` in `init`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Reads the tasks in `folder` and its subfolders. What cannot be read is logged and
/// skipped.
fn walk(folder: &ITaskFolder, zone: &TimeZone, tasks: &mut Vec<RawTask>) {
    // SAFETY (all blocks in this function): `folder` and the collections it returns
    // are live interfaces from the Task Scheduler service.
    let path = unsafe { folder.Path() }
        .map(|path| path.to_string())
        .unwrap_or_default();
    if is_windows_own(&path) {
        return;
    }
    match unsafe { folder.GetTasks(TASK_ENUM_HIDDEN.0) } {
        Ok(found) => {
            let count = unsafe { found.Count() }.unwrap_or(0);
            for index in 1..=count {
                match unsafe { found.get_Item(&variant(index)) }
                    .and_then(|task| read_task(&task, zone))
                {
                    Ok(task) => tasks.push(task),
                    Err(error) => {
                        tracing::warn!(folder = %path, %error, "skipping a scheduled task")
                    }
                }
            }
        }
        Err(error) => tracing::warn!(folder = %path, %error, "cannot list scheduled tasks"),
    }
    match unsafe { folder.GetFolders(0) } {
        Ok(folders) => {
            let count = unsafe { folders.Count() }.unwrap_or(0);
            for index in 1..=count {
                match unsafe { folders.get_Item(&variant(index)) } {
                    Ok(subfolder) => walk(&subfolder, zone, tasks),
                    Err(error) => {
                        tracing::warn!(folder = %path, %error, "cannot open a task folder")
                    }
                }
            }
        }
        Err(error) => tracing::warn!(folder = %path, %error, "cannot list task folders"),
    }
}

fn read_task(task: &IRegisteredTask, zone: &TimeZone) -> windows::core::Result<RawTask> {
    // SAFETY: `task` and every interface reached from it are live interfaces from the
    // service; out-parameters are local values the calls fill in.
    unsafe {
        let definition = task.Definition()?;
        let info = definition.RegistrationInfo()?;
        let principal = definition.Principal()?;
        Ok(RawTask {
            path: task.Path()?.to_string(),
            enabled: is_true(task.Enabled()?),
            author: text(|out| info.Author(out)),
            run_as: text(|out| principal.UserId(out))
                .or_else(|| text(|out| principal.GroupId(out))),
            actions: read_actions(&definition.Actions()?)?,
            triggers: read_triggers(&definition.Triggers()?)?,
            last_run: task
                .LastRunTime()
                .ok()
                .and_then(|days| ole_time(days, zone)),
            next_run: task
                .NextRunTime()
                .ok()
                .and_then(|days| ole_time(days, zone)),
        })
    }
}

fn read_actions(actions: &IActionCollection) -> windows::core::Result<Vec<RawAction>> {
    // SAFETY: as in `read_task`. The collection counts from 1.
    unsafe {
        let mut count = 0;
        actions.Count(&mut count)?;
        let mut found = Vec::new();
        for index in 1..=count {
            let action = actions.get_Item(index)?;
            let mut kind = TASK_ACTION_TYPE::default();
            action.Type(&mut kind)?;
            found.push(if kind == TASK_ACTION_EXEC {
                let exec: IExecAction = action.cast()?;
                RawAction {
                    program: text(|out| exec.Path(out))
                        .map(|program| unquote(&program).to_owned())
                        .unwrap_or_default(),
                    arguments: text(|out| exec.Arguments(out)),
                    folder: text(|out| exec.WorkingDirectory(out))
                        .map(|folder| unquote(&folder).to_owned()),
                }
            } else if kind == TASK_ACTION_COM_HANDLER {
                let handler: IComHandlerAction = action.cast()?;
                let class = text(|out| handler.ClassId(out)).unwrap_or_default();
                RawAction {
                    program: format!("COM handler {class}"),
                    arguments: text(|out| handler.Data(out)),
                    folder: None,
                }
            } else {
                RawAction {
                    program: "e-mail or message action".to_owned(),
                    arguments: None,
                    folder: None,
                }
            });
        }
        Ok(found)
    }
}

fn read_triggers(triggers: &ITriggerCollection) -> windows::core::Result<Vec<Trigger>> {
    // SAFETY: as in `read_task`. The collection counts from 1.
    unsafe {
        let mut count = 0;
        triggers.Count(&mut count)?;
        let mut found = Vec::new();
        for index in 1..=count {
            let trigger = triggers.get_Item(index)?;
            let mut kind = TASK_TRIGGER_TYPE2::default();
            trigger.Type(&mut kind)?;
            let mut enabled = VARIANT_BOOL::default();
            trigger.Enabled(&mut enabled)?;
            let mut read = Trigger {
                kind: trigger_kind(kind),
                start: text(|out| trigger.StartBoundary(out)),
                interval: None,
                days: None,
                repeat: trigger
                    .Repetition()
                    .ok()
                    .and_then(|repetition| text(|out| repetition.Interval(out))),
                enabled: is_true(enabled),
            };
            if kind == TASK_TRIGGER_DAILY
                && let Ok(daily) = trigger.cast::<IDailyTrigger>()
            {
                let mut days = 0;
                read.interval = daily.DaysInterval(&mut days).ok().map(|()| days);
            } else if kind == TASK_TRIGGER_WEEKLY
                && let Ok(weekly) = trigger.cast::<IWeeklyTrigger>()
            {
                let (mut days, mut weeks) = (0, 0);
                read.days = weekly.DaysOfWeek(&mut days).ok().map(|()| i32::from(days));
                read.interval = weekly.WeeksInterval(&mut weeks).ok().map(|()| weeks);
            } else if kind == TASK_TRIGGER_MONTHLY
                && let Ok(monthly) = trigger.cast::<IMonthlyTrigger>()
            {
                let mut days = 0;
                read.days = monthly.DaysOfMonth(&mut days).ok().map(|()| days);
            }
            found.push(read);
        }
        Ok(found)
    }
}

fn trigger_kind(kind: TASK_TRIGGER_TYPE2) -> TriggerKind {
    match kind {
        TASK_TRIGGER_EVENT => TriggerKind::Event,
        TASK_TRIGGER_TIME => TriggerKind::Once,
        TASK_TRIGGER_DAILY => TriggerKind::Daily,
        TASK_TRIGGER_WEEKLY => TriggerKind::Weekly,
        TASK_TRIGGER_MONTHLY => TriggerKind::Monthly,
        TASK_TRIGGER_MONTHLYDOW => TriggerKind::MonthlyByWeekday,
        TASK_TRIGGER_IDLE => TriggerKind::Idle,
        TASK_TRIGGER_REGISTRATION => TriggerKind::Registration,
        TASK_TRIGGER_BOOT => TriggerKind::Boot,
        TASK_TRIGGER_LOGON => TriggerKind::Logon,
        TASK_TRIGGER_SESSION_STATE_CHANGE => TriggerKind::SessionChange,
        _ => TriggerKind::Other,
    }
}

/// Calls a getter that fills in a string, and returns the text unless the call
/// failed or the text is empty.
fn text(getter: impl FnOnce(*mut BSTR) -> windows::core::Result<()>) -> Option<String> {
    let mut value = BSTR::new();
    getter(&mut value).ok()?;
    let value = value.to_string();
    (!value.trim().is_empty()).then_some(value)
}

fn is_true(value: VARIANT_BOOL) -> bool {
    value.0 != 0
}

/// A VARIANT holding a 32-bit integer, as collection indexes need.
fn variant(value: i32) -> VARIANT {
    let mut variant = VARIANT::default();
    // SAFETY: sets the type tag and the matching member of a zeroed VARIANT. An
    // integer VARIANT owns nothing, so it needs no `VariantClear`.
    unsafe {
        let inner = &mut variant.Anonymous.Anonymous;
        inner.vt = VT_I4;
        inner.Anonymous.lVal = value;
    }
    variant
}
