//! The notifications: the list ito-notices hangs from the bell. Each row is
//! "repository #number title" with what happened under it (the inbox's
//! reason when the thread is a card there, else why GitHub sent it) and the
//! age. The newest is on top; the cursor is `app.ns.sel`, kept in step with
//! the list's own (its arrows move it, `j` `k` move ours).

use std::time::{Duration, UNIX_EPOCH};

use egui::Ui;
use ito_notices::{Action, Kind, Notice, Words};
use minato_core::list::epoch;
use minato_core::notes::{self, Tone};

use crate::app::{App, NoteClick};
use crate::ui::Out;

/// The list's own cursor, as ito-notices keeps it in the context.
fn cursor_id() -> egui::Id {
    egui::Id::new("ito-notices-row")
}

/// The notices as ito-notices wants them: oldest first, `go` the index in
/// `app.notes` (which is newest first).
fn notices(app: &App) -> Vec<Notice<usize>> {
    app.notes
        .iter()
        .enumerate()
        .rev()
        .map(|(k, n)| {
            let (_, card) = app.note_place(&n.url);
            let (what, tone) = match card.and_then(|c| c.rule) {
                Some(rule) => (app.tr(rule.why()), rule.tone()),
                None => {
                    let (key, tone) = notes::event(&n.reason);
                    (app.tr(key), tone)
                }
            };
            let repo = n.repo.rsplit('/').next().unwrap_or(&n.repo);
            let title = match notes::number(&n.url) {
                Some(num) => format!("{repo} #{num} {}", n.title),
                None => format!("{repo} {}", n.title),
            };
            let in_inbox = app.note_place(&n.url).1.is_some();
            let note = if in_inbox { format!("{what} · {}", app.tr("notes.inbox")) } else { what.to_owned() };
            let secs = epoch(&n.updated_at).unwrap_or(0).max(0) as u64;
            Notice {
                id: k as u64,
                at: UNIX_EPOCH + Duration::from_secs(secs),
                kind: kind(tone),
                title,
                note: Some(note),
                go: Some(k),
                read: false,
            }
        })
        .collect()
}

fn kind(t: Tone) -> Kind {
    match t {
        Tone::Bad => Kind::Err,
        Tone::Ok => Kind::Ok,
        Tone::Ask => Kind::Wait,
        Tone::Run | Tone::Idle | Tone::Accent => Kind::Info,
    }
}

pub fn show(ui: &Ui, app: &App, out: &mut Out) {
    let ctx = ui.ctx().clone();
    let Some(bell) = out.bell_at else {
        return;
    };
    let list = notices(app);
    if !list.is_empty() {
        let sel = app.ns.sel.min(list.len() - 1);
        ctx.data_mut(|d| d.insert_temp(cursor_id(), Some(sel)));
    }
    let title = format!("{} ({})", app.tr("notes.title"), app.tr("notes.unread").replace("{}", &list.len().to_string()));
    let words = Words {
        title: &title,
        mark_read: app.tr("notes.mark_read"),
        empty: app.tr("notes.empty"),
        ago: app.tr("notes.ago"),
        kinds: [
            app.tr("notes.kind.ok"),
            app.tr("notes.kind.info"),
            app.tr("notes.kind.warn"),
            app.tr("notes.kind.err"),
            app.tr("notes.kind.wait"),
        ],
    };
    let action = ito_notices::show(&ctx, bell, &list, &words);
    out.note = match action {
        Some(Action::Open(Some(k), _)) => Some(NoteClick::Open(k)),
        Some(Action::Open(None, _)) => None,
        Some(Action::ReadAll) => Some(NoteClick::ReadAll),
        // A click on the bell itself toggles it.
        Some(Action::Close) if out.bell => None,
        Some(Action::Close) => Some(NoteClick::Close),
        None => None,
    };
}

/// The list's cursor after its arrows moved it, for `app.ns`.
pub fn cursor(ctx: &egui::Context) -> Option<usize> {
    ctx.data(|d| d.get_temp::<Option<usize>>(cursor_id())).flatten()
}
