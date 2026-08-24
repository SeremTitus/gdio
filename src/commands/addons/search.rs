use crate::config::Config;
use anyhow::{Context, Result};
use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::{
    Terminal,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};
use std::io;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const ID_WIDTH: usize = 35;
const PAGE_SIZE: usize = 15;

#[derive(Clone)]
struct SearchResult {
    identifier: String,
    description: String,
    store_url: String,
}

struct RepoState {
    url: String,
    scroll: Option<String>,
    exhausted: bool,
}

// 6 = left border (1) + right border (1) + highlight symbol ">> " (3) + gap (1)
const UI_OVERHEAD: usize = 6;

fn desc_width(width: u16) -> usize {
    (width as usize).saturating_sub(ID_WIDTH + 1 + UI_OVERHEAD)
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let budget = max_width.saturating_sub(3);
    let mut w = 0;
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        let cw = c.width().unwrap_or(0);
        if w + cw > budget {
            end = i;
            break;
        }
        w += cw;
    }
    if end < s.len() {
        format!("{}...", &s[..end])
    } else {
        s.to_string()
    }
}

fn fuzzy_match(query: &str, item: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query_chars: Vec<char> = query.to_lowercase().chars().collect();
    let li = item.to_lowercase();
    let mut qi = 0;
    for c in li.chars() {
        if qi < query_chars.len() && c == query_chars[qi] {
            qi += 1;
        }
    }
    qi == query_chars.len()
}

async fn fetch_more_pages(
    client: &reqwest::Client,
    query: &str,
    repo_states: &mut [RepoState],
    results: &mut Vec<SearchResult>,
) {
    for state in repo_states.iter_mut() {
        if state.exhausted || state.scroll.is_none() {
            continue;
        }
        let scroll = state.scroll.clone();
        if let Ok(page) = fetch_page(client, &state.url, query, scroll.as_deref()).await {
            state.scroll = page.scroll;
            if page.items.is_empty() || state.scroll.is_none() {
                state.exhausted = true;
            }
            for item in page.items {
                results.push(item);
            }
        } else {
            state.exhausted = true;
        }
    }
}

fn build_items(results: &[SearchResult], width: u16) -> Vec<String> {
    let dw = desc_width(width);
    results
        .iter()
        .map(|r| {
            let id = truncate_to_width(&r.identifier, ID_WIDTH);
            let desc = truncate_to_width(&r.description, dw);
            let pad = ID_WIDTH.saturating_sub(id.width());
            format!("{}{} {}", id, " ".repeat(pad), desc)
        })
        .collect()
}

fn run_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    all_items: &[String],
    repo_states: &[RepoState],
    filter: &str,
    cursor: usize,
    matched: &[usize],
) -> Result<bool> {
    let has_more = repo_states
        .iter()
        .any(|s| !s.exhausted && s.scroll.is_some());

    terminal.draw(|f| {
        let area = f.area();
        f.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(if filter.is_empty() { 0 } else { 1 }),
            ])
            .split(area);

        let header = Line::from(vec![Span::styled(
            "Press Ctrl+C to exit, type to filter, Enter to select",
            Style::default().fg(Color::Blue),
        )]);
        let header_widget = Block::default().borders(Borders::BOTTOM).title(header);
        f.render_widget(header_widget, chunks[0]);

        let items: Vec<ListItem> = matched
            .iter()
            .map(|&idx| {
                let style = if idx == cursor {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(all_items[idx].clone(), style)))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT))
            .highlight_symbol(">> ");

        let mut state = ListState::default();
        state.select(Some(cursor));

        f.render_stateful_widget(list, chunks[1], &mut state);

        if !filter.is_empty() {
            let filter_line = Line::from(Span::styled(
                format!("Filter: {}", filter),
                Style::default().fg(Color::Yellow),
            ));
            f.render_widget(ratatui::widgets::Paragraph::new(filter_line), chunks[2]);
        }
    })?;

    Ok(has_more)
}

struct CleanupGuard;

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, Show);
    }
}

pub async fn run(query: &str, config: &Config) -> Result<()> {
    let client = reqwest::Client::builder().user_agent("gdio").build()?;
    let repos = &config.addons.repositories;

    if repos.is_empty() {
        println!("No repositories configured.");
        return Ok(());
    }

    let mut repo_states: Vec<RepoState> = repos
        .iter()
        .map(|r| RepoState {
            url: r.url.clone(),
            scroll: None,
            exhausted: false,
        })
        .collect();

    let mut results: Vec<SearchResult> = Vec::new();
    let mut all_failed = true;

    for state in &mut repo_states {
        match fetch_page(&client, &state.url, query, None).await {
            Ok(page) => {
                all_failed = false;
                state.scroll = page.scroll;
                if page.items.is_empty() || state.scroll.is_none() {
                    state.exhausted = true;
                }
                for item in page.items {
                    results.push(item);
                }
            }
            Err(e) => {
                state.exhausted = true;
                eprintln!("Warning: failed to fetch from {}: {}", state.url, e);
            }
        }
    }

    if results.is_empty() {
        if all_failed {
            println!("Failed to fetch from any repository for '{}'.", query);
        } else {
            println!("No results found for '{}'.", query);
        }
        return Ok(());
    }

    enable_raw_mode()?;
    let _guard = CleanupGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut filter = String::new();
    let mut cursor: usize = 0;

    loop {
        let width = terminal.size()?.width;
        let all_items = build_items(&results, width);
        let matched: Vec<usize> = all_items
            .iter()
            .enumerate()
            .filter(|(_, item)| fuzzy_match(&filter, item))
            .map(|(i, _)| i)
            .collect();

        if cursor >= matched.len() {
            cursor = matched.len().saturating_sub(1);
        }

        let has_more = run_ui(
            &mut terminal,
            &all_items,
            &repo_states,
            &filter,
            cursor,
            &matched,
        )?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => {
                    terminal.show_cursor()?;
                    anyhow::bail!("Cancelled");
                }
                KeyCode::Char('c')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    terminal.show_cursor()?;
                    anyhow::bail!("Cancelled");
                }
                KeyCode::Enter => {
                    if matched.is_empty() {
                        continue;
                    }
                    let selected_idx = matched[cursor];
                    let selected = &results[selected_idx];

                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                    terminal.show_cursor()?;

                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(selected.identifier.clone());
                    }

                    let parts: Vec<&str> = selected.identifier.splitn(2, '/').collect();
                    let publisher = parts.first().unwrap_or(&"?");
                    let asset = parts.get(1).unwrap_or(&"?");

                    println!();
                    println!("Selected: {}/{}", publisher, asset);
                    println!("{} copied to clipboard", selected.identifier);
                    println!();
                    println!("  To install: gdio addons add {}", selected.identifier);
                    println!("  Store page: {}", selected.store_url);

                    return Ok(());
                }
                KeyCode::Up => {
                    cursor = cursor.saturating_sub(1);
                }
                KeyCode::Down => {
                    if cursor + 1 < matched.len() {
                        cursor += 1;
                    } else if has_more {
                        fetch_more_pages(&client, query, &mut repo_states, &mut results).await;
                    }
                }
                KeyCode::Home => {
                    cursor = 0;
                }
                KeyCode::End => {
                    let new_cursor = matched.len().saturating_sub(1);
                    if new_cursor > cursor {
                        cursor = new_cursor;
                    } else if has_more {
                        fetch_more_pages(&client, query, &mut repo_states, &mut results).await;
                    }
                }
                KeyCode::PageUp => {
                    cursor = cursor.saturating_sub(PAGE_SIZE);
                }
                KeyCode::PageDown => {
                    let new_cursor = (cursor + PAGE_SIZE).min(matched.len().saturating_sub(1));
                    if new_cursor > cursor {
                        cursor = new_cursor;
                    } else if has_more {
                        fetch_more_pages(&client, query, &mut repo_states, &mut results).await;
                    }
                }
                KeyCode::Backspace => {
                    filter.pop();
                    cursor = 0;
                }
                KeyCode::Char(c) => {
                    filter.push(c);
                    cursor = 0;
                }
                _ => {}
            }
        }
    }
}

struct FetchResult {
    items: Vec<SearchResult>,
    scroll: Option<String>,
}

async fn fetch_page(
    client: &reqwest::Client,
    store_url: &str,
    query: &str,
    scroll_token: Option<&str>,
) -> Result<FetchResult> {
    let base = store_url.trim_end_matches('/');
    let mut url = reqwest::Url::parse(&format!("{}/api/v1/search/query/", base))
        .context("Failed to parse store URL")?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("batch_size", &PAGE_SIZE.to_string())
        .append_pair("type", "0");
    if let Some(token) = scroll_token {
        url.query_pairs_mut().append_pair("scroll", token);
    }
    let url = url.to_string();

    let resp = client
        .get(&url)
        .send()
        .await
        .context("Failed to search asset store")?;

    if !resp.status().is_success() {
        anyhow::bail!("Search failed: HTTP {}", resp.status());
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse search results")?;

    let hits = data["hits"].as_array().cloned().unwrap_or_default();

    let next_scroll = data["scroll"].as_str().map(|s| s.to_string());

    let mut items = Vec::new();
    for hit in &hits {
        let asset = &hit["asset"];
        let publisher = asset["publisher"]["slug"].as_str().unwrap_or("?");
        let slug = asset["slug"].as_str().unwrap_or("?");
        let desc = asset["description"].as_str().unwrap_or("");
        let desc = if desc.is_empty() {
            "(no description)"
        } else {
            desc
        };
        let store = asset["store_url"].as_str().unwrap_or("").to_string();

        items.push(SearchResult {
            identifier: format!("{}/{}", publisher, slug),
            description: desc.to_string(),
            store_url: store,
        });
    }

    Ok(FetchResult {
        items,
        scroll: next_scroll,
    })
}
