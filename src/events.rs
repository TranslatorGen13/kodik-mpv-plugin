use anyhow::{Context as _, Result};
use lazy_regex::{Lazy, Regex, regex};
use mpv_client::{Handle, Node};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::config::Quality;
use crate::mpv_ext::{MpvExt, MpvResultExt};
use crate::shiki::ShikiMetaData;
use crate::state::PluginState;
use crate::{kodik, shiki};

const ON_LOAD_REPLY: u64 = 0;
const ON_PRELOADED_REPLY: u64 = 1;
const OBSERVE_AID_REPLY: u64 = 2;
const OBSERVE_YTDL_FORMAT_REPLY: u64 = 3;
const ON_LOAD_PRIORITY: i32 = 50;
const ON_PRELOADED_PRIORITY: i32 = 50;
const KODIK_PAYLOAD_KEY: &str = "kodik-payload";
const KODIK_HOST_NAME: &str = "kodikplayer";
const SHIKI_HOST_NAME: &str = "shikimori";
const MAL_HOST_NAME: &str = "myanimelist";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MetaData {
    Shiki(ShikiMetaData),
    Mal,
    Imdb,
    Kinopoisk,
    Mdl,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Payload {
    metadata_key: String,
    episode: usize,
}

impl Payload {
    pub const fn new(metadata_key: String, episode: usize) -> Self {
        Self { metadata_key, episode }
    }

    pub fn encode(&self) -> Result<String> {
        let json = serde_json::to_string(self).context("failed to serialize kodik payload")?;

        let script_opt = format!("{KODIK_PAYLOAD_KEY}={json}");
        let quoted = format!("%{}%{}", script_opt.len(), script_opt);

        Ok(format!("script-opts-append={quoted}"))
    }

    pub fn decode(raw_json: &str) -> Result<Self> {
        serde_json::from_str(raw_json).context("failed to deserialize kodik payload")
    }

    pub fn metadata_key(&self) -> &str {
        &self.metadata_key
    }

    pub const fn episode(&self) -> usize {
        self.episode
    }
}

pub fn register(mp: &Handle) -> Result<()> {
    mp.hook_add_ext(ON_LOAD_REPLY, "on_load", ON_LOAD_PRIORITY)?;
    mp.hook_add_ext(ON_PRELOADED_REPLY, "on_preloaded", ON_PRELOADED_PRIORITY)?;
    mp.observe_property_ext::<i64>(OBSERVE_AID_REPLY, "current-tracks/audio/id")?;
    mp.observe_property_ext::<String>(OBSERVE_YTDL_FORMAT_REPLY, "ytdl-format")?;

    Ok(())
}

pub fn handle_event(state: &mut PluginState, mp: &Handle, reply: u64) -> Result<()> {
    match reply {
        ON_LOAD_REPLY => on_load(state, mp),
        ON_PRELOADED_REPLY => on_preloaded(state, mp),
        OBSERVE_AID_REPLY => observe_aid_reply(state, mp),
        OBSERVE_YTDL_FORMAT_REPLY => observe_ytdl_format_reply(state, mp),
        _ => Ok(()),
    }
}

fn on_load(state: &mut PluginState, mp: &Handle) -> Result<()> {
    let filename = mp.get_stream_open_filename()?;

    let url = match Url::parse(&filename) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url,
        _ => {
            let mut script_opts = mp.get_script_opts()?;

            let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
                return Ok(());
            };

            let Node::String(payload_encoded) = node else {
                anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
            };

            let payload = Payload::decode(&payload_encoded)?;

            match payload.metadata_key.split_once('.').context("expected host")?.0 {
                SHIKI_HOST_NAME => shiki::on_load(state, mp, &payload),
                MAL_HOST_NAME => todo!(),
                "imdb" => todo!(),
                "kinopoisk" => todo!(),
                "mydramalist" => todo!(),
                _ => Ok(()),
            }?;

            return Ok(());
        }
    };

    let Some(host) = url.host_str() else {
        return Ok(());
    };

    let Some(host_name) = host.rsplit_once('.').map(|(lp, _rp)| lp) else {
        return Ok(());
    };

    match host_name {
        KODIK_HOST_NAME => kodik::on_load(state, mp, &filename),
        SHIKI_HOST_NAME => shiki::expand(state, mp, url.as_str(), host),
        MAL_HOST_NAME => todo!(),
        "kinopoisk" => todo!(),
        "imdb" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

pub fn mark_as_watched(state: &mut PluginState, mp: &Handle) -> Result<()> {
    let mut script_opts = mp.get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        anyhow::bail!("missing `{KODIK_PAYLOAD_KEY}` in `script-opts`")
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;

    match payload.metadata_key.split_once('.').context("expected host")?.0 {
        SHIKI_HOST_NAME => shiki::mark_as_watched(state, mp, &payload),
        MAL_HOST_NAME => todo!(),
        "imdb" => todo!(),
        "kinopoisk" => todo!(),
        "mydramalist" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

fn on_preloaded(state: &PluginState, mp: &Handle) -> Result<()> {
    const AUDIO_TRACK_PLACEHOLDER: &str =
        "ffmpeg://data:audio/wav;base64,UklGRigAAABXQVZFZm10IBAAAAABAAEAIlYAAIhYAQACABAAZGF0YQQAAAAAAAAD";

    let mut script_opts = mp.get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;
    let (metadata_key, episode) = (payload.metadata_key(), payload.episode());

    let kodik_videos = state
        .kodik_videos()
        .get(metadata_key)
        .context("kodik videos should exist after on-load hook")?;

    for result in &kodik_videos.results {
        let mut episodes_accum = 0;
        let title = &result.translation.title;

        let found_season = if let Some(seasons) = &result.seasons {
            let mut found = None;
            for (_, season) in seasons.iter().filter(|(number, _)| **number > 0) {
                let last_episode = season.episodes.last_key_value().context("season must have episodes")?.0;
                if (episodes_accum + last_episode) < episode {
                    episodes_accum += last_episode;
                } else {
                    found = Some(season);
                    break;
                }
            }
            found
        } else {
            None
        };

        if let Some(season) = found_season {
            if !season.episodes.contains_key(&(episode - episodes_accum)) {
                continue;
            }
        } else if episode > 1 {
            continue;
        }

        mp.audio_add(AUDIO_TRACK_PLACEHOLDER, "auto", title)?;
    }

    Ok(())
}

fn observe_aid_reply(state: &mut PluginState, mp: &Handle) -> Result<()> {
    let Some(_) = mp.get_script_opts()?.get(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    // Get current aid. If unavailable or invalid, do nothing.
    let aid = match mp.get_property::<i64>("aid") {
        Ok(id) if id > 0 => id,
        _ => return Ok(()),
    };

    // Resolve title from track-list instead of current-tracks/audio/title.
    // External placeholder tracks often report title as unavailable via the
    // latter property, which previously caused a silent early return.
    let track_list = mp
        .get_property::<Node>("track-list")
        .mpv_context("failed to get track-list")?;

    let Node::Array(tracks) = track_list else {
        anyhow::bail!("track-list is not an array");
    };

    let mut title: Option<String> = None;

    for track in tracks {
        let Node::Map(map) = track else { continue };

        let id = map.get("id").and_then(|n| match n {
            Node::Int(v) => Some(*v),
            _ => None,
        });

        let is_audio = map
            .get("type")
            .and_then(|n| match n {
                Node::String(s) => Some(s.as_str() == "audio"),
                _ => None,
            })
            .unwrap_or(false);

        if id == Some(aid) && is_audio {
            if let Some(Node::String(t)) = map.get("title") {
                if !t.is_empty() {
                    title = Some(t.clone());
                }
            }
            break;
        }
    }

    let Some(current_translation_title) = title else {
        // Title still unavailable — exit quietly without breaking playback.
        return Ok(());
    };

    state
        .config_mut()
        .set_translation_title(Some(regex::escape(&current_translation_title)));

    // Preserve playback position via file-local-options/start, then reload.
    // Note: playlist-play-index accepts only one argument in current MPV
    // (the old "current yes" form is invalid and caused arg parse errors).
    let time_pos = mp.get_time_pos().unwrap_or(0.0);
    let _ = mp.set_file_local_options_start(time_pos);

    // Small delay to let MPV finish track switching.
    std::thread::sleep(std::time::Duration::from_millis(50));

    mp.command(["playlist-play-index", "current"])
        .mpv_context("failed to `playlist-play-index current`")?;

    Ok(())
}

fn observe_ytdl_format_reply(state: &mut PluginState, mp: &Handle) -> Result<()> {
    const EXTRACT_HEIGHT_PATTERN: &Lazy<Regex> = lazy_regex::regex!(r"height<=\??(\d+)");

    let quality = EXTRACT_HEIGHT_PATTERN
        .captures(&mp.get_ytdl_format()?)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .map_or_else(
            || state.config().quality(),
            |height| match height {
                h if h >= 720 => Quality::P720,
                480 => Quality::P480,
                h if h <= 360 => Quality::P360,
                _ => state.config().quality(),
            },
        );

    state.config_mut().set_quality(quality);

    Ok(())
}
