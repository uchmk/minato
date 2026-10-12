//! minato's own words, in English and Japanese. The language is the one in
//! common.toml (`auto`: the OS's), shared by every uchmk app; the settings
//! screen's own words are `ito_prefs::Words`.

const EN: &[(&str, &str)] = &[
    ("view.inbox", "Inbox"),
    ("view.list", "List"),
    ("view.actions", "Actions"),
    ("view.board", "Board"),
    ("view.settings", "Settings"),
    ("palette.hint", "Search commands and items"),
    ("bell", "Notifications"),
    ("account", "Account"),
    ("empty.inbox", "Things that are your turn will gather here."),
    ("empty.list", "Issues, PRs and Discussions of your repositories will be listed here."),
    ("empty.actions", "Workflow runs will be listed here."),
    ("empty.board", "The cards of the Project will stand here by Status."),
    ("empty.soon", "Not connected to GitHub yet."),
("net.no_token", "No GitHub token: run `gh auth login` or set GITHUB_TOKEN."),
    ("help.title", "Keys"),
    ("help.global", "Everywhere"),
    ("help.close", "Press ? or Esc to close"),
    ("key.views", "Open a view"),
    ("key.settings", "Settings"),
    ("key.help", "This list"),
    ("key.palette", "Command palette"),
    ("key.notes", "Notifications"),
    ("key.undo", "Undo the last action"),
    ("key.next", "Next / previous"),
    ("key.details", "Page the details"),
    ("key.write", "Write"),
    ("key.done", "Done"),
    ("key.later", "Later"),
    ("key.merge", "Merge"),
    ("key.browser", "Open in the browser"),
    ("key.columns", "Move between columns"),
    ("key.filter", "Filter"),
    ("key.inbox_add", "Add to the inbox"),
    ("key.rerun", "Re-run failed jobs"),
    ("key.cancel", "Cancel the run"),
    ("key.dispatch", "Run by hand"),
    ("key.logs", "Logs"),
    ("key.card", "Move the card"),
    ("key.open", "Open"),
    ("key.back", "Back"),
    ("settings.general", "General"),
    ("settings.general_lead", "The language, the size and the clock, shared by every uchmk app."),
    ("settings.theme", "Theme"),
    ("settings.theme_lead", "The colours, shared by every uchmk app."),
    ("settings.display", "Display"),
    ("settings.theme_note", "Your own themes go in"),
    ("settings.no_folder", "This machine has no config folder."),
    ("settings.sample", "A sample in this theme"),
];

const JA: &[(&str, &str)] = &[
    ("view.inbox", "受信箱"),
    ("view.list", "一覧"),
    ("view.actions", "Actions"),
    ("view.board", "ボード"),
    ("view.settings", "設定"),
    ("palette.hint", "命令と項目を探す"),
    ("bell", "通知"),
    ("account", "アカウント"),
    ("empty.inbox", "あなたの番のものがここに集まります。"),
    ("empty.list", "リポジトリの Issue・PR・Discussion がここに並びます。"),
    ("empty.actions", "workflow の実行がここに並びます。"),
    ("empty.board", "Project のカードが Status の列に並びます。"),
    ("empty.soon", "まだ GitHub につないでいません。"),
("net.no_token", "GitHub のトークンがありません。`gh auth login` するか GITHUB_TOKEN を設定してください。"),
    ("help.title", "キーの一覧"),
    ("help.global", "どこでも"),
    ("help.close", "? か Esc で閉じる"),
    ("key.views", "ビューを開く"),
    ("key.settings", "設定"),
    ("key.help", "この一覧"),
    ("key.palette", "コマンドパレット"),
    ("key.notes", "通知"),
    ("key.undo", "最後の操作を取り消す"),
    ("key.next", "次・前"),
    ("key.details", "詳しくをめくる"),
    ("key.write", "書く"),
    ("key.done", "済み"),
    ("key.later", "あとで"),
    ("key.merge", "マージ"),
    ("key.browser", "ブラウザで開く"),
    ("key.columns", "列を動く"),
    ("key.filter", "絞り込み"),
    ("key.inbox_add", "受信箱に入れる"),
    ("key.rerun", "失敗だけ再実行"),
    ("key.cancel", "止める"),
    ("key.dispatch", "手で走らせる"),
    ("key.logs", "ログ"),
    ("key.card", "カードを動かす"),
    ("key.open", "開く"),
    ("key.back", "戻る"),
    ("settings.general", "一般"),
    ("settings.general_lead", "言語・大きさ・時計。uchmk のどのアプリでも共通です。"),
    ("settings.theme", "テーマ"),
    ("settings.theme_lead", "色。uchmk のどのアプリでも共通です。"),
    ("settings.display", "表示"),
    ("settings.theme_note", "自分のテーマの置き場所:"),
    ("settings.no_folder", "このマシンには設定のフォルダがありません。"),
    ("settings.sample", "このテーマでの見本"),
];

/// The language codes minato has words for.
pub const AVAILABLE: [&str; 2] = ["en", "ja"];

/// The words for `key` in `lang` (`ja`, else English); the key itself when
/// there are none.
pub fn tr(lang: &str, key: &'static str) -> &'static str {
    let table = if lang == "ja" { JA } else { EN };
    table.iter().chain(EN).find(|(k, _)| *k == key).map_or(key, |(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_have_the_same_keys() {
        fn keys(t: &[(&'static str, &'static str)]) -> Vec<&'static str> {
            t.iter().map(|(k, _)| *k).collect()
        }
        assert_eq!(keys(EN), keys(JA));
    }

    #[test]
    fn unknown_keys_come_back_as_they_are() {
        assert_eq!(tr("ja", "nope"), "nope");
        assert_eq!(tr("ja", "view.inbox"), "受信箱");
        assert_eq!(tr("fr", "view.inbox"), "Inbox");
    }
}
