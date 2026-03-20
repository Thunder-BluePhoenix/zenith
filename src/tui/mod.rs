/// Zenith terminal dashboard — ratatui two-pane TUI.
///
/// Layout:
///   ┌─────────────────────────────────────┐
///   │  Left: run list  │  Right: steps    │
///   │  (scrollable)    │  (step log view) │
///   └─────────────────────────────────────┘
///
/// Key bindings:
///   ↑/k  — move selection up
///   ↓/j  — move selection down
///   Enter — expand/collapse step log
///   r     — refresh runs
///   q/Esc — quit

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::ui::history::{self, RunOutcome, RunSummary, StepStatus};

// ─── App state ────────────────────────────────────────────────────────────────

struct App {
    runs:           Vec<RunSummary>,
    run_state:      ListState,
    steps:          Vec<history::StepEvent>,
    step_state:     ListState,
    expanded_step:  Option<usize>,   // index of step whose logs are expanded
    focus:          Pane,
}

#[derive(PartialEq)]
enum Pane { Runs, Steps }

impl App {
    fn new() -> Self {
        let mut app = Self {
            runs:          vec![],
            run_state:     ListState::default(),
            steps:         vec![],
            step_state:    ListState::default(),
            expanded_step: None,
            focus:         Pane::Runs,
        };
        app.refresh_runs();
        app
    }

    fn refresh_runs(&mut self) {
        self.runs = history::list_runs(50);
        // Keep selection or reset to first
        if self.runs.is_empty() {
            self.run_state.select(None);
            self.steps.clear();
        } else {
            let sel = self.run_state.selected().unwrap_or(0).min(self.runs.len() - 1);
            self.run_state.select(Some(sel));
            self.load_steps_for_selected();
        }
    }

    fn load_steps_for_selected(&mut self) {
        if let Some(i) = self.run_state.selected() {
            if let Some(run) = self.runs.get(i) {
                self.steps = history::get_steps(&run.run_id);
                self.step_state.select(if self.steps.is_empty() { None } else { Some(0) });
                self.expanded_step = None;
            }
        }
    }

    fn move_run(&mut self, delta: i32) {
        if self.runs.is_empty() { return; }
        let cur = self.run_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, self.runs.len() as i32 - 1) as usize;
        self.run_state.select(Some(next));
        self.load_steps_for_selected();
    }

    fn move_step(&mut self, delta: i32) {
        if self.steps.is_empty() { return; }
        let cur = self.step_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, self.steps.len() as i32 - 1) as usize;
        self.step_state.select(Some(next));
    }

    fn toggle_step_log(&mut self) {
        if let Some(sel) = self.step_state.selected() {
            if self.expanded_step == Some(sel) {
                self.expanded_step = None;
            } else {
                self.expanded_step = Some(sel);
            }
        }
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let result = run_app(&mut term);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    term.show_cursor()?;

    result
}

fn run_app(term: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();

    loop {
        term.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,

                    KeyCode::Char('r') => app.refresh_runs(),

                    KeyCode::Tab => {
                        app.focus = if app.focus == Pane::Runs { Pane::Steps } else { Pane::Runs };
                    }

                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.focus == Pane::Runs { app.move_run(-1); }
                        else { app.move_step(-1); }
                    }

                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.focus == Pane::Runs { app.move_run(1); }
                        else { app.move_step(1); }
                    }

                    KeyCode::Right | KeyCode::Enter => {
                        if app.focus == Pane::Runs {
                            app.focus = Pane::Steps;
                        } else {
                            app.toggle_step_log();
                        }
                    }

                    KeyCode::Left => {
                        app.focus = Pane::Runs;
                    }

                    _ => {}
                }
            }
        }
    }
    Ok(())
}

// ─── UI rendering ─────────────────────────────────────────────────────────────

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.size();

    // Title bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let title = Paragraph::new("  Zenith Dashboard  |  q:quit  r:refresh  Tab:switch  ↑↓:navigate  Enter:expand")
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    f.render_widget(title, chunks[0]);

    // Main two-pane layout
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(42), Constraint::Min(0)])
        .split(chunks[1]);

    render_run_list(f, app, main[0]);
    render_steps(f, app, main[1]);

    // Status bar
    let status_text = if app.runs.is_empty() {
        "  No runs found. Run `zenith run` to start.".to_string()
    } else {
        let sel = app.run_state.selected().unwrap_or(0);
        if let Some(run) = app.runs.get(sel) {
            format!("  Run: {}  |  Job: {}  |  Steps: {}/{} OK",
                &run.run_id[..16], run.job, run.steps_ok, run.step_count)
        } else {
            "".to_string()
        }
    };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::DarkGray).bg(Color::Black));
    f.render_widget(status, chunks[2]);
}

fn render_run_list(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Pane::Runs;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app.runs.iter().map(|r| {
        let (dot, dot_color) = match r.status {
            RunOutcome::Success => ("● ", Color::Green),
            RunOutcome::Failed  => ("● ", Color::Red),
            RunOutcome::Running => ("◉ ", Color::Yellow),
        };
        let line = Line::from(vec![
            Span::styled(dot, Style::default().fg(dot_color)),
            Span::styled(
                format!("{:<16}  {}", &r.run_id[..16.min(r.run_id.len())], r.job),
                Style::default().fg(Color::White),
            ),
        ]);
        ListItem::new(line)
    }).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Runs ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(236))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.run_state);
}

fn render_steps(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Pane::Steps;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // If a step log is expanded, split area
    if let Some(exp) = app.expanded_step {
        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        render_step_list(f, app, v_chunks[0], focused, border_style);
        render_log_pane(f, app, exp, v_chunks[1]);
    } else {
        render_step_list(f, app, area, focused, border_style);
    }
}

fn render_step_list(f: &mut Frame, app: &mut App, area: Rect, _focused: bool, border_style: Style) {
    let items: Vec<ListItem> = app.steps.iter().map(|s| {
        let (tag, tag_color) = match s.status {
            StepStatus::Done    => ("[DONE]   ", Color::Green),
            StepStatus::Failed  => ("[FAILED] ", Color::Red),
            StepStatus::Started => ("[RUN]    ", Color::Yellow),
            StepStatus::Cached  => ("[CACHED] ", Color::Cyan),
        };
        let log_hint = if !s.log_lines.is_empty() { " ▸" } else { "" };
        let line = Line::from(vec![
            Span::styled(tag, Style::default().fg(tag_color).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}{}", s.name, log_hint),
                Style::default().fg(Color::White),
            ),
        ]);
        ListItem::new(line)
    }).collect();

    let block = Block::default()
        .title(" Steps  (Enter to expand logs) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    if items.is_empty() {
        let msg = Paragraph::new("\n  No steps recorded for this run.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(236))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.step_state);
}

fn render_log_pane(f: &mut Frame, app: &App, step_idx: usize, area: Rect) {
    let (title, content) = if let Some(step) = app.steps.get(step_idx) {
        let t = format!(" Logs: {} ", step.name);
        let c = if step.log_lines.is_empty() {
            "(no log output captured)".to_string()
        } else {
            step.log_lines.join("\n")
        };
        (t, c)
    } else {
        (" Logs ".to_string(), String::new())
    };

    let para = Paragraph::new(content)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}
