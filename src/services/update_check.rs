// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::Deserialize;

const API_ROOT: &str = "https://api.github.com/repos/lgse/strata/releases";
const RELEASES_URL: &str = "https://github.com/lgse/strata/releases";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseMetadata {
    pub version: String,
    pub url: String,
    pub notes: String,
    pub notes_markup: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateCheck {
    UpToDate,
    Available {
        release: ReleaseMetadata,
        download_url: Option<String>,
    },
    Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseNotes {
    Found(ReleaseMetadata),
    Unavailable { url: String },
    Failed { message: String, url: String },
}

#[derive(Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// The asset naming convention published by `.github/workflows/release.yml`.
fn archive_name(version: &str) -> String {
    format!(
        "strata-{version}-{}-unknown-linux-gnu.tar.gz",
        std::env::consts::ARCH
    )
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into()
}

fn request_release(url: &str) -> Result<ReleaseResponse, ureq::Error> {
    agent()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "strata-file-manager")
        .call()
        .and_then(|mut response| response.body_mut().read_json::<ReleaseResponse>())
}

fn metadata(release: &ReleaseResponse) -> ReleaseMetadata {
    let notes = release.body.clone().unwrap_or_default();
    ReleaseMetadata {
        version: release.tag_name.trim_start_matches('v').to_owned(),
        url: release.html_url.clone(),
        notes_markup: markdown_to_pango(&notes),
        notes,
    }
}

fn escape_markup(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Converts the supported GitHub Markdown subset to inert Pango markup. This is
/// called while release metadata is being processed on a worker thread.
fn markdown_to_pango(markdown: &str) -> String {
    let mut output = String::new();
    let mut links = Vec::new();
    let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => output.push_str("<span size=\"large\"><b>"),
            Event::End(TagEnd::Heading(_)) => output.push_str("</b></span>\n"),
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => output.push_str("\n\n"),
            Event::Start(Tag::Item) => output.push_str("•  "),
            Event::End(TagEnd::Item) => output.push('\n'),
            Event::Start(Tag::Emphasis) => output.push_str("<i>"),
            Event::End(TagEnd::Emphasis) => output.push_str("</i>"),
            Event::Start(Tag::Strong) => output.push_str("<b>"),
            Event::End(TagEnd::Strong) => output.push_str("</b>"),
            Event::Start(Tag::Strikethrough) => output.push_str("<s>"),
            Event::End(TagEnd::Strikethrough) => output.push_str("</s>"),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let destination = dest_url.as_ref();
                let external =
                    destination.starts_with("https://") || destination.starts_with("http://");
                links.push(external);
                if external {
                    output.push_str("<a href=\"");
                    output.push_str(&escape_markup(destination));
                    output.push_str("\">");
                } else {
                    output.push_str("<u>");
                }
            }
            Event::End(TagEnd::Link) => output.push_str(if links.pop().unwrap_or(false) {
                "</a>"
            } else {
                "</u>"
            }),
            Event::Start(Tag::Image { .. }) => output.push_str("[Image: "),
            Event::End(TagEnd::Image) => output.push(']'),
            Event::Start(Tag::CodeBlock(_)) => output.push_str("<tt>"),
            Event::End(TagEnd::CodeBlock) => output.push_str("</tt>\n"),
            Event::Code(text) => {
                output.push_str("<tt>");
                output.push_str(&escape_markup(&text));
                output.push_str("</tt>");
            }
            Event::Text(text) => output.push_str(&escape_markup(&text)),
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::Rule => output.push_str("────────\n"),
            Event::Html(text) | Event::InlineHtml(text) => output.push_str(&escape_markup(&text)),
            Event::TaskListMarker(checked) => {
                output.push_str(if checked { "☑ " } else { "☐ " })
            }
            _ => {}
        }
    }
    output.trim().to_owned()
}

/// Queries the latest GitHub release off the GTK thread and reports the outcome once.
pub fn check_for_updates(current_version: &'static str) -> Receiver<UpdateCheck> {
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("strata-update-check".into())
        .spawn(move || {
            let _sent = sender.send(fetch_latest_release(current_version));
        });
    drop(spawned);
    receiver
}

/// Fetches the release whose tag exactly matches the installed package version.
pub fn fetch_release_notes(version: &'static str) -> Receiver<ReleaseNotes> {
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("strata-release-notes".into())
        .spawn(move || {
            let _sent = sender.send(fetch_exact_release(version));
        });
    drop(spawned);
    receiver
}

fn fetch_latest_release(current_version: &str) -> UpdateCheck {
    match request_release(&format!("{API_ROOT}/latest")) {
        Ok(release) => {
            let release_metadata = metadata(&release);
            if is_newer(&release_metadata.version, current_version) {
                let archive_name = archive_name(&release_metadata.version);
                let download_url = release
                    .assets
                    .iter()
                    .find(|asset| asset.name == archive_name)
                    .map(|asset| asset.browser_download_url.clone());
                UpdateCheck::Available {
                    release: release_metadata,
                    download_url,
                }
            } else {
                UpdateCheck::UpToDate
            }
        }
        Err(error) => UpdateCheck::Failed(request_error_message(&error)),
    }
}

fn fetch_exact_release(version: &str) -> ReleaseNotes {
    let url = release_page_url(version);
    match request_release(&format!("{API_ROOT}/tags/v{version}")) {
        Ok(release) => ReleaseNotes::Found(metadata(&release)),
        Err(ureq::Error::StatusCode(404)) => ReleaseNotes::Unavailable { url },
        Err(error) => ReleaseNotes::Failed {
            message: request_error_message(&error),
            url,
        },
    }
}

fn request_error_message(error: &ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(403 | 429) => "GitHub API rate limit reached".to_owned(),
        ureq::Error::StatusCode(code) => format!("GitHub API returned HTTP {code}"),
        _ => format!("Network request failed: {error}"),
    }
}

fn release_page_url(version: &str) -> String {
    format!("{RELEASES_URL}/tag/v{version}")
}

fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

fn parse_version(value: &str) -> (u64, u64, u64) {
    let mut parts = value.split('.').map(|part| part.parse().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests;
