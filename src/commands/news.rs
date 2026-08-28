use anyhow::Result;
use crossterm::terminal;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

fn get_terminal_width() -> usize {
    terminal::size().map(|(w, _)| w as usize).unwrap_or(80)
}

pub async fn run(count: usize) -> Result<()> {
    let client = Client::new();
    let response = client
        .get("https://godotengine.org/rss.xml")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    let xml = response.text().await?;

    let items: Vec<&str> = xml.split("<item>").skip(1).collect();
    let count = count.min(items.len());

    println!();
    for i in 0..count {
        let item = items[i];
        let title = extract_tag(item, "title").unwrap_or_default();
        let link = extract_tag(item, "link").unwrap_or_default();
        let summary = extract_tag(item, "summary").unwrap_or_default();
        let pub_date = extract_tag(item, "pubDate")
            .map(|d| format_date(&d))
            .unwrap_or_default();

        let max_desc_len = get_terminal_width();
        let truncated = truncate_str(&summary, max_desc_len);

        if i > 0 {
            println!();
        }

        if !pub_date.is_empty() {
            println!("\x1b[90m{}\x1b[0m", pub_date);
        }
        println!("\x1b[36m{}\x1b[0m", title);

        if !truncated.is_empty() {
            println!("\x1b[36m{}\x1b[0m", truncated);
        }
        println!("\x1b[36m{}\x1b[0m", link);
    }
    println!();

    Ok(())
}

pub async fn show_latest(config: &mut Config) -> Result<()> {
    let client = Client::new();
    let response = client
        .get("https://godotengine.org/rss.xml")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    let xml = response.text().await?;

    let item_start = xml.find("<item>").unwrap_or(0);
    let item = &xml[item_start..];

    let title = extract_tag(item, "title").unwrap_or_default();
    let link = extract_tag(item, "link").unwrap_or_default();
    let pub_date = extract_tag(item, "pubDate")
        .map(|d| format_date(&d))
        .unwrap_or_default();
    let summary = extract_tag(item, "summary").unwrap_or_default();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let same_article = config.news.last_shown_url.as_deref() == Some(&link);
    let twelve_hours = 12 * 60 * 60;

    if same_article {
        let recent = config
            .news
            .last_shown_at
            .map(|t| now.saturating_sub(t) < twelve_hours)
            .unwrap_or(false);
        if recent {
            return Ok(());
        }
        if config.news.shown_count.unwrap_or(0) >= 4 {
            return Ok(());
        }
        config.news.shown_count = Some(config.news.shown_count.unwrap_or(0) + 1);
    } else {
        config.news.last_shown_url = Some(link.clone());
        config.news.shown_count = Some(1);
    }
    config.news.last_shown_at = Some(now);

    let term_width = get_terminal_width();
    let indent = "    ";
    let prefix_len = indent.len() + 6;
    let max_desc_len = term_width.saturating_sub(prefix_len);
    let truncated = truncate_str(&summary, max_desc_len);

    println!();
    println!("\x1b[90mLatest news:\x1b[0m");
    if !pub_date.is_empty() {
        println!("\t\x1b[90m{}\x1b[0m", pub_date);
    }
    println!("\t\x1b[36m{}\x1b[0m", title);

    if !truncated.is_empty() {
        println!("\t\x1b[36m{}\x1b[0m", truncated);
    }
    println!("\t\x1b[36m{}\x1b[0m", link);
    println!();

    let _ = config.save();

    Ok(())
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    Some(xml[start..start + end].trim().to_string())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    if max_len <= 3 {
        return s[..max_len].to_string();
    }
    let end = s.floor_char_boundary(max_len - 3);
    format!("{}...", &s[..end])
}

fn format_date(raw: &str) -> String {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() >= 4 {
        format!("{} {}", parts[1], parts[2])
    } else {
        raw.to_string()
    }
}
