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

struct App {
    cwd: PathBuf,
    dirs: Vec<(String, PathBuf)>,
    files: Vec<PathBuf>,
    dsel: ListState,
    fsel: ListState,
    focus_files: bool,
    body: String,
}

impl App {
    fn new(cwd: PathBuf) -> Self {
        let mut app = App {
            cwd,
            dirs: vec![],
            files: vec![],
            dsel: ListState::default(),
            fsel: ListState::default(),
            focus_files: false,
            body: String::new(),
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

            let dirs = List::new(app.dirs.iter().map(|(n, _)| n.clone()))
                .block(Block::bordered().title(app.cwd.to_string_lossy().into_owned()))
                .highlight_style(hl(!app.focus_files));
            f.render_stateful_widget(dirs, l, &mut app.dsel);

            let files = List::new(app.files.iter().map(|p| name(p)))
                .block(Block::bordered().title("files"))
                .highlight_style(hl(app.focus_files));
            f.render_stateful_widget(files, m, &mut app.fsel);

            let title = app.fsel.selected().map_or("-".to_string(), |i| name(&app.files[i]));
            f.render_widget(
                Paragraph::new(app.body.as_str())
                    .block(Block::bordered().title(title))
                    .wrap(Wrap { trim: false }),
                r,
            );
        })?;

        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Left | KeyCode::Char('h') => app.focus_files = false,
            KeyCode::Right | KeyCode::Char('l') => app.focus_files = true,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                let d = if matches!(k.code, KeyCode::Up | KeyCode::Char('k')) { -1 } else { 1 };
                if app.focus_files {
                    step(&mut app.fsel, app.files.len(), d);
                    app.reload_body();
                } else {
                    step(&mut app.dsel, app.dirs.len(), d);
                    app.reload_files();
                }
            }
            KeyCode::Enter if !app.focus_files => {
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
    fn reads_this_source_and_rejects_binary() {
        assert!(preview(Path::new(file!())).contains("fn main"));
        let bin = std::env::temp_dir().join("agcat_bin_test");
        fs::write(&bin, [0u8, 1, 2]).unwrap();
        assert!(preview(&bin).starts_with("<binary"));
    }
}
