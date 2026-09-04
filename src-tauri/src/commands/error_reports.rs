//! The commands the webview drives error reporting through.
//!
//! Thin by design: every decision lives in `crate::error_reports`, which is a
//! plain module with tests, and these wrappers only supply the things a
//! command has that a module function does not -- the app handle, the reports
//! directory, and the session token.

use tauri::State;

use crate::commands::auth::{get_api_base, SessionCache};
use crate::error_reports::{self, ErrorReport, Phase, ReportKind, ReportMode};
use crate::network::reports as net_reports;

fn dir() -> Result<std::path::PathBuf, String> {
    error_reports::active_reports_dir()
        .ok_or_else(|| "Kein Verzeichnis für Fehlerberichte gefunden".to_string())
}

/// Every report still waiting on disk, oldest first, with its consent state.
///
/// The webview calls this on mount, which is how a crash from a previous run
/// is found: the panic hook wrote the file and did nothing else (D5).
#[tauri::command]
pub async fn list_error_reports() -> Result<Vec<ErrorReport>, String> {
    if error_reports::mode() == ReportMode::Never {
        return Ok(Vec::new());
    }
    let reports = error_reports::list_reports(&dir()?);
    if !reports.is_empty() {
        // The queue depth, so a dialog that "came back" can be told apart from
        // the next report in a backlog.
        log::info!("{} Fehlerbericht(e) warten auf eine Entscheidung", reports.len());
    }
    Ok(reports)
}

/// One report, exactly as it sits on disk. The content is already scrubbed --
/// scrubbing happens at creation (D3) -- so what the dialog shows verbatim is
/// what would be sent.
#[tauri::command]
pub async fn read_error_report(report_id: String) -> Result<Option<ErrorReport>, String> {
    let path = dir()?.join(format!("{}.json", report_id));
    Ok(error_reports::read_report(&path))
}

/// Create a report for a failure the webview saw (D6): a command error the UI
/// could not classify as an expected outcome, or an unhandled webview error.
/// Answers `None` when reports are switched off, which is not an error.
#[tauri::command]
pub async fn create_error_report(
    kind: String,
    phase: String,
    message: String,
    job_id: Option<String>,
) -> Result<Option<ErrorReport>, String> {
    let kind = match kind.as_str() {
        "ui_error" => ReportKind::UiError,
        "command_error" => ReportKind::CommandError,
        other => return Err(format!("Unbekannte Berichtsart: {}", other)),
    };
    Ok(error_reports::create(
        kind,
        Phase::from_str_or_unknown(&phase),
        &message,
        job_id,
    ))
}

/// Record the user's answer. Declining deletes the file; nothing has been
/// transmitted at that point, and nothing will be.
#[tauri::command]
pub async fn decide_error_report(
    report_id: String,
    grant: bool,
    comment: Option<String>,
) -> Result<Option<ErrorReport>, String> {
    let outcome = error_reports::decide(&dir()?, &report_id, grant, comment);
    // Logged because the log could not previously answer "did the user decline,
    // or was there simply another report behind it?" -- which is exactly the
    // question a reappearing dialog raises.
    let short: String = report_id.chars().take(8).collect();
    if grant {
        log::info!("Fehlerbericht {} freigegeben, wartet auf Versand", short);
    } else {
        log::info!("Fehlerbericht {} abgelehnt und geloescht", short);
    }
    Ok(outcome)
}

/// The absolute path of a report's file, for the "Bericht-Datei anzeigen"
/// affordance (D7): a user whose sign-in is the thing that failed can reveal
/// the file and mail it instead of waiting for a session that will not come.
/// The webview cannot build this path itself -- the reports directory is
/// resolved on the Rust side and differs per platform.
#[tauri::command]
pub async fn error_report_path(report_id: String) -> Result<String, String> {
    Ok(dir()?
        .join(format!("{}.json", report_id))
        .to_string_lossy()
        .to_string())
}

/// What one submission produced, for the notice the webview shows.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubmittedReport {
    pub report_id: String,
    pub number: String,
}

/// Send every granted report the current session can carry (D7).
///
/// The one mechanism serves both the signed-out case and the retry: a report
/// created while signed out waits with its consent recorded, and a submission
/// that failed is simply still granted the next time this runs. Called after
/// a successful sign-in and after a grant.
///
/// A success deletes the file. A failure keeps it, so the same report id goes
/// out next time and the number the user was shown does not change.
#[tauri::command]
pub async fn submit_error_reports(
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
) -> Result<Vec<SubmittedReport>, String> {
    let reports_dir = dir()?;
    error_reports::sweep_stale(&reports_dir, chrono::Utc::now());

    if error_reports::mode() == ReportMode::Never {
        return Ok(Vec::new());
    }

    // Read the token out before the first await: a `State` guard must not be
    // held across one.
    let access_token = session.access_token.lock().unwrap().clone();
    let Some(access_token) = access_token else {
        // Not signed in yet. The reports stay exactly where they are.
        return Ok(Vec::new());
    };
    let api_base = get_api_base(&app);

    let mut sent = Vec::new();
    for report in error_reports::list_reports(&reports_dir) {
        if report.consent != error_reports::Consent::Granted {
            continue;
        }
        match net_reports::submit_report(&report, api_base, &access_token).await {
            Ok(number) => {
                error_reports::delete_report(&reports_dir, &report.report_id);
                log::info!("Fehlerbericht {} gesendet", number);
                sent.push(SubmittedReport {
                    report_id: report.report_id,
                    number,
                });
            }
            Err(err) => {
                // Kept on disk deliberately: the next sign-in retries it with
                // the same id, so the number the user copied still finds it.
                log::warn!(
                    "Fehlerbericht {} bleibt liegen: {}",
                    report.number(),
                    err
                );
            }
        }
    }
    Ok(sent)
}

/// Record what the user is doing, so a panic knows which screen it happened
/// on (D3's `phase`).
///
/// Only the webview knows this: `login`, `review` and `settings` are screens,
/// not backend states, and Rust cannot see them. Before this existed those
/// three phases were unreachable and every report outside a recording claimed
/// `idle`. The webview derives the value with `phaseForScreen`, which has a
/// table test; an unrecognised string here becomes `Unknown` rather than an
/// error, because a wrong phase must never cost a report.
#[tauri::command]
pub async fn set_error_report_phase(phase: String) -> Result<(), String> {
    error_reports::set_phase(Phase::from_str_or_unknown(&phase));
    Ok(())
}

/// Force a failure on purpose, so the dialog and the on-disk queue can be
/// exercised by hand (tasks 1.2, 1.4 and 3.3 are by-hand checks).
///
/// The failing bodies are behind `debug_assertions`, so a release binary does
/// not contain the panic at all -- it contains a command that refuses. It is
/// registered unconditionally because `generate_handler!` takes a plain list
/// and cannot carry a `cfg` on one entry.
///
/// `kind` selects the failure:
/// - `main_thread_panic` -- panics on the command's own thread. The hook writes
///   the file and does nothing else (D5); the next launch is what finds it.
/// - `background_panic` -- panics on a spawned thread, which is the case the
///   hook must survive without an app handle.
/// - `command_error` -- an ordinary `Err` the classifier treats as a defect,
///   so it becomes a report.
/// - `expected_command_error` -- an ordinary `Err` the classifier recognises as
///   an expected outcome, so it must produce no report.
#[tauri::command]
pub async fn debug_trigger_failure(kind: String, app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(debug_assertions))]
    {
        let _ = (kind, app);
        Err("Nur in Entwicklungsbuilds verfügbar".to_string())
    }
    #[cfg(debug_assertions)]
    {
        match kind.as_str() {
            "main_thread_panic" => {
                // A command body does NOT run on the main thread -- async
                // commands run on the async runtime, so `panic!()` here is
                // just another background panic and the app survives it. That
                // made this button a duplicate of the one below. The event
                // loop is reached through the app handle.
                app.run_on_main_thread(|| panic!("Absichtlicher Testpanic (Haupt-Thread)"))
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            "background_panic" => {
                let done = std::thread::spawn(|| {
                    panic!("Absichtlicher Testpanic (Hintergrund-Thread)")
                });
                // Wait for the hook to have written the file, so the refresh
                // the webview does next actually finds it.
                let _ = done.join();
                Ok(())
            }
            "command_error" => Err(
                "Upload fehlgeschlagen: 500 - {\"error\":\"absichtlicher Testfehler\"}"
                    .to_string(),
            ),
            // The other half of D6: a failure the UI can already explain must
            // produce no report at all. Suppression is the easier half to
            // break without noticing, so it needs a trigger of its own.
            "expected_command_error" => Err("Keine aktive Aufnahme".to_string()),
            other => Err(format!("Unbekannte Testart: {}", other)),
        }
    }
}
