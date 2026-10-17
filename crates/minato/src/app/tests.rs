use super::*;

fn app() -> App {
    App::new(Prefs::empty())
}

fn run(id: u64, status: actions::Status, pr: Option<u64>) -> Run {
    Run {
        id,
        repo: "uchmk/kura".into(),
        workflow_id: 5,
        workflow: "check".into(),
        number: id,
        title: "t".into(),
        event: "push".into(),
        branch: "main".into(),
        sha: "abc".into(),
        actor: "u".into(),
        status,
        started_at: "2026-10-12T09:00:00Z".into(),
        updated_at: "2026-10-12T09:01:00Z".into(),
        url: format!("https://github.com/uchmk/kura/actions/runs/{id}"),
        pr,
        path: ".github/workflows/check.yml".into(),
        head_sha: "abc0000".into(),
    }
}

#[test]
fn the_keybar_shows_the_run_keys_that_apply() {
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Bad, Some(9)), run(2, actions::Status::Running, None), run(3, actions::Status::Ok, None)];
    let words = |a: &App| a.keys_now().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
    let w = words(&a);
    assert!(w.contains(&"key.rerun") && !w.contains(&"key.cancel") && w.contains(&"run.open_pr"));
    a.feed('j');
    let w = words(&a);
    assert!(!w.contains(&"key.rerun") && w.contains(&"key.cancel") && !w.contains(&"run.open_pr"));
    a.feed('j');
    let w = words(&a);
    assert!(!w.contains(&"key.rerun") && !w.contains(&"key.cancel") && w.contains(&"key.dispatch"));
}

#[test]
fn the_keybar_follows_what_is_open() {
    let mut a = with_pr();
    let bar = |a: &App| a.bar_keys().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
    assert_eq!(bar(&a).last(), Some(&"key.help"));
    a.feed('r');
    let w = bar(&a);
    assert_eq!((w[0], w.last()), ("bar.send", Some(&"bar.leave")));
    a.compose.as_mut().unwrap().preview = true;
    assert_eq!(bar(&a).last(), Some(&"bar.edit"));
    a.open_pick(0);
    assert_eq!(bar(&a), ["bar.tick", "bar.move", "bar.apply"]);
    a.close_compose();
    a.new_issue();
    let w = bar(&a);
    assert_eq!((w[0], w.last()), ("bar.create", Some(&"bar.no")));
    assert!(!w.contains(&"bar.send_close") && !w.contains(&"pick.labels"), "a new issue has no fields");
    a.close_compose();
    a.feed('x');
    assert_eq!(bar(&a), ["key.close", "bar.no"]);
    a.escape();
    a.palette = Some(ito_palette::View::new());
    assert_eq!(bar(&a).last(), Some(&"key.close"));
    a.palette = None;
    a.toggle_help();
    assert_eq!(a.bar_keys().last(), Some(&("F1 Esc".to_owned(), "key.back")));
}

#[test]
fn f5_asks_for_a_sync_and_the_keybar_offers_it() {
    let mut a = app();
    let words = a.bar_keys().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
    assert!(words.ends_with(&["key.sync", "key.owner", "key.help"]));
    a.run_cmd(Cmd::Sync);
    assert!(a.sync_wanted);
    assert!(a.palette_entries("").iter().any(|x| x.pick == Cmd::Sync && x.key == "F5"));
}

#[test]
fn at_switches_the_owner_and_back() {
    let mut a = app();
    a.viewer = Some("me".into());
    assert!(a.feed('@'));
    assert_eq!(a.palette.as_ref().map(|v| v.query.as_str()), Some("@"));
    let owners = |a: &App, q: &str| {
        a.palette_entries(q).into_iter().filter_map(|x| if let Cmd::Owner(o) = x.pick { Some(o) } else { None }).collect::<Vec<_>>()
    };
    assert!(owners(&a, "@").is_empty(), "nobody to switch to yet");
    assert_eq!(owners(&a, "@rust-lang"), ["rust-lang"]);
    a.palette = None;
    a.run_cmd(Cmd::Owner("rust-lang".into()));
    assert_eq!((a.settings.owner.as_str(), a.scope_text[0].as_str()), ("rust-lang", "rust-lang"));
    assert_eq!(a.owner(), Some("rust-lang"));
    // Back to the viewer, who is offered first; the other owner is kept.
    assert_eq!(owners(&a, "@"), ["me"]);
    a.show_owner("me");
    assert_eq!(a.settings.owner, "");
    assert_eq!(a.owner(), Some("me"));
    assert_eq!(owners(&a, "@"), ["rust-lang"]);
    assert_eq!(owners(&a, "@tokio-rs"), ["tokio-rs", "rust-lang"]);
}

#[test]
fn actions_keys_rerun_cancel_and_open() {
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Bad, Some(9)), run(2, actions::Status::Running, None)];
    assert!(a.feed('R'));
    assert_eq!(a.outbox.len(), 1);
    assert_eq!(a.outbox[0].key, "write:1:rerun");
    // Only a going run can be stopped.
    assert!(a.feed('X'));
    assert_eq!(a.outbox.len(), 1);
    assert!(a.feed('p'));
    assert_eq!(a.open_url.take().as_deref(), Some("https://github.com/uchmk/kura/pull/9"));
    assert!(a.feed('j'));
    assert!(a.feed('X'));
    assert_eq!((a.confirm.as_deref(), a.confirm_act), (Some("2"), Confirm::Cancel));
    assert_eq!(a.outbox.len(), 1, "nothing stops before Enter");
    assert!(a.enter());
    assert_eq!(a.outbox[1].key, "write:2:cancel");
    assert!(a.feed('R'));
    assert_eq!(a.outbox.len(), 2, "a running run is not re-run");
    // Not before its workflow file is read: the inputs may want the form.
    assert!(a.feed('D'));
    assert_eq!(a.outbox.len(), 2);
    assert_eq!(a.toast.as_ref().map(|t| t.0.clone()), Some(a.tr("run.reading").to_string()));
    a.run_detail = Some(RunDetail { id: 2, yaml_tried: true, ..Default::default() });
    assert!(a.feed('D') && a.enter());
    assert_eq!(a.outbox[2].key, "write:2:dispatch");
    assert!(a.feed('e'));
    assert!(a.open_url.take().unwrap().ends_with("/runs/2"));
    assert!(a.feed('p'));
    assert!(a.open_url.is_none(), "no pull request to open");
    assert!(!a.open_outside);
    // `O` is `o` in the system's browser.
    assert!(a.feed('O'));
    assert!(a.open_url.take().unwrap().ends_with("/runs/2"));
    assert!(a.open_outside);
}

#[test]
fn actions_filters_take_the_keys_in_their_column() {
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Bad, None)];
    assert!(a.feed('h'));
    assert!(!a.feed('R'), "the filter column ignores it");
    assert!(a.outbox.is_empty(), "R does nothing in the filter column");
}

fn board_with_two_cards() -> board::Project {
    let col = |id: &str, name: &str| board::Column { option: Some(id.into()), name: name.into() };
    let card = |item: &str, option: &str| board::Card {
        item: item.into(),
        id: format!("N-{item}"),
        repo: "uchmk/kura".into(),
        number: 1,
        title: "t".into(),
        url: format!("https://example.test/{item}"),
        pr: false,
        labels: Vec::new(),
        option: Some(option.into()),
        priority: None,
    };
    board::Project {
        id: "P".into(),
        title: "uchmk".into(),
        field: "F".into(),
        options: vec![col("o1", "Todo"), col("o2", "Doing")],
        priority_field: "PF".into(),
        priority_options: vec![col("p1", "High")],
        cards: vec![card("I1", "o1"), card("I2", "o1")],
    }
}

#[test]
fn the_board_moves_a_card_writes_and_undoes() {
    let mut a = app();
    a.feed('4');
    assert!(!a.feed('L'), "no board yet");
    a.board = Some(board_with_two_cards());
    assert!(a.feed('L'));
    assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o2"));
    assert_eq!(a.outbox.len(), 1);
    assert!(a.outbox[0].key.starts_with("board:I1:"));
    assert_eq!(a.bs.col, 1);
    assert!(a.feed('u'));
    assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o1"));
    assert_eq!(a.outbox.len(), 2);
    assert!(a.feed('u'), "nothing left to undo is still the board's key");
    assert_eq!(a.outbox.len(), 2);
}

#[test]
fn a_dropped_card_goes_to_its_column_once() {
    let mut a = app();
    a.feed('4');
    a.board = Some(board_with_two_cards());
    a.drop_card(1, 1);
    assert_eq!(a.board.as_ref().unwrap().cards[1].option.as_deref(), Some("o2"));
    assert_eq!(a.outbox.len(), 1);
    a.drop_card(1, 1);
    assert_eq!(a.outbox.len(), 1, "dropping on its own column writes nothing");
    a.drop_card(9, 0);
    assert_eq!(a.outbox.len(), 1);
}

#[test]
fn board_o_opens_the_card_in_the_browser() {
    let mut a = app();
    a.feed('4');
    a.board = Some(board_with_two_cards());
    assert!(a.feed('o'));
    assert_eq!(a.open_url.as_deref(), Some("https://example.test/I1"));
}

fn note(id: &str) -> Note {
    Note {
        id: id.into(),
        repo: "o/r".into(),
        title: "t".into(),
        kind: "Issue".into(),
        reason: "mention".into(),
        updated_at: String::new(),
        url: format!("https://example.test/{id}"),
    }
}

#[test]
fn n_opens_the_notifications_and_enter_reads_one() {
    let mut a = app();
    a.notes = vec![note("1"), note("2")];
    a.unread = 2;
    assert!(a.feed('n'));
    assert_eq!(a.mode, Mode::Notes);
    assert!(a.feed('j'));
    assert!(a.enter());
    assert_eq!(a.open_url.as_deref(), Some("https://example.test/2"));
    assert_eq!((a.notes.len(), a.unread), (1, 1));
    assert!(a.outbox[0].key.ends_with("note:2"));
    // Enter closes the list, as a click on a row does.
    assert_eq!(a.mode, Mode::Main);
    assert!(a.feed('n'));
    a.note_click(NoteClick::Close);
    assert_eq!(a.mode, Mode::Main);
}

#[test]
fn a_note_on_a_thread_minato_has_opens_it_there() {
    let mut a = with_pr();
    a.items[0].url = "https://example.test/P1".into();
    a.view = View::Board;
    a.notes = vec![Note { url: "https://example.test/P1".into(), ..note("1") }];
    a.unread = 1;
    a.feed('n');
    assert!(a.is_on(View::Board));
    a.note_click(NoteClick::Open(0));
    assert_eq!(a.open_url, None);
    assert_eq!(a.mode, Mode::Main);
    assert!(matches!(a.view, View::Inbox | View::List));
    assert_eq!(a.selected().map(|i| i.id.as_str()), Some("P1"));
    assert!(a.notes.is_empty());
}

#[test]
fn clicks_pick_rows_and_columns() {
    let mut a = with_pr();
    a.items.push(Item { id: "P2".into(), number: 4, ..pr("P2") });
    a.click(Click::ListItem(1));
    assert_eq!((a.list.col, a.list.sel), (minato_core::list::Col::Items, 1));
    a.click(Click::ListCol(minato_core::list::Col::Detail));
    assert_eq!(a.list.col, minato_core::list::Col::Detail);
    a.runs = vec![run(1, actions::Status::Ok, None), run(2, actions::Status::Bad, None)];
    a.click(Click::ActRun(1));
    assert_eq!((a.acts.col, a.acts.sel), (actions::Col::Runs, 1));
    a.click(Click::ActCol(actions::Col::Detail));
    assert_eq!(a.acts.col, actions::Col::Detail);
}

#[test]
fn a_runs_note_opens_its_run_in_actions() {
    let mut a = app();
    let mut old = run(1, actions::Status::Bad, None);
    old.started_at = "2026-10-12T08:00:00Z".into();
    let later = Run { updated_at: "2026-10-12T10:00:00Z".into(), ..run(3, actions::Status::Running, None) };
    let other = Run { branch: "dev".into(), ..run(4, actions::Status::Bad, None) };
    a.runs = vec![run(2, actions::Status::Ok, None), later, old, other];
    a.acts.state = Some(actions::Bucket::Bad);
    // Seen, and still shown when a notification opens it.
    a.acts.seen.insert(2);
    let ci = Note {
        repo: "uchmk/kura".into(),
        kind: "CheckSuite".into(),
        title: "check workflow run failed for main branch".into(),
        updated_at: "2026-10-12T09:01:00Z".into(),
        url: "https://github.com/uchmk/kura/actions".into(),
        ..note("1")
    };
    a.notes = vec![ci.clone(), Note { title: "gone workflow run failed for main branch".into(), ..ci }];
    a.feed('n');
    a.note_click(NoteClick::Open(0));
    assert_eq!(a.view, View::Actions);
    assert_eq!(a.run().map(|r| r.id), Some(2), "the newest done by then, the filters cleared");
    assert_eq!(a.acts.state, None);
    assert!(a.acts.seen.contains(&2));
    assert_eq!(a.open_url, None);
    // A run minato does not have opens the repository's Actions.
    a.feed('n');
    a.note_click(NoteClick::Open(0));
    assert_eq!(a.open_url.as_deref(), Some("https://github.com/uchmk/kura/actions"));
}

#[test]
fn a_note_on_a_card_done_today_opens_it_in_the_inbox() {
    let mut a = with_pr();
    a.items[0].url = "https://github.com/uchmk/kura/pull/3".into();
    a.marks.insert("P1".into(), Mark::Done(now_epoch()));
    a.notes = vec![Note { url: a.items[0].url.clone(), ..note("1") }];
    assert_eq!(a.note_place(&a.items[0].url).1, None, "no badge for a done card");
    a.feed('n');
    a.note_click(NoteClick::Open(0));
    assert_eq!(a.view, View::Inbox);
    assert_eq!(a.selected().map(|i| i.id.as_str()), Some("P1"));
    assert!(a.outbox.iter().all(|o| !o.key.starts_with("fetch:")));
}

#[test]
fn a_runs_note_opens_the_red_main_card_before_actions() {
    let mut a = with_pr();
    a.items.push(Item {
        id: "main:uchmk/kura".into(),
        title: "CI is red on main".into(),
        number: 0,
        author: String::new(),
        ..pr("main:uchmk/kura")
    });
    a.runs = vec![run(1, actions::Status::Bad, None)];
    let ci = Note {
        repo: "uchmk/kura".into(),
        kind: "CheckSuite".into(),
        title: "check workflow run failed for main branch".into(),
        url: "https://github.com/uchmk/kura/actions".into(),
        ..note("1")
    };
    a.notes = vec![Note { title: "check workflow run failed for dev branch".into(), ..ci.clone() }, ci];
    a.feed('n');
    a.note_click(NoteClick::Open(1));
    assert_eq!(a.view, View::Inbox);
    assert_eq!(a.selected().map(|i| i.id.as_str()), Some("main:uchmk/kura"));
    // Another branch's run is not the red main: Actions.
    a.runs[0].branch = "dev".into();
    a.feed('n');
    a.note_click(NoteClick::Open(0));
    assert_eq!(a.view, View::Actions);
}

#[test]
fn a_note_on_a_closed_pr_fetches_it_into_the_list() {
    let mut a = with_pr();
    let url = "https://github.com/uchmk/kura/pull/7";
    a.notes = vec![Note { url: url.into(), kind: "PullRequest".into(), ..note("1") }];
    a.feed('n');
    a.note_click(NoteClick::Open(0));
    assert_eq!(a.open_url, None, "not the browser");
    assert!(a.outbox.iter().any(|o| o.key == format!("fetch:{url}")));
    // What Net does when it comes.
    let got = Item { id: "P7".into(), number: 7, url: url.into(), merged: true, ..pr("P7") };
    a.fetched.push(got.clone());
    a.items.push(got);
    a.view = View::Board;
    a.show_fetched("P7");
    assert_eq!(a.view, View::List);
    assert_eq!(a.selected().map(|i| i.id.as_str()), Some("P7"));
    assert!(a.inbox_all().iter().all(|c| a.items[c.at].id != "P7"), "not in the inbox");
}

#[test]
fn a_note_read_here_is_not_listed_again_by_an_old_list() {
    let mut a = app();
    a.take_notes(vec![note("1"), note("2")]);
    a.feed('n');
    a.feed('d');
    assert_eq!(a.unread, 1);
    a.take_notes(vec![note("1"), note("2")]);
    assert_eq!((a.notes.len(), a.notes[0].id.as_str()), (1, "2"));
    a.feed('A');
    a.take_notes(vec![note("1"), note("2")]);
    assert_eq!(a.unread, 0);
}

#[test]
fn capital_a_marks_everything_read_once() {
    let mut a = app();
    a.notes = vec![note("1")];
    a.unread = 1;
    a.feed('n');
    a.feed('A');
    a.feed('A');
    assert!(a.notes.is_empty());
    assert_eq!((a.unread, a.outbox.len()), (0, 1));
}

#[test]
fn the_palette_lists_commands_and_runs_one() {
    let mut a = app();
    let e = a.palette_entries("");
    assert!(!e.iter().any(|x| x.key == "1"), "no lines for the views");
    // The names of the commands tsumugi has too are tsumugi's.
    let name = |c: Cmd| e.iter().find(|x| x.pick == c).map(|x| x.title.clone());
    assert_eq!(name(Cmd::Settings).as_deref(), Some(a.tr("cmd.settings")));
    assert_eq!(name(Cmd::Notes).as_deref(), Some(a.tr("cmd.notes")));
    assert_eq!(name(Cmd::Help).as_deref(), Some(a.tr("cmd.help")));
    a.run_cmd(Cmd::Notes);
    assert_eq!(a.mode, Mode::Notes);
    a.palette = Some(ito_palette::View::new());
    assert!(a.feed('q'));
    assert!(!a.quit);
}

#[test]
fn alt_s_picks_a_status_for_an_item_in_the_project() {
    let mut a = with_pr();
    a.board = Some(board_with_two_cards());
    a.board.as_mut().unwrap().cards[0].id = a.items[0].id.clone();
    a.feed('r');
    a.outbox.clear();
    a.open_field_picker(false);
    let c = a.compose.as_mut().unwrap();
    let (kind, pk) = c.picker.as_mut().unwrap();
    assert_eq!(*kind, Pick::Status);
    pk.step(true);
    pk.toggle();
    a.apply_pick();
    assert!(a.outbox[0].key.starts_with("board:I1:"));
    assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o2"));
}

#[test]
fn alt_p_says_so_when_the_item_is_not_in_the_project() {
    let mut a = with_pr();
    a.board = Some(board_with_two_cards());
    a.feed('r');
    a.open_field_picker(true);
    assert!(a.compose.as_ref().unwrap().picker.is_none() && a.toast.is_some());
}

#[test]
fn milestone_and_reviewers_are_written_from_their_pickers() {
    let mut a = with_pr();
    a.feed('r');
    let c = a.compose.as_mut().unwrap();
    c.milestones = vec![(4, "v1".into()), (5, "v2".into())];
    c.people = vec!["amy".into(), "bob".into()];
    a.outbox.clear();
    a.open_extra_picker(false);
    let (kind, pk) = a.compose.as_mut().unwrap().picker.as_mut().unwrap();
    assert_eq!(*kind, Pick::Milestone);
    pk.step(true);
    pk.toggle();
    a.apply_pick();
    let Call::Send { path, body, .. } = &a.outbox[0].call else { panic!() };
    assert!(path.ends_with("/issues/3") && body.contains("\"milestone\":5"), "{path} {body}");
    a.open_extra_picker(true);
    let (kind, pk) = a.compose.as_mut().unwrap().picker.as_mut().unwrap();
    assert_eq!(*kind, Pick::Reviewers);
    pk.toggle();
    a.apply_pick();
    let Call::Send { path, body, .. } = &a.outbox[1].call else { panic!() };
    assert!(path.ends_with("/pulls/3/requested_reviewers") && body.contains("amy"), "{path} {body}");
}

#[test]
fn hidden_fields_say_so_and_settings_are_read_back() {
    let mut a = app();
    assert!(a.field_shown(0));
    assert!(!a.field_shown(3) && a.toast.is_some());
    let dir = std::env::temp_dir().join(format!("minato-settings-{}", std::process::id()));
    let file = dir.join("config.toml");
    a.keep_settings(file.clone());
    a.settings.fields[3] = true;
    a.searches_text = "is:open
"
    .into();
    a.sync_searches();
    a.settings.rules.0[6] = true;
    a.save_settings();
    for _ in 0..100 {
        if file.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut b = app();
    b.keep_settings(file);
    assert_eq!(b.settings, a.settings);
    assert_eq!(b.list.searches, ["is:open"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn x_reopens_a_closed_item_after_enter() {
    let mut a = with_pr();
    a.feed('2');
    assert!(a.feed('C') && a.show_closed);
    for i in &mut a.items {
        i.closed = true;
    }
    let before = a.outbox.len();
    assert!(a.feed('x'));
    assert_eq!(a.confirm_act, Confirm::Reopen);
    assert_eq!(a.outbox.len(), before, "nothing is sent before Enter");
    assert!(a.escape() && a.confirm.is_none());
    assert_eq!(a.outbox.len(), before);
    assert!(a.feed('x') && a.enter());
    let Call::Send { body, .. } = &a.outbox.last().unwrap().call else { panic!() };
    assert!(a.outbox.last().is_some_and(|o| o.key.ends_with(":state")) && body.contains(r#""state":"open""#), "{body}");
    assert!(a.feed('C') && !a.show_closed);
    // Only the list searches closed ones; the inbox keeps the open.
    a.show_closed = true;
    assert!(a.closed_shown());
    a.view = View::Inbox;
    assert!(!a.closed_shown());
}

#[test]
fn reviewers_are_for_prs_only() {
    let mut a = with_pr();
    a.items[0].kind = Kind::Issue;
    a.feed('r');
    a.open_extra_picker(true);
    assert!(a.compose.as_ref().unwrap().picker.is_none() && a.toast.is_some());
    // Hidden in the settings, it still says it is for PRs.
    a.settings.fields[5] = false;
    a.toast = None;
    assert!(!a.field_shown(5));
    assert_eq!(a.toast.as_ref().map(|t| t.0.clone()), Some(a.tr("write.not_pr").to_string()));
}

#[test]
fn seen_runs_drop_out_and_are_read_back_after_a_restart() {
    let dir = std::env::temp_dir().join(format!("minato-seen-{}", std::process::id()));
    let file = dir.join("seen.txt");
    let mut a = app();
    a.keep_seen(file.clone());
    a.view = View::Actions;
    a.runs = vec![run(1, actions::Status::Ok, None), run(2, actions::Status::Bad, None)];
    a.settle_runs(1.0);
    a.settle_runs(2.5);
    assert!(a.feed('j'));
    a.settle_runs(3.0);
    assert_eq!(a.acts.shown(&a.runs).len(), 1);
    assert_eq!(a.run().map(|r| r.id), Some(2));
    for _ in 0..100 {
        if file.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut b = app();
    b.keep_seen(file);
    assert_eq!(b.acts.seen, [1].into());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn marks_are_read_back_after_a_restart() {
    let dir = std::env::temp_dir().join(format!("minato-marks-{}", std::process::id()));
    let file = dir.join("m.json");
    let mut a = with_pr();
    a.view = View::Inbox;
    a.keep_marks(file.clone());
    a.feed('d');
    let want = a.marks.clone();
    assert_eq!(want.len(), 1);
    for _ in 0..100 {
        if file.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut b = app();
    b.keep_marks(file);
    assert_eq!(b.marks, want);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn c_copies_a_prompt_for_claude() {
    let mut a = with_pr();
    a.view = View::Inbox;
    a.items[0].url = "https://github.com/uchmk/kura/pull/3".into();
    assert!(a.feed('c'));
    let text = a.copy.take().unwrap();
    assert!(text.starts_with("uchmk/kura PR #3: t\nhttps://github.com/uchmk/kura/pull/3\n\n"), "{text}");
    assert!(text.contains(a.tr("prompt.why_wait")) && text.contains(a.tr("prompt.ask_wait")), "{text}");
    assert!(a.toast.is_some());
    // A card a rule took in asks what the rule wants.
    a.items[0].ci = Some(minato_core::list::Ci::Bad);
    a.feed('c');
    assert!(a.copy.take().unwrap().contains(a.tr("ask.red_ci")));
}

#[test]
fn digits_open_the_views() {
    let mut a = app();
    assert_eq!(a.view, View::Inbox);
    for (c, v) in
        [('2', View::List), ('3', View::Actions), ('4', View::Board), ('5', View::Status), ('6', View::Security), ('1', View::Inbox)]
    {
        assert!(a.feed(c));
        assert_eq!(a.view, v);
    }
    assert!(!a.feed('7'));
    assert!(!a.feed('z'));
}

fn alert(n: u64, source: security::Source) -> security::Alert {
    security::Alert {
        repo: "uchmk/kura".into(),
        source,
        number: n,
        severity: security::Severity::High,
        what: "rule".into(),
        summary: String::new(),
        place: "src/a.rs:1".into(),
        created_at: "2026-10-12T09:00:00Z".into(),
        url: format!("https://github.com/uchmk/kura/security/{n}"),
        body: String::new(),
        extra: Vec::new(),
    }
}

#[test]
fn x_asks_then_dismisses_and_u_reopens() {
    let mut a = app();
    a.feed('6');
    assert_eq!(a.view, View::Security);
    a.alerts = vec![alert(1, security::Source::Code), alert(2, security::Source::Dependabot)];
    a.feed('j');
    assert_eq!(a.security_sel, 1);
    a.feed('x');
    assert_eq!(a.confirm.as_deref(), Some("dep:uchmk/kura#2"));
    a.enter();
    assert!(a.confirm.is_none());
    assert_eq!(a.alerts.len(), 1);
    assert_eq!(a.security_sel, 0);
    assert_eq!(a.outbox.len(), 1);
    assert!(a.alerts_changed);
    // Esc leaves it alone; `u` queues the reopening of the one dismissed.
    a.feed('x');
    assert!(a.confirm.is_some());
    a.confirm = None;
    assert_eq!(a.alerts.len(), 1);
    a.feed('u');
    assert_eq!(a.outbox.len(), 2);
    assert!(a.sec_undo.is_empty());
    a.feed('u');
    assert_eq!(a.outbox.len(), 2);
    a.feed('o');
    assert_eq!(a.open_url.as_deref(), Some("https://github.com/uchmk/kura/security/1"));
}

#[test]
fn slash_narrows_the_security_alerts_and_keys_act_on_the_shown_one() {
    let mut a = app();
    a.feed('6');
    let mut dep = alert(2, security::Source::Dependabot);
    dep.what = "lodash".into();
    a.alerts = vec![alert(1, security::Source::Code), dep];
    a.feed('/');
    assert!(a.find_on());
    a.set_find("lodash");
    assert_eq!(a.find_count(), 1);
    assert_eq!(a.sec_shown(), vec![1]);
    a.feed('x');
    assert_eq!(a.confirm.as_deref(), Some("dep:uchmk/kura#2"));
    a.set_find("nothing-like-this");
    assert_eq!(a.find_count(), 0);
    a.confirm = None;
    a.feed('x');
    assert!(a.confirm.is_none());
    a.find_clear();
    assert_eq!(a.sec_shown(), vec![0, 1]);
}

#[test]
fn w_saves_the_list_search_with_the_box_words_once() {
    let mut a = app();
    a.feed('2');
    a.set_find("flaky ci");
    a.feed('W');
    assert_eq!(a.list.searches.len(), 1);
    assert!(a.list.searches[0].ends_with(" | flaky ci"), "{:?}", a.list.searches);
    a.feed('W');
    assert_eq!(a.list.searches.len(), 1);
    assert_eq!(a.searches_text.lines().count(), 1);
}

#[test]
fn z_forgets_the_saved_search_in_force() {
    let mut a = app();
    a.feed('2');
    // Nothing in force: nothing is lost.
    a.searches_text = "is:open one
is:open two
"
    .into();
    a.sync_searches();
    a.feed('Z');
    assert_eq!(a.list.searches.len(), 2);
    a.list.filter.search = Some(0);
    a.feed('Z');
    assert_eq!(a.list.searches, ["is:open two"]);
    assert_eq!(
        a.searches_text,
        "is:open two
"
    );
    assert_eq!(a.list.filter.search, None);
}

#[test]
fn tab_cycles_the_views_round() {
    let mut a = app();
    for v in [View::List, View::Actions, View::Board, View::Status, View::Security, View::Inbox] {
        assert!(a.cycle_view(false));
        assert_eq!(a.view, v);
    }
    assert!(a.cycle_view(true));
    assert_eq!(a.view, View::Security);
    assert!(a.cycle_view(true));
    assert_eq!(a.view, View::Status);
    a.mode = Mode::Notes;
    assert!(!a.cycle_view(false));
    assert_eq!(a.view, View::Status);
}

#[test]
fn w_watches_a_run_and_it_rings_when_it_ends() {
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Running, None), run(2, actions::Status::Ok, None)];
    let words = |a: &App| a.keys_now().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
    assert!(words(&a).contains(&"key.watch"));
    assert!(a.feed('w'));
    assert!(a.watching(1) && a.watched.len() == 1);
    // Watching again stops it; a run that has ended is not watched.
    a.feed('w');
    assert!(a.watched.is_empty());
    a.feed('j');
    assert!(!words(&a).contains(&"key.watch"));
    a.feed('w');
    assert!(a.watched.is_empty());
    a.feed('k');
    a.feed('w');
    assert!(a.watching(1));
    // Still going: nothing rings.
    a.runs_came(&[]);
    assert!(a.rings.is_empty() && a.watching(1));
    a.runs[0].status = actions::Status::Bad;
    a.runs_came(&[]);
    assert!(a.watched.is_empty());
    assert_eq!(a.rings.len(), 1);
    assert!(a.rings[0].sound.as_deref() == Some("alert") && a.rings[0].title == a.tr("watch.failed"));
    assert!(a.rings[0].system && a.rings[0].focus);
    assert!(a.rings[0].body.contains("uchmk/kura") && a.rings[0].body.contains("check"));
}

#[test]
fn a_watched_run_rings_as_the_settings_say() {
    assert_eq!(minato_core::prefs::SOUNDS, ito_notify::SOUNDS);
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Running, None), run(2, actions::Status::Running, None)];
    a.feed('w');
    a.feed('j');
    a.feed('w');
    let n = &mut a.settings.notify;
    n.system[2] = false;
    n.sound[0] = false;
    n.sounds[2] = "low".into();
    n.focus = false;
    a.runs[0].status = actions::Status::Ok;
    a.runs[1].status = actions::Status::Idle;
    a.runs_came(&[]);
    let r = &a.rings;
    assert_eq!(r.len(), 2);
    assert!(r[0].system && r[0].sound.is_none() && r[0].title == a.tr("watch.finished"));
    assert!(!r[1].system && r[1].sound.as_deref() == Some("low") && r[1].title == a.tr("watch.cancelled"));
    assert!(!r[1].focus);
    // No address, no webhook.
    assert!(r.iter().all(|r| r.hooks.is_empty()));
    // Said in the window either way.
    assert!(a.toast.as_ref().is_some_and(|t| t.0.starts_with(a.tr("watch.cancelled"))));
}

#[test]
fn a_watched_run_goes_to_the_webhook_as_the_settings_say() {
    use ito_notify::webhook::Level;
    assert_eq!(minato_core::prefs::FORMATS, ito_notify::FORMATS);
    let mut a = app();
    a.feed('3');
    a.runs = (1..=3).map(|id| run(id, actions::Status::Running, None)).collect();
    for _ in 0..3 {
        a.feed('w');
        a.feed('j');
    }
    let n = &mut a.settings.notify;
    let mut first = minato_core::prefs::Hook::new();
    (first.url, first.format, first.on[2]) = ("http://127.0.0.1:9/hook".into(), "json".into(), false);
    let second = minato_core::prefs::Hook { url: "https://ntfy.sh/t".into(), format: "ntfy".into(), on: [false, true, true] };
    n.hooks = vec![first, second];
    a.runs[0].status = actions::Status::Ok;
    a.runs[1].status = actions::Status::Bad;
    a.runs[2].status = actions::Status::Idle;
    a.runs_came(&[]);
    let hooks: Vec<Vec<_>> =
        a.rings.iter().map(|r| r.hooks.iter().map(|h| (h.url.as_str(), h.format.as_str(), h.level)).collect()).collect();
    let url = "http://127.0.0.1:9/hook";
    assert_eq!(hooks[0], [(url, "json", Level::Done)]);
    assert_eq!(hooks[1], [(url, "json", Level::Error), ("https://ntfy.sh/t", "ntfy", Level::Error)]);
    assert_eq!(hooks[2], [("https://ntfy.sh/t", "ntfy", Level::Done)]);
    assert_eq!(a.rings[1].hooks[0].folder.as_str(), a.runs[1].repo.as_str());
}

#[test]
fn a_watched_run_pushed_out_of_the_runs_is_asked_apart_and_rings() {
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Running, None)];
    a.feed('w');
    assert!(a.watched_away().is_empty());
    // Newer runs push it out of the ten asked for: it is still watched.
    a.runs = vec![run(2, actions::Status::Running, None)];
    a.runs_came(&[]);
    assert!(a.watching(1) && a.rings.is_empty());
    assert_eq!(a.watched_away().iter().map(|r| r.id).collect::<Vec<_>>(), [1]);
    // Asked apart, it has ended.
    a.runs_came(&[run(1, actions::Status::Ok, None)]);
    assert!(a.watched.is_empty());
    assert!(a.rings.len() == 1 && a.rings[0].sound.as_deref() == Some("chime") && a.rings[0].title == a.tr("watch.finished"));
}

#[test]
fn y_copies_the_link_in_every_view() {
    let mut a = with_pr();
    a.items[0].url = "https://github.com/uchmk/kura/pull/3".into();
    assert!(a.feed('y'));
    assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/pull/3"));
    assert!(a.toast.is_some());
    a.view = View::Inbox;
    a.feed('y');
    assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/pull/3"));
    a.feed('3');
    a.runs = vec![run(7, actions::Status::Ok, None)];
    a.feed('y');
    assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/actions/runs/7"));
    // No link, nothing copied.
    a.copy_link(String::new());
    assert!(a.copy.is_none());
}

fn labelled(id: &str, title: &str, labels: &[&str]) -> Item {
    let mut i = pr(id);
    i.kind = Kind::Issue;
    i.title = title.into();
    i.url = format!("https://github.com/uchmk/kura/issues/{id}");
    i.labels = labels.iter().map(|l| minato_core::list::Label { name: (*l).into(), color: "ededed".into() }).collect();
    i
}

#[test]
fn the_status_view_lists_lanes_releases_and_what_waits() {
    let mut a = app();
    a.viewer = Some("uchmk".into());
    a.items = vec![
        labelled("1", "Lane status: win", &["lane-status"]),
        labelled("2", "Which way?", &["question"]),
        labelled("3", "Run on the PC", &["needs-pc"]),
    ];
    a.status_repos = vec!["uchmk/kura".into(), "uchmk/ito".into()];
    let shipped = overview::Shipped {
        tag: "v1".into(),
        published_at: "2026-10-14T00:00:00Z".into(),
        files: 6,
        sums: true,
        url: "https://github.com/uchmk/kura/releases/tag/v1".into(),
        ..Default::default()
    };
    a.shipped.insert("uchmk/kura".into(), Some(shipped));
    a.shipped.insert("uchmk/ito".into(), None);
    a.ahead.insert("uchmk/kura".into(), ("v1".into(), 4));
    let rows = a.status_rows();
    assert_eq!(rows.len(), 4, "a lane, one release (ito has none), two items");
    assert!(matches!(&rows[0], overview::Row::Lane(l) if l.name == "win"));
    assert!(matches!(&rows[1], overview::Row::Release { ahead: Some(4), .. }));
    assert_eq!(rows[2], overview::Row::Item(2, "needs-pc"));
    assert_eq!(rows[3], overview::Row::Item(1, "question"));
    // Counted from an older tag: not shown.
    a.ahead.insert("uchmk/kura".into(), ("v0".into(), 9));
    assert!(matches!(&a.status_rows()[1], overview::Row::Release { ahead: None, .. }));

    assert!(a.feed('5'));
    assert!(a.feed('k'), "k at the top stays");
    assert_eq!(a.status_sel, 0);
    a.feed('o');
    assert_eq!(a.open_url.take().as_deref(), Some("https://github.com/uchmk/kura/issues/1"));
    a.feed('j');
    a.feed('y');
    assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/releases/tag/v1"));
    a.enter();
    assert_eq!(a.open_url.take().as_deref(), Some("https://github.com/uchmk/kura/releases/tag/v1"));
    for _ in 0..9 {
        a.feed('j');
    }
    assert_eq!(a.status_sel, 3, "j stops at the last line");
    assert!(!a.feed('z'));
    // Enter on an item shows it in the list.
    a.enter();
    assert_eq!(a.view, View::List);
    assert!(a.open_url.is_none());
    // A click picks a line; a double click opens it.
    a.feed('5');
    a.click(Click::StatusRow(0, false));
    assert_eq!(a.status_sel, 0);
    assert!(a.open_url.is_none());
    a.click(Click::StatusRow(1, true));
    assert_eq!((a.status_sel, a.open_url.take().is_some()), (1, true));
}

#[test]
fn other_owners_come_alongside_and_go() {
    let mut a = app();
    let entries = |a: &App, q: &str| a.palette_entries(q).into_iter().map(|x| x.pick).collect::<Vec<_>>();
    // Before GitHub says who the viewer is, there is nobody to stand beside.
    a.run_cmd(Cmd::AddOwner("rust-lang".into()));
    assert!(a.settings.also.is_empty());
    a.viewer = Some("me".into());
    assert_eq!(a.owners_shown(), "me");
    assert!(entries(&a, "+@rust-lang").contains(&Cmd::AddOwner("rust-lang".into())));
    a.run_cmd(Cmd::AddOwner("rust-lang".into()));
    assert_eq!((a.settings.owner.as_str(), a.settings.also.as_slice()), ("me", &["rust-lang".to_string()][..]));
    assert_eq!(a.owners_shown(), "me rust-lang");
    assert_eq!(a.scope_text[0], a.settings.owners_line());
    // Shown already: not offered again; the way back is.
    let e = entries(&a, "+@Rust-Lang");
    assert!(!e.iter().any(|c| matches!(c, Cmd::AddOwner(_))));
    assert!(e.contains(&Cmd::DropOwner("rust-lang".into())));
    a.run_cmd(Cmd::AddOwner("me".into()));
    assert_eq!(a.settings.also.len(), 1, "the owner is not beside itself");
    a.run_cmd(Cmd::DropOwner("RUST-LANG".into()));
    assert!(a.settings.also.is_empty());
    assert_eq!(a.owners_shown(), "me");
    assert!(!entries(&a, "").iter().any(|c| matches!(c, Cmd::DropOwner(_))));
}

#[test]
fn comma_and_question_mark_open_and_escape_closes() {
    let mut a = app();
    assert!(a.feed(','));
    assert_eq!(a.mode, Mode::Settings);
    // In the settings the screen's own controls take the characters.
    assert!(!a.feed('3'));
    assert_eq!(a.view, View::Inbox);
    assert!(a.escape());
    assert_eq!(a.mode, Mode::Main);
    assert!(!a.escape());
    assert!(a.feed('?'));
    assert_eq!(a.mode, Mode::Help);
    assert!(!a.feed('3'), "the help swallows the views' digits");
    assert!(a.feed('?'));
    assert_eq!(a.mode, Mode::Main);
}

#[test]
fn list_keys_work_only_in_the_list_view() {
    let mut a = app();
    assert!(!a.feed('h'), "no list keys in the inbox");
    a.feed('2');
    assert!(a.feed('h'));
    assert_eq!(a.list.col, minato_core::list::Col::Filters);
    assert!(a.feed('l'));
    assert!(a.feed('o'), "o with nothing selected is used and does nothing");
    assert!(a.open_url.is_none());
}

#[test]
fn the_tab_in_force_is_the_view_only_in_the_main_mode() {
    let mut a = app();
    assert!(a.is_on(View::Inbox));
    a.feed(',');
    assert!(!a.is_on(View::Inbox));
}

#[test]
fn every_view_has_keys_and_words_for_them() {
    for v in View::ALL {
        assert!(!v.keys().is_empty());
        for (_, w) in v.keys() {
            assert_ne!(crate::i18n::tr("en", w), *w, "no words for {w}");
        }
        assert_ne!(crate::i18n::tr("en", v.label()), v.label());
        assert_ne!(crate::i18n::tr("en", v.empty()), v.empty());
    }
    for (_, w) in GLOBAL_KEYS {
        assert_ne!(crate::i18n::tr("en", w), w, "no words for {w}");
    }
}

fn pr(id: &str) -> Item {
    Item {
        id: id.into(),
        kind: Kind::Pr,
        repo: "uchmk/kura".into(),
        number: 3,
        title: "t".into(),
        url: String::new(),
        author: "uchmk".into(),
        labels: Vec::new(),
        assignees: Vec::new(),
        milestone: String::new(),
        reviewers: Vec::new(),
        comments: 0,
        last_by: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
        draft: false,
        merged: false,
        closed: false,
        conflict: false,
        mentioned: false,
        peek: String::new(),
        head: String::new(),
        ci: None,
    }
}

fn with_pr() -> App {
    let mut a = app();
    a.view = View::List;
    a.viewer = Some("uchmk".into());
    a.items = vec![pr("P1")];
    a
}

#[test]
fn slash_searches_the_inbox_and_the_list_and_esc_clears() {
    let mut a = with_pr();
    let mut other = pr("P2");
    other.title = "Font size in the Preview".into();
    other.number = 8;
    a.items.push(other);
    a.list.sel = 1;
    assert!(a.feed('/'));
    assert!(a.find_open && a.find_fresh);
    a.set_find("font PREV");
    assert_eq!(a.find_count(), 0);
    a.set_find("font prev");
    assert_eq!(a.find_count(), 1);
    assert_eq!((a.list.sel, a.selected().map(|i| i.number)), (0, Some(8)));
    // The inbox is searched with the same words.
    assert_eq!(a.inbox.find, a.list.find);
    // Enter keeps the words; Esc then clears them.
    a.find_open = false;
    assert!(a.find_on());
    assert!(a.escape());
    assert!(!a.find_on() && a.find_text.is_empty());
    assert_eq!(a.find_count(), 2);
    // Not in the views without a search, nor under the palette.
    a.view = View::Actions;
    a.feed('/');
    assert!(!a.find_open);
    a.view = View::List;
    a.feed('/');
    a.palette = Some(ito_palette::View::new());
    a.find_check();
    assert!(!a.find_open);
}

#[test]
fn r_opens_the_box_and_asks_for_labels_and_people() {
    let mut a = with_pr();
    assert!(a.feed('r'));
    assert!(a.compose.is_some());
    let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["labels:uchmk/kura", "people:uchmk/kura", "miles:uchmk/kura"]);
    // Typed characters do not reach the views while the box is open.
    assert!(a.feed('2'));
    assert_eq!(a.view, View::List);
}

#[test]
fn sending_queues_a_comment_and_close_but_not_an_empty_one() {
    let mut a = with_pr();
    a.feed('r');
    a.outbox.clear();
    a.compose.as_mut().unwrap().text = "ok".into();
    a.send_comment(true);
    let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["write:0:comment", "write:0:state"]);
    assert!(a.compose.is_none());
    a.outbox.clear();
    a.feed('r');
    a.outbox.clear();
    a.send_comment(false);
    assert!(a.outbox.is_empty());
}

#[test]
fn a_comment_settles_an_inbox_card_only_when_auto_done_is_on() {
    let mut a = with_pr();
    assert!(a.inbox_all().iter().any(|c| c.group != Group::Done), "the pull request is in the inbox");
    a.feed('r');
    a.compose.as_mut().unwrap().text = "ok".into();
    a.send_comment(false);
    assert!(a.marks.is_empty(), "off by default");
    a.feed('r');
    a.settings.auto_done = true;
    a.compose.as_mut().unwrap().text = "ok".into();
    a.send_comment(false);
    assert!(matches!(a.marks.get("P1"), Some(Mark::Done(_))));
    assert!(a.inbox_all().iter().all(|c| c.group == Group::Done));
}

#[test]
fn leaving_the_box_keeps_the_text_as_a_draft() {
    let mut a = with_pr();
    a.feed('r');
    a.compose.as_mut().unwrap().text = "half".into();
    a.close_compose();
    assert!(a.compose.is_none());
    assert_eq!(a.drafts.get("P1").map(String::as_str), Some("half"));
    a.click(Click::Write);
    assert_eq!(a.compose.as_ref().unwrap().text, "half");
    assert!(a.drafts.is_empty());
    // Opening it again on the same item keeps what is there.
    a.compose.as_mut().unwrap().text = "half more".into();
    a.click(Click::Write);
    assert_eq!(a.compose.as_ref().unwrap().text, "half more");
    // An empty box leaves no draft.
    a.compose.as_mut().unwrap().text = "  ".into();
    a.close_compose();
    assert!(a.drafts.is_empty());
}

#[test]
fn a_field_chip_opens_its_picker_and_writes_the_one_before() {
    let mut a = with_pr();
    a.click(Click::Field(0));
    assert!(matches!(a.compose.as_ref().unwrap().picker, Some((Pick::Labels, _))));
    let c = a.compose.as_mut().unwrap();
    c.people = vec!["uchmk".into()];
    if let Some((_, pk)) = c.picker.as_mut() {
        pk.options = vec!["bug".into()];
        pk.toggle();
    }
    a.outbox.clear();
    a.click(Click::Field(1));
    assert!(matches!(a.compose.as_ref().unwrap().picker, Some((Pick::Assignees, _))));
    let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["write:0:labels"]);
    // Status needs the item in the Project: it only says so.
    a.click(Click::Field(2));
    assert!(a.compose.as_ref().unwrap().picker.is_none());
}

#[test]
fn the_lists_are_asked_once_and_reach_a_picker_opened_before_them() {
    let mut a = with_pr();
    let repo = a.selected().unwrap().repo.clone();
    a.feed('r');
    let keys: Vec<String> = a.outbox.iter().map(|o| o.key.clone()).collect();
    assert_eq!(keys, [format!("labels:{repo}"), format!("people:{repo}"), format!("miles:{repo}")]);
    a.open_pick(1);
    assert!(a.compose.as_ref().unwrap().picker.as_ref().unwrap().1.options.is_empty());
    a.got_lists(&repo, |l| l.people = vec!["amy".into(), "bob".into()]);
    assert_eq!(a.compose.as_ref().unwrap().picker.as_ref().unwrap().1.options, ["amy", "bob"]);
    // Opened again: what came is there at once and not asked again.
    a.close_compose();
    a.outbox.clear();
    a.feed('r');
    assert!(a.outbox.is_empty());
    assert_eq!(a.compose.as_ref().unwrap().people, ["amy", "bob"]);
    // Another repository's list is kept but not put in this box.
    a.got_lists("o/other", |l| l.labels = vec!["x".into()]);
    assert!(a.compose.as_ref().unwrap().labels.is_empty() && a.lists["o/other"].labels == ["x"]);
    a.lists_failed(&repo);
    assert_eq!(a.ask_lists(&repo).len(), 3);
}

#[test]
fn an_empty_list_that_came_is_not_loading_and_pickers_start_from_the_item() {
    let mut a = with_pr();
    let repo = a.selected().unwrap().repo.clone();
    let id = a.selected().unwrap().id.clone();
    let it = a.items.iter_mut().find(|i| i.id == id).unwrap();
    (it.milestone, it.reviewers) = ("v1".into(), vec!["amy".into()]);
    a.feed('r');
    a.open_pick(3);
    assert!(!a.compose.as_ref().unwrap().picker_ready());
    a.got_lists(&repo, |l| (l.milestones, l.came[2]) = (Vec::new(), true));
    assert!(a.compose.as_ref().unwrap().picker_ready());
    assert_eq!(a.compose.as_ref().unwrap().picker.as_ref().unwrap().1.chosen, ["v1"]);
    a.got_lists(&repo, |l| (l.people, l.came[1]) = (vec!["amy".into(), "bob".into()], true));
    // Reviewers: bob ticked is asked, amy unticked is taken off.
    a.outbox.clear();
    a.open_pick(5);
    let pk = &mut a.compose.as_mut().unwrap().picker.as_mut().unwrap().1;
    assert!(pk.is_chosen("amy"));
    pk.toggle();
    pk.step(true);
    pk.toggle();
    a.apply_pick();
    let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["write:0:reviewers", "write:0:unreview"]);
}

#[test]
fn the_owner_is_the_settings_one_else_the_viewer() {
    let mut a = app();
    assert_eq!(a.owner(), None);
    a.viewer = Some("uchmk".into());
    assert_eq!(a.owner(), Some("uchmk"));
    a.settings.owner = "rust-lang".into();
    assert_eq!(a.owner(), Some("rust-lang"));
}

#[test]
fn the_next_box_button_is_its_key() {
    let mut a = app();
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Bad, None)];
    a.click(Click::Key('R'));
    assert_eq!(a.outbox.len(), 1);
    assert_eq!(a.outbox[0].key, "write:1:rerun");
}

#[test]
fn the_strip_buttons_open_the_box_and_send() {
    let mut a = with_pr();
    a.click(Click::Tool(Tool::Bold));
    let c = a.compose.as_ref().unwrap();
    assert!(c.tool == Some(Tool::Bold) && !c.preview);
    a.click(Click::Tab(true));
    assert!(a.compose.as_ref().unwrap().preview);
    a.outbox.clear();
    // Comment with nothing written does nothing and keeps the box.
    a.click(Click::Send(false));
    assert!(a.outbox.is_empty() && a.compose.is_some());
    // Close with nothing written asks first.
    a.click(Click::Send(true));
    assert!(a.compose.is_none());
    assert_eq!(a.confirm.as_deref(), Some("P1"));
    assert!(a.confirm_act == Confirm::Close);
    a.confirm = None;
    a.click(Click::Write);
    a.outbox.clear();
    a.compose.as_mut().unwrap().text = "done".into();
    a.click(Click::Send(true));
    let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["write:0:comment", "write:0:state"]);
}

#[test]
fn the_widths_come_back_from_state_toml() {
    let dir = std::env::temp_dir().join(format!("minato-state-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("state.toml");
    let mut a = app();
    let mut st = a.columns();
    assert!(!st.inbox.is_empty());
    for r in st.inbox.iter_mut().chain(st.list.iter_mut()).chain(st.actions.iter_mut()) {
        *r = 0.31;
    }
    std::fs::write(&f, minato_core::state::format(&st)).unwrap();
    a.keep_state(f.clone());
    assert_eq!(a.columns(), st);
    // A list of another length is left alone.
    let mut short = st.clone();
    short.inbox.push(0.5);
    let mut b = app();
    std::fs::write(&f, minato_core::state::format(&short)).unwrap();
    let before = b.columns().inbox;
    b.keep_state(f);
    assert_eq!(b.columns().inbox, before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_waits_for_enter_and_escape_cancels() {
    let mut a = with_pr();
    a.view = View::List;
    a.ask_merge();
    assert_eq!(a.confirm.as_deref(), Some("P1"));
    assert!(a.escape());
    assert!(a.outbox.is_empty());
    a.ask_merge();
    assert!(a.enter());
    assert_eq!(a.outbox.len(), 1);
    assert!(a.outbox[0].key.ends_with(":merge"));
}

#[test]
fn a_draft_or_an_issue_is_not_merged() {
    let mut a = with_pr();
    a.items[0].draft = true;
    a.ask_merge();
    assert!(a.confirm.is_none() && a.toast.is_some());
}

#[test]
fn a_applies_the_picker_only_when_it_changed() {
    let mut a = with_pr();
    a.feed('r');
    a.outbox.clear();
    let c = a.compose.as_mut().unwrap();
    c.picker = Some((Pick::Labels, Picker::new(vec!["bug".into()], Vec::new())));
    a.apply_pick();
    assert!(a.outbox.is_empty());
    let c = a.compose.as_mut().unwrap();
    let mut pk = Picker::new(vec!["bug".into()], Vec::new());
    pk.toggle();
    c.picker = Some((Pick::Labels, pk));
    a.apply_pick();
    assert!(a.outbox[0].key.ends_with(":labels"));
}

fn with_detail(a: &mut App, yaml: &str) {
    a.feed('3');
    a.runs = vec![run(1, actions::Status::Bad, None)];
    a.acts.col = actions::Col::Runs;
    let job = |id, name: &str, status| actions::Job {
        id,
        name: name.into(),
        status,
        started_at: String::new(),
        completed_at: String::new(),
        url: String::new(),
    };
    a.run_detail = Some(RunDetail {
        id: 1,
        jobs: Some(vec![job(10, "lint", actions::Status::Ok), job(11, "Test (linux)", actions::Status::Bad)]),
        workflow: Some(workflow::parse(yaml)),
        yaml_tried: true,
        ..Default::default()
    });
}

#[test]
fn d_opens_the_form_when_the_workflow_has_inputs_and_sends_them() {
    let mut a = app();
    with_detail(
        &mut a,
        "on:
  workflow_dispatch:
    inputs:
      who:
        required: true
jobs:
  lint:
    runs-on: x
",
    );
    a.feed('D');
    assert!(matches!(a.overlay, Some(Overlay::Form(_))));
    assert!(a.outbox.is_empty());
    a.overlay_key(K::Send);
    assert!(a.overlay.is_some() && a.outbox.is_empty());
    a.overlay_key(K::Char('x'));
    a.overlay_key(K::Send);
    assert!(a.overlay.is_none());
    let Call::Send { body, .. } = &a.outbox[0].call else { panic!() };
    assert!(body.contains("\"inputs\"") && body.contains("\"who\":\"x\""), "{body}");
}

#[test]
fn d_asks_before_starting_without_inputs_and_l_opens_the_failed_log() {
    let mut a = app();
    with_detail(
        &mut a,
        "jobs:
  lint:
    runs-on: x
",
    );
    a.feed('D');
    assert!(a.overlay.is_none());
    assert_eq!((a.confirm.as_deref(), a.confirm_act), (Some("1"), Confirm::Dispatch));
    assert!(a.outbox.is_empty(), "nothing starts before Enter");
    assert!(a.escape() && a.confirm.is_none() && a.outbox.is_empty());
    a.feed('D');
    assert!(a.enter() && a.confirm.is_none());
    assert_eq!(a.outbox.len(), 1);
    assert_eq!(a.outbox[0].key, "write:1:dispatch");
    a.feed('L');
    let Some(Overlay::Log(l)) = &a.overlay else { panic!() };
    assert_eq!(l.job_id, 11);
    assert_eq!(a.outbox[1].key, "a-log:11");
    assert!(a.escape() && a.overlay.is_none());
}

#[test]
fn x_closes_only_after_enter_and_m_works_in_the_list() {
    let mut a = with_pr();
    assert!(a.feed('x'));
    assert_eq!((a.confirm.as_deref(), a.confirm_act), (Some("P1"), Confirm::Close));
    assert!(a.escape() && a.outbox.is_empty());
    a.feed('x');
    assert!(a.enter());
    assert_eq!(a.outbox.len(), 1);
    assert!(a.outbox[0].key.ends_with(":state"));
    a.feed('m');
    assert_eq!(a.confirm_act, Confirm::Merge);
    a.enter();
    assert!(a.outbox[1].key.ends_with(":merge"));
}

#[test]
fn a_merged_item_is_not_closed() {
    let mut a = with_pr();
    a.items[0].merged = true;
    a.feed('x');
    assert!(a.confirm.is_none() && a.toast.is_some());
}

#[test]
fn gg_and_g_go_to_the_ends() {
    let mut a = with_pr();
    a.items.push(pr("P2"));
    a.items.push(pr("P3"));
    a.view = View::List;
    a.list.col = minato_core::list::Col::Items;
    a.feed('G');
    assert_eq!(a.list.sel, 2);
    a.feed('g');
    assert_eq!(a.list.sel, 2, "one g waits for the second");
    a.feed('g');
    assert_eq!(a.list.sel, 0);
    // A key between the two g's starts over.
    a.feed('G');
    a.feed('g');
    a.feed('k');
    a.feed('g');
    assert_eq!(a.list.sel, 1);
}

fn press(a: &mut App, keys: &str) {
    for t in keys.split_whitespace() {
        a.press(Key::parse(t).unwrap());
    }
}

#[test]
fn the_keys_go_through_keymap_toml() {
    let mut a = with_pr();
    a.items.push(pr("P2"));
    a.list.col = minato_core::list::Col::Items;
    // The built-in keys: the arrows, a chord, Tab and Shift+Tab.
    press(&mut a, "<Down>");
    assert_eq!(a.list.sel, 1);
    press(&mut a, "g");
    assert_eq!((a.list.sel, a.key_pending.len()), (1, 1), "one g waits for the second");
    press(&mut a, "g");
    assert_eq!((a.list.sel, a.key_pending.len()), (0, 0));
    // A key that ends no chord is read alone.
    press(&mut a, "g j");
    assert_eq!((a.list.sel, a.key_pending.len()), (1, 0));
    // Esc lets go of a chord begun, and closes nothing.
    a.mode = Mode::Help;
    press(&mut a, "g");
    assert!(a.escape());
    assert_eq!((a.mode, a.key_pending.len()), (Mode::Help, 0));
    a.mode = Mode::Main;
    press(&mut a, "<Tab>");
    assert_eq!(a.view, View::Actions);
    press(&mut a, "<BackTab> 4");
    assert_eq!(a.view, View::Board);
    a.board = Some(board_with_two_cards());
    press(&mut a, "<S-Right>");
    assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o2"));
    // A user's file: F2 opens the list, `d` does nothing in the inbox and
    // Ctrl+D marks done; the keybar and the help follow.
    a.prefs.keymap = Some(
        r#"
            [main]
            prepend_keymap = [{ on = "<F2>", run = "view list" }]
            [inbox]
            prepend_keymap = [{ on = "<C-d>", run = "done" }, { on = "d", run = "none" }]
            "#
        .into(),
    );
    a.toast = None;
    a.load_keys();
    assert!(a.toast.is_none());
    press(&mut a, "<F2>");
    assert_eq!(a.view, View::List);
    a.view = View::Inbox;
    let ctrl_d = if cfg!(target_os = "macos") { "⌘D" } else { "Ctrl+D" };
    assert!(a.bar_keys().iter().any(|(k, w)| k == ctrl_d && *w == "key.done"), "{:?}", a.bar_keys());
    assert_eq!(a.key_rows(Ctx::View(View::Inbox), &GLOBAL_KEYS)[0].0, "1 F2 3 4 5 6");
    assert_eq!(a.char_label(Ctx::View(View::Inbox), "d"), ctrl_d);
    assert_eq!(a.char_label(Ctx::View(View::List), "i"), "i", "no action: as it is");
    // What does not work is said, and the rest stays.
    a.prefs.keymap = Some(
        "[main]
prepend_keymap = [{ on = \"q\", run = \"fly\" }]"
            .into(),
    );
    a.load_keys();
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("fly")), "{:?}", a.toast);
    press(&mut a, "2");
    assert_eq!(a.view, View::List);
}

#[test]
fn the_detail_stops_at_its_end() {
    let mut a = with_pr();
    a.view = View::List;
    a.list.col = minato_core::list::Col::Detail;
    a.detail_max = Some(4.0);
    for _ in 0..10 {
        a.feed('j');
    }
    assert_eq!(a.list.scroll, 4.0);
    a.feed('k');
    assert_eq!(a.list.scroll, 3.0, "one up from the end shows at once");
    a.feed('G');
    assert_eq!(a.list.scroll, 4.0);
    a.feed('g');
    a.feed('g');
    assert_eq!(a.list.scroll, 0.0);
}

#[test]
fn f1_opens_and_closes_the_keys() {
    let mut a = app();
    a.palette = Some(ito_palette::View::new());
    a.toggle_help();
    assert!(a.mode == Mode::Help && a.palette.is_none());
    // Tab turns the help's page, not the view.
    assert!(a.cycle_view(false));
    assert!(a.help_rules && a.view == View::Inbox);
    a.toggle_help();
    assert_eq!(a.mode, Mode::Main);
}

#[test]
fn a_toast_goes_when_its_time_is_up() {
    let mut a = app();
    a.say("Assigned to you");
    assert!(a.toast_left().is_some());
    a.toast = Some(("Assigned to you".into(), Instant::now()));
    assert_eq!(a.toast_left(), None, "an expired toast is not drawn");
}

#[test]
fn the_scale_keys_step_and_reset() {
    use ito_common::ScaleTo;
    let mut a = app();
    a.scale(ScaleTo::In);
    let up = a.prefs.common.scale_or_one();
    assert!(up > 1.0);
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains('%')));
    a.scale(ScaleTo::Out);
    a.scale(ScaleTo::Out);
    assert!(a.prefs.common.scale_or_one() < 1.0);
    a.scale(ScaleTo::Reset);
    assert_eq!(a.prefs.common.scale_or_one(), 1.0);
}

#[test]
fn the_wheel_moves_a_list_and_scrolls_a_detail() {
    let mut a = with_pr();
    a.items.push(pr("P2"));
    a.items.push(pr("P3"));
    // Down a notch: the next row, and the column under the pointer.
    a.wheel(Wheel::List(minato_core::list::Col::Items), -1.0, None);
    assert_eq!((a.list.col, a.list.sel), (minato_core::list::Col::Items, 1));
    // A touchpad's small turns add up to a step.
    a.wheel(Wheel::List(minato_core::list::Col::Items), -0.5, None);
    assert_eq!(a.list.sel, 1);
    a.wheel(Wheel::List(minato_core::list::Col::Items), -0.5, None);
    assert_eq!(a.list.sel, 2);
    a.wheel(Wheel::List(minato_core::list::Col::Items), 2.0, None);
    assert_eq!(a.list.sel, 0);
    // The detail: three lines a notch, stopping at its end and the top.
    a.wheel(Wheel::Detail, -1.0, Some(10.0));
    assert_eq!(a.list.scroll, 3.0);
    a.wheel(Wheel::Detail, -5.0, Some(10.0));
    assert_eq!(a.list.scroll, 10.0);
    a.wheel(Wheel::Detail, 9.0, Some(10.0));
    assert_eq!(a.list.scroll, 0.0);
}
