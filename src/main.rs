use std::{
    fs,
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
    p.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

fn preview(p: &Path) -> String {
    match fs::read(p) {
        Err(e) => format!("<{e}>"),
        Ok(bytes) => {
            let head = &bytes[..bytes.len().min(PREVIEW_LIMIT)];
            if head.contains(&0) {
                return format!("<binary: {} bytes>", bytes.len());
            }
            String::from_utf8_lossy(head).replace('\t', "    ")
        }
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

fn main() -> std::io::Result<()> {
    let cwd = match std::env::args().nth(1) {
        Some(a) => PathBuf::from(a).canonicalize()?,
        None => std::env::current_dir()?,
    };
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
                Block::bordered()
                    .title(title)
                    .border_style(if on { Style::new().add_modifier(Modifier::BOLD) } else { Style::new() })
            };

            let dirs = List::new(app.dirs.iter().map(|(n, _)| n.clone()))
                .block(blk(app.cwd.to_string_lossy().into_owned(), app.pane == Pane::Dirs))
                .highlight_style(hl(app.pane == Pane::Dirs));
            f.render_stateful_widget(dirs, l, &mut app.dsel);

            let files = List::new(app.files.iter().map(|p| name(p)))
                .block(blk("files".into(), app.pane == Pane::Files))
                .highlight_style(hl(app.pane == Pane::Files));
            f.render_stateful_widget(files, m, &mut app.fsel);

            let title = app.fsel.selected().map_or("-".to_string(), |i| name(&app.files[i]));
            let body = Paragraph::new(app.body.as_str())
                .block(blk(title, app.pane == Pane::Preview))
                .wrap(Wrap { trim: false });
            // line_count は枠の上下 2 行を含むので、外枠込みの r.height と直接比べられる。
            let lines = body.line_count(r.width.saturating_sub(2)) as u16;
            app.scroll = clamp_scroll(app.scroll, lines, r.height);
            f.render_widget(body.scroll((app.scroll, 0)), r);
        })?;

        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Left | KeyCode::Char('h') => {
                app.pane = if app.pane == Pane::Preview { Pane::Files } else { Pane::Dirs };
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.pane = if app.pane == Pane::Dirs { Pane::Files } else { Pane::Preview };
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                let d = if matches!(k.code, KeyCode::Up | KeyCode::Char('k')) { -1 } else { 1 };
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

    #[test]
    fn reads_this_source_and_rejects_binary() {
        assert!(preview(Path::new(file!())).contains("fn main"));
        let bin = std::env::temp_dir().join("agcat_bin_test");
        fs::write(&bin, [0u8, 1, 2]).unwrap();
        assert!(preview(&bin).starts_with("<binary"));
    }
}
