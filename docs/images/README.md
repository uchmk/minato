# README の画像の置き場

README の「What it looks like」と、紹介画像（GitHub の Social preview）の右に入れる実画面の置き場。
どれもリリースのビルドを既定のテーマで、持ち主のアカウント（uchmk）につないで撮った。

| ファイル | 何を写すか | 撮り方 |
| --- | --- | --- |
| `inbox.png` | 受信箱（`1`）。いちばん上のカードと、その「次にやること」・本文・書く箱 | 窓を 1800x1100 にして撮る |
| `board.png` | ボード（`4`）。Project「uchmk」の Todo・In Progress・Done | 窓を 1800x1100 にして撮る |
| `minato.gif` | 受信箱 → 一覧 → ボード → キーの一覧 → 受信箱と回る動き。押したキーを右上に字幕で出す | 窓を 1600x1000 にして ffmpeg の `gdigrab`（15 fps、マウスなし）で撮り、幅 1200 px・10 fps・128 色の GIF にする |
| `banner-shot.png` | `docs/banner.html` の右の実画面。受信箱 | 窓を 1500x1000 にして撮る |

撮るときの注意:

- **GitHub を変えるキーを押さない**（`a`・`x`・`m`・`H`・`L`・`R` など）。撮るのは見るだけのキー（`1`〜`4`・`j`・`k`・`h`・`l`・`F1`・`Esc`）で。
- **公開のリポジトリのものだけが写っているかを、全部のコマで見る。**自分のアカウントのリポジトリには非公開のものもあり、
  一覧の検索（`user:`）・Actions のリポジトリの列・ボードに名前が出ることがある。Actions の画面は撮っていない。
- 設定と状態は持ち主のものを使わない（`MINATO_CONFIG_HOME`・`MINATO_STATE_HOME`・`UCHMK_CONFIG_DIR` を空のフォルダに向ける）。
- 紹介画像は `docs/social-preview.png`（元は `docs/banner.html`。描き直し方は HTML の冒頭）。
  GitHub に API が無いので、Settings > General > Social preview から手で上げる。
