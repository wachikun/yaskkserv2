# yaskkserv2

Rust 製の skkserv です。

以下のような特徴があります。

- SKK 辞書を yaskkserv2 専用の dictionary  <sup>[1](#footnote1)</sup> に変換して使用
- EUC/UTF-8 の SKK 辞書に対応 <sup>[2](#footnote2)</sup>
- 複数 SKK 辞書に対応 <sup>[3](#footnote3)</sup>
- Google Japanese Input / Suggest 対応 <sup>[4](#footnote4)</sup>
- server completion 対応
- 前作 yaskkserv の yaskkserv_hairy <sup>[5](#footnote5)</sup> を Rust でリライトして整理したようなもの

<sub><span id="footnote1">1</span>: この文章では SKK の辞書を **SKK 辞書**、 SKK 辞書を yaskkserv2 用に変換したものを **dictionary** と記述しています。</sub>

<sub><span id="footnote2">2</span>: SKK protocol/上の制約から基本的に EUC に変換されますが、 ddskk に手を入れることで UTF-8 で使用することも可能(詳細は後述)。</sub>

<sub><span id="footnote3">3</span>: 複数の SKK 辞書を yaskkserv2 専用 dictionary へ変換するタイミングでマージして 1 つにします。</sub>

<sub><span id="footnote4">4</span>: デフォルトでは dictionary で変換できなかった場合にのみ Google Japanese Input を使用するため、通常は高速に変換できますが、 dictionary に存在しないレアな単語を変換するような場合だけ、少し変換に時間がかかります。 Google Suggest はデフォルトでは disable となります。</sub>

<sub><span id="footnote5">5</span>: C++ 製の機能てんこもりサーバ。機能を詰め込み過ぎて複雑になってしまったので、 yaskkserv2 はその反省を活かす形で設計されています。</sub>




## install

```console
$ cargo build --release
$ cp -av target/release/yaskkserv2 /YOUR-SBIN-PATH/
$ cp -av target/release/yaskkserv2_make_dictionary /YOUR-BIN-PATH/
```




## 用語の定義

| 用語       | 意味                                              |                         |
|:-----------|:--------------------------------------------------|:------------------------|
| SKK 辞書   | SKK の辞書                                        | SKK-JISYO.L など        |
| dictionary | SKK 辞書から作成される yaskkserv2 用の dictionary |                         |
| midashi    | ユーザが入力する文字列                            | げんじ                  |
| candidate  | 変換結果の個々の文字列                            | 源氏 元治 言辞          |
| candidates | candidate 群                                      | /源氏/元治/言辞/        |
| entry      | midashi と candidates のセット。 SKK 辞書の 1 行  | げんじ /源氏/元治/言辞/ |

基本は http://openlab.ring.gr.jp/skk/wiki/wiki.cgi?page=SKK%CD%D1%B8%EC%BD%B8 に準じています。




## つかいかた

まず `yaskkserv2_make_dictionary` コマンドで、 SKK 辞書から yaskkserv2 用の dictionary を作成する必要があります。

入力の SKK 辞書は EUC でも UTF-8 でもかまいません。デフォルトでは EUC に変換された dictionary が出力されます。

```console
$ yaskkserv2_make_dictionary --dictionary-filename=/tmp/dictionary.yaskkserv2 SKK-JISYO.total+zipcode SKK-JISYO.kancolle
```

以上の操作で `SKK-JISYO.total+zipcode` (EUC) と `SKK-JISYO.kancolle` (UTF-8) から、マージされた `/tmp/dictionary.yaskkserv2` (EUC) が作成されます。

下記のように、作成した dictionary を指定してサーバ `yaskkserv2` を起動します。

```console
# yaskkserv2 /tmp/dictionary.yaskkserv2
```


### 注意


#### 変換できない文字

SKK 辞書に文字コード変換できない文字が含まれる場合は、その文字コードが 16 進で dictionary へ出力されます。具体的には UTF-8 の絵文字などは EUC に変換することができません。

protocol 制約上 client に手を入れる必要はありますが、 UTF-8 辞書の文字コードを変換せずに UTF-8 dictionary を出力する方法もあります(後述)。


#### アーキテクチャ依存

dictionary はアーキテクチャ依存です。異なるアーキテクチャのマシンで作成された dictionary は使用できません。


### Google Japanese Input

デフォルトでは dictionary に candidates が見付からなかった場合に Google Japanese Input API を呼びだします。

`yaskkserv2` に下記のようなオプションを指定することで、 Google Japanese Input の動作を指定できます。

```console
# yaskkserv2 --google-suggest /tmp/dictionary.yaskkserv2
# yaskkserv2 --google-japanese-input=last --google-suggest /tmp/dictionary.yaskkserv2
# yaskkserv2 --google-japanese-input=disable /tmp/dictionary.yaskkserv2
```

`--google-japanese-input` オプションには `notfound`, `disable`, `last` または `first` を指定します。

- `notfound` は dictionary 探索後に candidates が見付からなかった場合のみ Google Japanese Input API を呼びます (デフォルト)
- `disable` は Google Japanese Input API を呼びません
- `last` は dictionary の探索後常に Google Japanese Input API を呼びます (変換のたびに呼ぶので体感でわかるくらい遅いです)
- `first` は dictionary の探索前常に Google Japanese Input API を呼びます (変換のたびに呼ぶので体感でわかるくらい遅いです)

下記のように `--google-cache-filename` オプションで、 Google Japanese Input API の結果をキャッシュすることもできます。デフォルトではキャッシュしません。

キャッシュファイルは排他制御されないため、 yaskkserv2 を複数起動する場合は別のファイルを指定する必要があります。

```console
# yaskkserv2 --google-cache-filename=/tmp/yaskkserv2.cache /tmp/dictionary.yaskkserv2
```


#### 複数の単語に分割される場合

たとえば「あさかい」は「朝会」ではなく「朝/麻/浅/あさ/アサ」と「回/会/かい/界/χ」のように複数の単語に分割されてしまいます。これを「/朝回/朝会/朝かい/朝界/朝χ/浅回/浅会/浅かい/浅界/浅χ/あさ回/あさ会/あさかい/あさ界/あさχ/麻回/麻会/麻かい/麻界/麻χ/アサ回/アサ会/アサかい/アサ界/アサχ/」のようにマージして返します。

マージすると candidates が膨大になるため、上限を `--google-max-candidates-length` オプションで指定できます。 `--google-max-candidates-length` はマージされない場合の上限にも影響します。デフォルトは 25 です。




### UTF-8 dictionary (Emacs)

**SKK protocol は EUC を要求するため、 ddskk の関数 `skk-open-server` を UTF-8 で受信するように書き換える必要があります。**

**前提として、 ddskk で UTF-8 辞書を扱えるよう設定しておく必要があります。**

SKK protocol は EUC を前提としていますが、 ddskk 側でサーバからの受信を UTF-8 に設定し、 yaskkserv2 側で UTF-8 dictionary を使用することで、サーバを UTF-8 辞書と同等に扱うことが可能です。これは EUC 変換を介さないため、 EUC にできない文字もそのまま扱うことができる利点があります。

UTF-8 dictionary は下記のように `--utf8` オプションを渡すことで作成します。

```console
$ yaskkserv2_make_dictionary --utf8 --dictionary-filename=/tmp/dictionary.yaskkserv2 SKK-JISYO.total+zipcode SKK-JISYO.kancolle
```

下記のように `init.el` などで ddskk 側のふるまいを変更すると UTF-8 dictionary を使用できます <sup>[6](#footnote6)</sup>。

```elisp
(defun skk-open-server-decoding-utf-8 ()
  "辞書サーバと接続する。サーバープロセスを返す。 decoding coding-system が euc ではなく utf8 となる。"
  (unless (skk-server-live-p)
    (setq skkserv-process (skk-open-server-1))
    (when (skk-server-live-p)
      (let ((code (cdr (assoc "euc" skk-coding-system-alist))))
	(set-process-coding-system skkserv-process 'utf-8 code))))
  skkserv-process)
(setq skk-mode-hook
      '(lambda()
         (advice-add 'skk-open-server :override 'skk-open-server-decoding-utf-8)))
```

<sub><span id="footnote6">6</span>: client -> server は EUC のまま、 server -> client だけが UTF-8 になります。</sub>




### UTF-8 dictionary (uim-skk など)

UTF-8 対応の uim-skk などで yaskkserv2 を使用する場合は、 `--midashi-utf8` オプションで、 UTF-8 で送信される見出しを受け付けるよう設定する必要があります。

`--midashi-utf8` オプションは、 yaskkserv2 内部で UTF-8 を EUC へ変換するため、絵文字などの見出しは使用できない制限があります(見出しは基本ひらがなであるため、問題はないと思いますが)。




### yaskkserv2 dictionary から SKK 辞書の作成 (逆変換)

下記コマンドで yaskkserv2 dictionary から SKK 辞書を作成することができます。

デフォルトは EUC で、 `--utf8` オプションを渡すことで UTF-8 の SKK 辞書を作成します。

```console
$ yaskkserv2_make_dictionary --dictionary-filename=/tmp/dictionary.yaskkserv2 --output-jisyo-filename=/tmp/SKK-JISYO.euc
$ yaskkserv2_make_dictionary --dictionary-filename=/tmp/dictionary.yaskkserv2 --utf8 --output-jisyo-filename=/tmp/SKK-JISYO.utf8
```


### Google Japanese Input cache から SKK 辞書の作成

下記コマンドで Google Japanese Input cache から SKK 辞書を作成することができます。

```console
$ yaskkserv2_make_dictionary --cache-filename=/tmp/yaskkserv2.cache --output-jisyo-filename=/tmp/SKK-JISYO.euc
$ yaskkserv2_make_dictionary --cache-filename=/tmp/yaskkserv2.cache --utf8 --output-jisyo-filename=/tmp/SKK-JISYO.utf8
```




## yaskkserv との違い

- yaskkserv2 は yaskkserv を Rust でシンプルに再設計して必要な機能のみ残したもの
- Google Japanese Input を標準で有効に
- yaskkserv で複雑だったコマンドライン指定を整理
- dictionary の複数指定は dictionary 作成時にマージしてしまうことで廃止
- dictionary の reload は restart した方が色々と楽なので廃止
- dictionary の複数アーキテクチャ対応は複雑になるので廃止
- 先代の yaskkserv はこのあたりが絡みあい、組み合わせが非常に複雑になってしまったので……
- dictionary も新設計なので yaskkserv との互換性は無し
- SKK 辞書の読み込み時チェックを強化
- SKK 辞書は見出しありなしでソートされてなくとも良い (dictionary 作成時にマージ/ソートするため。 SKK 辞書としては正しくない形式だが)
- yaskkserv は iconv で文字コード変換、 yaskkserv2 は変換表を dictionary などに保持し自前で変換
- yaskkserv は EUC の SKK 辞書のみ対応、 yaskkserv2 は EUC/UTF-8 の SKK 辞書に対応
- yaskkserv はメモリ 32KB 程度で libc すら当てにならない組み込み環境を前提とした設計
- yaskkserv2 はメモリ数十 M 程度の Rust が動作する環境 <sup>[7](#footnote7)</sup> を前提とした設計
- yaskkserv は C++03 で 14000 行
- yaskkserv2 は Rust で 4900 <sup>[8](#footnote8)</sup> 行

<sub><span id="footnote7">7</span>: とはいえ省メモリにはそれなりに気をつかった設計となっています。</sub>

<sub><span id="footnote8">8</span>: test を含めると 9200 行。</sub>




## test について

UNIX 系の環境 <sup>[9](#footnote9)</sup> を必要とし、下記のような動作をします。

- 環境変数 `YASKKSERV2_TEST_DIRECTORY` が指定されていなければ test は失敗
- 環境変数 `YASKKSERV2_TEST_HEAVY` が指定されていなければ、重い test は何もせず成功
- curl で SKK 辞書などを環境変数 `YASKKSERV2_TEST_DIRECTORY` が指すディレクトリにダウンロード <sup>[10](#footnote10)</sup>
- ダウンロードした gzip/tar を展開
- 存在すれば比較用に yaskkserv を起動して benchmark
- gcc で比較用に C 版 echo server を compile して benchmark
- `yaskkserv2` の各種 test
- `yaskkserv2_make_dictionary` の各種 test

環境変数 `YASKKSERV2_TEST_HEAVY` が設定されている場合、 --test-threads=1 でも 100 以上の thread を同時に起動するので、負荷には注意が必要です。また、 OS やリソース量によっては test は常に失敗します。

<sub><span id="footnote9">9</span>: 外部コマンドとして curl や tar などを呼び出します。</sub>

<sub><span id="footnote10">10</span>: 一部、手動でダウンロードしてファイルを配置することで test 可能になるものがあります。</sub>




## benchmark

test 環境は下記のとおりです。

| CPU           | memory | OS                           | rustc        | scaling_governor |
|:--------------|:-------|:-----------------------------|:-------------|:-----------------|
| Ryzen 7 2700X | 64GB   | Linux version 4.14.52-gentoo | rustc 1.38.0 | performance      |

GitHub などから辞書をダウンロードして展開するため、 test 用ディレクトリを指定する環境変数 `YASKKSERV2_TEST_DIRECTORY` を必要とします。この環境変数が存在しない場合、 test は失敗します。

下記コマンドで benchmark を兼ねた test を実行します。

```console
$ export YASKKSERV2_TEST_DIRECTORY=/tmp/yaskkserv2_test_directory
$ cargo test --release benchmark -- --nocapture --test-threads=1
```

または

```console
$ export YASKKSERV2_TEST_DIRECTORY=/tmp/yaskkserv2_test_directory
$ cargo run --release --bin=test_wrapper -- N benchmark
```

で、 N 回 test を実行し、その平均値などを取得できます。


### benchmark 結果

下記コマンドの結果です。 client single thread は 30 回、 client multi thread は 10 回計測したものから最小値最大値を除いて平均を取った値となります。

```console
$ cargo run --release --bin=test_wrapper -- 30 yaskkserv2_benchmark_0
$ cargo run --release --bin=test_wrapper -- 10 yaskkserv2_benchmark_1
$ cargo run --release --bin=test_wrapper -- 30 yaskkserv_benchmark_0
$ cargo run --release --bin=test_wrapper -- 10 yaskkserv_benchmark_1
$ cargo run --release --bin=test_wrapper -- 10 yaskkserv2_benchmark_send_std_net_tcp_test
$ cargo run --release --bin=test_wrapper -- 10 echo_server_benchmark
```

- 単位は requests per second です
- test 内容は 518k entry ある巨大辞書を舐めて結果を比較するものです
- この test 環境での benchmark 結果は、実行するたびに数 k rps. 程度前後することがありました

| client single thread    | yaskkserv2 | yaskkserv | Rust std::net echo | Rust mio echo | C single thread echo |
|:------------------------|:-----------|:----------|:-------------------|---------------|----------------------|
| EUC sequential          | 111k       | 112k      | -                  | -             | -                    |
| EUC random              | 100k       | 95k       | -                  | -             | -                    |
| EUC abbrev sequential   | 101k       | 100k      | -                  | -             | -                    |
| EUC abbrev random       | 90k        | 86k       | -                  | -             | -                    |
| UTF-8 sequential        | 112k       | -         | 130k               | 123k          | 135k                 |
| UTF-8 random            | 99k        | -         | -                  | -             | -                    |
| UTF-8 abbrev sequential | 101k       | -         | -                  | -             | -                    |
| UTF-8 abbrev random     | 91k        | -         | -                  | -             | -                    |

| client multi thread(8)  | yaskkserv2 | yaskkserv | Rust std::net echo | Rust mio echo |
|:------------------------|:-----------|:----------|--------------------|---------------|
| EUC sequential          | 26k        | 27k       | -                  | -             |
| EUC random              | 24k        | 25k       | -                  | -             |
| EUC abbrev sequential   | 22k        | 24k       | -                  | -             |
| EUC abbrev random       | 21k        | 22k       | -                  | -             |
| UTF-8 sequential        | 26k        | -         | 108k               | 35k           |
| UTF-8 random            | 24k        | -         | -                  | -             |
| UTF-8 abbrev sequential | 22k        | -         | -                  | -             |
| UTF-8 abbrev random     | 21k        | -         | -                  | -             |


### 解説

skkserv 自体が単純なサーバである上 yaskkserv2 と yaskkserv の基本的な構造も近いことから、おおまかな benchmark の傾向は似たようなものとなりましたが、ほぼ同程度の rps. となったのは偶然の結果です。これは例えば Rust の I/O library を変更したり、 dictionary パラメータを変更するだけで数 krps. の変動があるためです。

Rust の I/O library に関しては、上記表の single thread std::net echo と mio echo の差を見るとわかりやすく、おおむね 7k rps. の差が出ていることがわかります <sup>[11](#footnote11)</sup>。 yaskkserv2 でも mio::tcp から std::net に置き換えると sequential で yaskkserv を越える 120k rps. 程度まで rps. が向上します。これは下記コマンドで test することができます。

```console
$ cargo test --release yaskkserv2_benchmark_send_std_net_tcp_test -- --nocapture
```

UTF-8 dictionary の abbrev のみサーバ側で EUC の midashi を UTF-8 にする変換処理が入るため、他より遅くなるはずですが、大きな rps. の低下は見られないようです。

client multi thread で std::net echo の rps. が極端に高いのは、サーバが multi thread であるためです。

<sub><span id="footnote11">11</span>: mio でも poll() の timeout を 0 にすると、 echo server で 174k rps. 、 yaskkserv2 でも sequential で 152k rps. 程度まで rps. が向上します。 poll() ではイベント発生を待ちたいため yaskkserv2 では timeout を 0 に設定することはありませんが。</sub>




## yaskkserv2 dictionary

yaskkserv2 dictionary には下記のデータが含まれます。

- header
- encoding\_table (UTF-8 <-> EUC 変換用)
- index\_data\_header
- index\_data
- string blocks (EUC または EUC/UTF-8)

string blocks に含まれる midashi は常に EUC で、 candidates は EUC または UTF-8 になります。

詳細は `src/skk/yaskkserv2_make_dictionary/mod.rs` をご覧ください。




## SKK protocol memo

下記がこの memo の情報源となります。

- skkserv/README, skkserv/skkserv.c
- regex-skkserv の MEMO
- <a href="http://pc10.2ch.net/test/read.cgi/unix/1124001722/74">2ch SKK専用スレッド Part7 の 74 さん</a>


### はじめに

ddskk では変数 `skk-jisyo-code` で辞書の文字コードを設定ができますが、この変数はあくまでも辞書に対するもので、サーバとのやりとりには使用されません。

サーバのやりとりで使用される文字コードは、下記のように常に EUC となります <sup>[12](#footnote12)</sup>。

```elisp
(defun skk-open-server ()
  "辞書サーバと接続する。サーバープロセスを返す。"
  (unless (skk-server-live-p)
    (setq skkserv-process (skk-open-server-1))
    (when (skk-server-live-p)
      (let ((code (cdr (assoc "euc" skk-coding-system-alist))))
	(set-process-coding-system skkserv-process code code))))
  skkserv-process)
```

SKK protocol は protocol によって終端コードがまちまちで、実装には注意が必要です <sup>[13](#footnote13)</sup>。

また、 protocol 1 と 4 の client -> server では改行は必要ありませんが、 server -> client では改行が必要となります。この改行も若干ややこしく、 protocol 1 や 4 で candidates が見付からなかった場合、 server は client に受け取った文字列を加工して返す必要があるのですが、ここで受け取った文字列に改行が無かった場合は改行を付加して返してやる必要があります。

<sub><span id="footnote12">12</span>: なのでサーバから UTF-8 を返したい場合は、 `skk-open-server` で UTF-8 を受け取れるよう変更する必要があります。</sub>

<sub><span id="footnote13">13</span>: Rust の `read_until()` がそのままでは使えない!</sub>


### "0"

| server read 終端コード |
|:-----------------------|
| なし                   |
 

サーバへコネクションを切断するよう要求します。

ddskk が emacs 終了時に送信します。余談ですが、正常に切断されなかった場合 emacs がコネクションを掴んだままになり、 emacs は終了しなくなります。


### "1EucMidashi "

| server read 終端コード | server send 終端コード |
|:-----------------------|:-----------------------|
| スペース               | 改行                   |

EucMidashi に対する candidates を要求します。 EucMidashi は `" "` (スペース)でターミネートされていることに注意が必要です。

サーバから返される candidates は / で区切られた `"1/foo/bar/baz/\n"` のような形式です。

サーバから返される文字列の末尾には `"\n"` が必要なことに注意が必要です。

midashi が存在しない場合は入力の先頭の `"1"` を `"4"` に変換したもの <sup>[14](#footnote14)</sup> を返します。但し、改行が必要になることに注意が必要です。

<sub><span id="footnote14">14</span>: 実は protocol 的には 4 で始まる文字列ならば何でも良いらしいですが、入力を含めないと一部のクライアントで問題が出る場合があるとのことです。</sub>


### "4EucMidashi "

| server read 終端コード | server send 終端コード |
|:-----------------------|:-----------------------|
| スペース               | 改行                   |

EucMidashi で始まる midashi 群を要求します。 EucMidashi は `" "` (スペース)でターミネートされていることに注意が必要です。

サーバから返される midashi は / で区切られた `"1/a/aa/abc/\n"` のような形式です。

サーバから返される文字列の末尾には `"\n"` が必要なことに注意が必要です。

これは新しい protocol で、今のところきちんとした定義は無いようです。

あいまいな点としては

- 「送りなし」 entry と「abbrev」entry のみ返され、「送りあり」 entry は一般に返されない?
- 出現順は?
- midashi が存在しない場合は入力をそのまま返す?
- サーバから返される midashi に完全一致の (要求した) midashi を含める?
- 含められない / などの文字の扱いは?

といったものが挙げられます。

yaskkserv2 では

- 返す midashi は「送りなし」と「abbrev」のみ
- midashi の出現順は内部的なソート順
- midashi が存在しない場合は入力をそのまま返す
- サーバから返される midashi に完全一致の midashi を含める
- 含められない文字は candidates と同様にエスケープされる

といった実装になっています。


### "2"

| server read 終端コード | server send 終端コード |
|:-----------------------|:-----------------------|
| なし                   | スペース               |

サーバへ「バージョンナンバー」を要求します。

サーバから返される「バージョンナンバー」は `"A.B "` のような形式です。 `" "` (スペース)でターミネート
されていることに注意が必要です。


### "3"

| server read 終端コード | server send 終端コード |
|:-----------------------|:-----------------------|
| なし                   | スペース               |

サーバへ「サーバのホスト名と IP アドレスのリスト」を要求します。

サーバから返される「サーバのホスト名と IP アドレスのリスト」は "hostname:addr:[addr...:] " の
ような形式です。 `" "` (スペース)でターミネートされていることに注意が必要です。

yaskkserv2 では未実装です。(ダミー文字列が返されます。)




## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.




## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
