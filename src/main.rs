use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, List, ListState, Paragraph, Wrap},
};

const PREVIEW_LIMIT: usize = 64 * 1024;

/// dir が持つエントリのうち、ディレクトリ/ファイルの一方だけを名前順で返す。隠しファイルは除外。
fn entries(dir: &Path, want_dir: bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() == want_dir && !name(p).starts_with('.'))
        .collect();
    v.sort();
    v
}

fn name(p: &Path) -> String {
    p.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// 先頭 PREVIEW_LIMIT バイトだけを読む。全体を読むと、大きいファイルにカーソルを
/// 合わせた瞬間に止まってしまう。
fn preview(p: &Path) -> String {
    // 名前付きパイプは書き込み側が現れるまで open が返らないので、開く前に stat で弾く。
    // デバイスファイルも同じ経路で除ける。
    let read_head = || -> std::io::Result<(Vec<u8>, u64)> {
        let md = fs::metadata(p)?;
        if !md.is_file() {
            return Err(std::io::Error::other("not a regular file"));
        }
        let mut buf = Vec::new();
        fs::File::open(p)?
            .take(PREVIEW_LIMIT as u64)
            .read_to_end(&mut buf)?;
        Ok((buf, md.len()))
    };
    match read_head() {
        Err(e) => format!("<{e}>"),
        Ok((head, size)) if head.contains(&0) => format!("<binary: {size} bytes>"),
        Ok((head, _)) => String::from_utf8_lossy(&head).replace('\t', "    "),
    }
}

fn step(state: &mut ListState, len: usize, d: isize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = state.selected().unwrap_or(0) as isize + d;
    state.select(Some(i.clamp(0, len as isize - 1) as usize));
}

/// 折り返し後の総行数 lines と表示高 height から、スクロール位置を可視範囲に収める。
fn clamp_scroll(scroll: u16, lines: u16, height: u16) -> u16 {
    scroll.min(lines.saturating_sub(height))
}

#[derive(Clone, Copy, PartialEq)]
enum Pane {
    Dirs,
    Files,
    Preview,
}

struct App {
    cwd: PathBuf,
    dirs: Vec<(String, PathBuf)>,
    files: Vec<PathBuf>,
    dsel: ListState,
    fsel: ListState,
    pane: Pane,
    body: String,
    scroll: u16,
}

impl App {
    fn new(cwd: PathBuf) -> Self {
        let mut app = App {
            cwd,
            dirs: vec![],
            files: vec![],
            dsel: ListState::default(),
            fsel: ListState::default(),
            pane: Pane::Dirs,
            body: String::new(),
            scroll: 0,
        };
        app.reload_dirs();
        app
    }

    fn reload_dirs(&mut self) {
        self.dirs = vec![(".".into(), self.cwd.clone())];
        if let Some(parent) = self.cwd.parent() {
            self.dirs.push(("..".into(), parent.to_path_buf()));
        }
        self.dirs
            .extend(entries(&self.cwd, true).into_iter().map(|p| (name(&p), p)));
        self.dsel.select(Some(0));
        self.reload_files();
    }

    fn reload_files(&mut self) {
        self.files = match self.dsel.selected() {
            Some(i) => entries(&self.dirs[i].1, false),
            None => vec![],
        };
        self.fsel.select((!self.files.is_empty()).then_some(0));
        self.reload_body();
    }

    fn reload_body(&mut self) {
        self.body = match self.fsel.selected() {
            Some(i) => preview(&self.files[i]),
            None => String::new(),
        };
        self.scroll = 0;
    }
}

const USAGE: &str = "\
Usage: agcat [DIR]

Arguments:
  DIR    開始ディレクトリ (省略時はカレントディレクトリ)

Options:
  -h, --help       このヘルプを表示
  -V, --version    バージョンを表示
";

/// 引数の解釈結果。Start はディレクトリとして実在を確認済みのパスを持つ。
enum Cli {
    Start(PathBuf),
    Help,
    Version,
    Error(String),
}

/// 実行ファイル名を除いた引数列を解釈する。実在確認とディレクトリ判定まで行う。
fn cli(args: &[String]) -> Cli {
    let mut dir: Option<&str> = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => return Cli::Help,
            "-V" | "--version" => return Cli::Version,
            s if s.starts_with('-') => return Cli::Error(format!("unknown option: {s}")),
            s if dir.is_some() => return Cli::Error(format!("unexpected argument: {s}")),
            s => dir = Some(s),
        }
    }
    match dir {
        None => match std::env::current_dir() {
            Ok(p) => Cli::Start(p),
            Err(e) => Cli::Error(format!("current directory: {e}")),
        },
        // canonicalize はシンボリックリンクを解決するので、その先が実際に
        // ディレクトリかどうかを見る。
        Some(d) => match PathBuf::from(d).canonicalize() {
            Err(e) => Cli::Error(format!("{d}: {e}")),
            Ok(p) if !p.is_dir() => Cli::Error(format!("{d}: not a directory")),
            Ok(p) => Cli::Start(p),
        },
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = match cli(&args) {
        Cli::Start(p) => p,
        Cli::Help => {
            print!("{USAGE}");
            return std::process::ExitCode::SUCCESS;
        }
        Cli::Version => {
            println!("agcat {}", env!("CARGO_PKG_VERSION"));
            return std::process::ExitCode::SUCCESS;
        }
        Cli::Error(m) => {
            eprintln!("agcat: {m}");
            eprintln!("Try 'agcat --help' for more information.");
            return std::process::ExitCode::from(2);
        }
    };
    match run(cwd) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("agcat: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cwd: PathBuf) -> std::io::Result<()> {
    let mut app = App::new(cwd);
    let mut term = ratatui::init();

    loop {
        term.draw(|f| {
            let [l, m, r] = Layout::horizontal([
                Constraint::Percentage(20),
                Constraint::Percentage(25),
                Constraint::Percentage(55),
            ])
            .areas(f.area());

            let hl = |on: bool| {
                if on {
                    Style::new().add_modifier(Modifier::REVERSED)
                } else {
                    Style::new().add_modifier(Modifier::BOLD)
                }
            };
            // アクティブなペインは枠を太字にする。
            let blk = |title: String, on: bool| {
                Block::bordered().title(title).border_style(if on {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                })
            };

            let dirs = List::new(app.dirs.iter().map(|(n, _)| n.clone()))
                .block(blk(
                    app.cwd.to_string_lossy().into_owned(),
                    app.pane == Pane::Dirs,
                ))
                .highlight_style(hl(app.pane == Pane::Dirs));
            f.render_stateful_widget(dirs, l, &mut app.dsel);

            let files = List::new(app.files.iter().map(|p| name(p)))
                .block(blk("files".into(), app.pane == Pane::Files))
                .highlight_style(hl(app.pane == Pane::Files));
            f.render_stateful_widget(files, m, &mut app.fsel);

            let title = app
                .fsel
                .selected()
                .map_or("-".to_string(), |i| name(&app.files[i]));
            let body = Paragraph::new(app.body.as_str())
                .block(blk(title, app.pane == Pane::Preview))
                .wrap(Wrap { trim: false });
            // line_count は枠の上下 2 行を含むので、外枠込みの r.height と直接比べられる。
            let lines = body.line_count(r.width.saturating_sub(2)) as u16;
            app.scroll = clamp_scroll(app.scroll, lines, r.height);
            f.render_widget(body.scroll((app.scroll, 0)), r);
        })?;

        let Event::Key(k) = event::read()? else {
            continue;
        };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Left | KeyCode::Char('h') => {
                app.pane = if app.pane == Pane::Preview {
                    Pane::Files
                } else {
                    Pane::Dirs
                };
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.pane = if app.pane == Pane::Dirs {
                    Pane::Files
                } else {
                    Pane::Preview
                };
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                let d = if matches!(k.code, KeyCode::Up | KeyCode::Char('k')) {
                    -1
                } else {
                    1
                };
                match app.pane {
                    // 下端のクランプは折り返し後の行数に依存するので、描画時に行う。
                    Pane::Preview => app.scroll = app.scroll.saturating_add_signed(d as i16),
                    Pane::Files => {
                        step(&mut app.fsel, app.files.len(), d);
                        app.reload_body();
                    }
                    Pane::Dirs => {
                        step(&mut app.dsel, app.dirs.len(), d);
                        app.reload_files();
                    }
                }
            }
            KeyCode::Enter if app.pane == Pane::Dirs => {
                if let Some(i) = app.dsel.selected() {
                    if let Ok(p) = app.dirs[i].1.canonicalize() {
                        app.cwd = p;
                        app.reload_dirs();
                    }
                }
            }
            _ => {}
        }
    }

    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_and_clamps() {
        let mut s = ListState::default();
        step(&mut s, 3, 1);
        assert_eq!(s.selected(), Some(1));
        step(&mut s, 3, -5);
        assert_eq!(s.selected(), Some(0));
        step(&mut s, 3, 9);
        assert_eq!(s.selected(), Some(2));
        step(&mut s, 0, 1);
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn scroll_clamps_to_last_page() {
        // 30 行を高さ 10 で見るなら、末尾が最終行に来る 20 が上限。
        assert_eq!(clamp_scroll(5, 30, 10), 5);
        assert_eq!(clamp_scroll(20, 30, 10), 20);
        assert_eq!(clamp_scroll(999, 30, 10), 20);
        // 全体が収まるなら一切スクロールしない。
        assert_eq!(clamp_scroll(7, 3, 10), 0);
    }

    #[test]
    fn scroll_reaches_end_of_wrapped_body() {
        // 折り返しを含めた実行数を ratatui に数えさせ、末尾まで到達できることを確かめる。
        let body = "abcdefghij ".repeat(20);
        let p = Paragraph::new(body.as_str())
            .block(Block::bordered())
            .wrap(Wrap { trim: false });
        let (w, h) = (22u16, 10u16);
        let lines = p.line_count(w - 2) as u16;
        assert!(lines > h, "折り返しで画面より長くなるはず: {lines}");
        assert_eq!(clamp_scroll(u16::MAX, lines, h), lines - h);
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Cli::Error のメッセージを取り出す。他のバリアントならテストを落とす。
    fn err(v: &[&str]) -> String {
        match cli(&args(v)) {
            Cli::Error(m) => m,
            _ => panic!("expected an error for {v:?}"),
        }
    }

    #[test]
    fn accepts_a_directory_and_defaults_to_cwd() {
        let here = Path::new(file!()).parent().unwrap().to_str().unwrap();
        match cli(&args(&[here])) {
            Cli::Start(p) => assert!(p.is_dir() && p.is_absolute()),
            _ => panic!("directory should be accepted"),
        }
        match cli(&args(&[])) {
            Cli::Start(p) => assert_eq!(p, std::env::current_dir().unwrap()),
            _ => panic!("no argument should fall back to cwd"),
        }
    }

    #[test]
    fn rejects_a_file_and_names_the_bad_path() {
        // ファイルは実在しても開始ディレクトリにはできない。
        assert_eq!(err(&[file!()]), format!("{}: not a directory", file!()));
        // 存在しないパスは、原因と一緒にパス自体をメッセージへ含める。
        let m = err(&["/nonexistent/path"]);
        assert!(m.starts_with("/nonexistent/path: "), "path missing: {m}");
        assert!(m.len() > "/nonexistent/path: ".len(), "reason missing: {m}");
    }

    #[test]
    fn handles_options_and_extra_arguments() {
        assert!(matches!(cli(&args(&["-h"])), Cli::Help));
        assert!(matches!(cli(&args(&["--help"])), Cli::Help));
        assert!(matches!(cli(&args(&["-V"])), Cli::Version));
        assert!(matches!(cli(&args(&["--version"])), Cli::Version));
        // 未知のオプションはディレクトリ名として扱わない。
        assert_eq!(err(&["--bogus"]), "unknown option: --bogus");
        assert_eq!(err(&["src", "extra"]), "unexpected argument: extra");
        // ヘルプは他の引数より優先し、検証を待たずに返す。
        assert!(matches!(cli(&args(&["/nonexistent", "--help"])), Cli::Help));
    }

    #[test]
    fn reads_this_source_and_rejects_binary() {
        assert!(preview(Path::new(file!())).contains("fn main"));
        let bin = std::env::temp_dir().join("agcat_bin_test");
        fs::write(&bin, [0u8, 1, 2]).unwrap();
        assert_eq!(preview(&bin), "<binary: 3 bytes>");
    }

    #[test]
    fn stops_reading_at_the_preview_limit() {
        // 上限を超える分は読まない。上限ちょうどで打ち切られていることを長さで見る。
        let big = std::env::temp_dir().join("agcat_big_test");
        fs::write(&big, "a".repeat(PREVIEW_LIMIT * 2 + 1)).unwrap();
        assert_eq!(preview(&big).len(), PREVIEW_LIMIT);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_files_that_would_never_end() {
        // /dev/zero は読めば無限に 0 を返す。通常ファイルでないものは読まずに諦める。
        assert_eq!(preview(Path::new("/dev/zero")), "<not a regular file>");
    }
}
