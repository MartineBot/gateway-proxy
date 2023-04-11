#[cfg(feature = "simd-json")]
use halfbrown::hashmap;
use hyper::{Body, Response};
use serde::Serialize;
#[cfg(not(feature = "simd-json"))]
use serde_json::{to_string, Value as OwnedValue};
#[cfg(feature = "simd-json")]
use simd_json::{to_string, OwnedValue};
use twilight_cache_inmemory::{InMemoryCache, InMemoryCacheStats, UpdateCache};
use twilight_model::{
    channel::{message::Sticker, Channel, StageInstance},
    gateway::{
        payload::incoming::{GuildCreate, GuildDelete},
        presence::{Presence, UserOrId},
        OpCode,
    },
    guild::{Emoji, Guild, Member, Role},
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
    voice::VoiceState,
};

use std::{collections::HashMap, sync::Arc};

use crate::{model::JsonObject, state::State};

#[derive(Serialize)]
pub struct Payload {
    pub d: Event,
    pub op: OpCode,
    pub t: String,
    pub s: usize,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum Event {
    Ready(JsonObject),
    GuildCreate(Box<GuildCreate>),
    GuildDelete(GuildDelete),
}

pub struct Guilds(Arc<InMemoryCache>, u32);

impl Guilds {
    pub fn new(cache: Arc<InMemoryCache>, shard_id: u32) -> Self {
        Self(cache, shard_id)
    }

    pub fn cache(&self) -> Arc<InMemoryCache> {
        self.0.clone()
    }

    pub fn update(&self, value: impl UpdateCache) {
        self.0.update(value);
    }

    pub fn stats(&self) -> InMemoryCacheStats {
        self.0.stats()
    }

    pub fn get_ready_payload(&self, mut ready: JsonObject, sequence: &mut usize) -> Payload {
        *sequence += 1;

        let unavailable_guilds = self
            .0
            .iter()
            .guilds()
            .map(|guild| {
                #[cfg(feature = "simd-json")]
                {
                    hashmap! {
                        String::from("id") => guild.id().to_string().into(),
                        String::from("unavailable") => true.into(),
                    }
                    .into()
                }
                #[cfg(not(feature = "simd-json"))]
                {
                    serde_json::json!({
                        "id": guild.id().to_string(),
                        "unavailable": true
                    })
                }
            })
            .collect();

        ready.insert(
            String::from("guilds"),
            OwnedValue::Array(unavailable_guilds),
        );

        Payload {
            d: Event::Ready(ready),
            op: OpCode::Dispatch,
            t: String::from("READY"),
            s: *sequence,
        }
    }

    fn channels_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Channel> {
        self.0
            .guild_channels(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|channel_id| {
                        let channel = self.0.channel(*channel_id)?;

                        if channel.kind.is_thread() {
                            None
                        } else {
                            Some(channel.value().clone())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn presences_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Presence> {
        self.0
            .guild_presences(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|user_id| {
                        let presence = self.0.presence(guild_id, *user_id)?;

                        Some(Presence {
                            activities: presence.activities().to_vec(),
                            client_status: presence.client_status().clone(),
                            guild_id: presence.guild_id(),
                            status: presence.status(),
                            user: UserOrId::UserId {
                                id: presence.user_id(),
                            },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn emojis_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Emoji> {
        self.0
            .guild_emojis(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|emoji_id| {
                        let emoji = self.0.emoji(*emoji_id)?;

                        Some(Emoji {
                            animated: emoji.animated(),
                            available: emoji.available(),
                            id: emoji.id(),
                            managed: emoji.managed(),
                            name: emoji.name().to_string(),
                            require_colons: emoji.require_colons(),
                            roles: emoji.roles().to_vec(),
                            user: emoji
                                .user_id()
                                .and_then(|id| self.0.user(id).map(|user| user.value().clone())),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn member(&self, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Option<Member> {
        let member = self.0.member(guild_id, user_id)?;

        Some(Member {
            avatar: member.avatar(),
            communication_disabled_until: member.communication_disabled_until(),
            deaf: member.deaf().unwrap_or_default(),
            flags: member.flags(),
            joined_at: member.joined_at(),
            mute: member.mute().unwrap_or_default(),
            nick: member.nick().map(ToString::to_string),
            pending: member.pending(),
            premium_since: member.premium_since(),
            roles: member.roles().to_vec(),
            user: self.0.user(member.user_id())?.value().clone(),
        })
    }

    fn members_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Member> {
        self.0
            .guild_members(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|user_id| self.member(guild_id, *user_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn roles_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Role> {
        self.0
            .guild_roles(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|role_id| Some(self.0.role(*role_id)?.value().resource().clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn stage_instances_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<StageInstance> {
        self.0
            .guild_stage_instances(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|stage_id| {
                        Some(self.0.stage_instance(*stage_id)?.value().resource().clone())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn stickers_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Sticker> {
        self.0
            .guild_stickers(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|sticker_id| {
                        let sticker = self.0.sticker(*sticker_id)?;

                        Some(Sticker {
                            available: sticker.available(),
                            description: Some(sticker.description().to_string()),
                            format_type: sticker.format_type(),
                            guild_id: Some(sticker.guild_id()),
                            id: sticker.id(),
                            kind: sticker.kind(),
                            name: sticker.name().to_string(),
                            pack_id: sticker.pack_id(),
                            sort_value: sticker.sort_value(),
                            tags: sticker.tags().to_string(),
                            user: sticker
                                .user_id()
                                .and_then(|id| self.0.user(id).map(|user| user.value().clone())),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn voice_states_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<VoiceState> {
        self.0
            .guild_voice_states(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|user_id| {
                        let voice_state = self.0.voice_state(*user_id, guild_id)?;

                        Some(VoiceState {
                            channel_id: Some(voice_state.channel_id()),
                            deaf: voice_state.deaf(),
                            guild_id: Some(voice_state.guild_id()),
                            member: self.member(guild_id, *user_id),
                            mute: voice_state.mute(),
                            self_deaf: voice_state.self_deaf(),
                            self_mute: voice_state.self_mute(),
                            self_stream: voice_state.self_stream(),
                            self_video: voice_state.self_video(),
                            session_id: voice_state.session_id().to_string(),
                            suppress: voice_state.suppress(),
                            user_id: voice_state.user_id(),
                            request_to_speak_timestamp: voice_state.request_to_speak_timestamp(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn threads_in_guild(&self, guild_id: Id<GuildMarker>) -> Vec<Channel> {
        self.0
            .guild_channels(guild_id)
            .map(|reference| {
                reference
                    .iter()
                    .filter_map(|channel_id| {
                        let channel = self.0.channel(*channel_id)?;

                        if channel.kind.is_thread() {
                            Some(channel.value().clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_guild_payloads<'a>(
        &'a self,
        sequence: &'a mut usize,
    ) -> impl Iterator<Item = Payload> + 'a {
        self.0.iter().guilds().map(move |guild| {
            *sequence += 1;

            if guild.unavailable() {
                let guild_delete = GuildDelete {
                    id: guild.id(),
                    unavailable: true,
                };

                Payload {
                    d: Event::GuildDelete(guild_delete),
                    op: OpCode::Dispatch,
                    t: String::from("GUILD_DELETE"),
                    s: *sequence,
                }
            } else {
                let guild_channels = self.channels_in_guild(guild.id());
                let presences = self.presences_in_guild(guild.id());
                let emojis = self.emojis_in_guild(guild.id());
                let members = self.members_in_guild(guild.id());
                let roles = self.roles_in_guild(guild.id());
                let stage_instances = self.stage_instances_in_guild(guild.id());
                let stickers = self.stickers_in_guild(guild.id());
                let voice_states = self.voice_states_in_guild(guild.id());
                let threads = self.threads_in_guild(guild.id());

                let new_guild = Guild {
                    afk_channel_id: guild.afk_channel_id(),
                    afk_timeout: guild.afk_timeout(),
                    application_id: guild.application_id(),
                    approximate_member_count: None, // Only present in with_counts HTTP endpoint
                    banner: guild.banner().map(ToOwned::to_owned),
                    approximate_presence_count: None, // Only present in with_counts HTTP endpoint
                    channels: guild_channels,
                    default_message_notifications: guild.default_message_notifications(),
                    description: guild.description().map(ToString::to_string),
                    discovery_splash: guild.discovery_splash().map(ToOwned::to_owned),
                    emojis,
                    explicit_content_filter: guild.explicit_content_filter(),
                    features: guild.features().cloned().collect(),
                    icon: guild.icon().map(ToOwned::to_owned),
                    id: guild.id(),
                    joined_at: guild.joined_at(),
                    large: guild.large(),
                    max_members: guild.max_members(),
                    max_presences: guild.max_presences(),
                    max_video_channel_users: None, // Not in the cache model
                    member_count: guild.member_count(),
                    members,
                    mfa_level: guild.mfa_level(),
                    name: guild.name().to_string(),
                    nsfw_level: guild.nsfw_level(),
                    owner_id: guild.owner_id(),
                    owner: guild.owner(),
                    permissions: guild.permissions(),
                    public_updates_channel_id: guild.public_updates_channel_id(),
                    preferred_locale: guild.preferred_locale().to_string(),
                    premium_progress_bar_enabled: guild.premium_progress_bar_enabled(),
                    premium_subscription_count: guild.premium_subscription_count(),
                    premium_tier: guild.premium_tier(),
                    presences,
                    roles,
                    rules_channel_id: guild.rules_channel_id(),
                    splash: guild.splash().map(ToOwned::to_owned),
                    stage_instances,
                    stickers,
                    system_channel_flags: guild.system_channel_flags(),
                    system_channel_id: guild.system_channel_id(),
                    threads,
                    unavailable: false,
                    vanity_url_code: guild.vanity_url_code().map(ToString::to_string),
                    verification_level: guild.verification_level(),
                    voice_states,
                    widget_channel_id: guild.widget_channel_id(),
                    widget_enabled: guild.widget_enabled(),
                };

                let guild_create = GuildCreate(new_guild);

                Payload {
                    d: Event::GuildCreate(Box::new(guild_create)),
                    op: OpCode::Dispatch,
                    t: String::from("GUILD_CREATE"),
                    s: *sequence,
                }
            }
        })
    }
}

pub fn not_found_body(type_name: &str) -> Body {
    let body = to_string(&HashMap::from([(
        "message",
        format!("Unknown {type_name}"),
    )]))
    .unwrap();
    Body::from(body)
}

fn serialize_fail_body(type_name: &str) -> Body {
    let body = to_string(&HashMap::from([(
        "message",
        format!("Failed to serialize {type_name}"),
    )]))
    .unwrap();
    Body::from(body)
}

fn bad_request_body() -> Body {
    let body = to_string(&HashMap::from([("message", "Bad Request")])).unwrap();
    Body::from(body)
}

pub fn handle_cache_guild(value: &str, state: State) -> Response<Body> {
    let response = Response::builder().header("Content-Type", "application/json");
    let id = match value.parse::<u64>() {
        Ok(id) => id,
        Err(_) => return response.status(400).body(bad_request_body()).unwrap(),
    };
    if id == 0 {
        return response.status(400).body(bad_request_body()).unwrap();
    }

    let guild_id = Id::<GuildMarker>::new(id);
    let mut guild = None;
    for shard in &state.shards {
        if !shard.guilds.cache().guild(guild_id).is_none() {
            guild = Some(shard.guilds.cache().guild(guild_id).unwrap().clone());
        }
    }

    if guild.is_none() {
        return response.status(404).body(not_found_body("guild")).unwrap();
    }

    if let Ok(serialized) = to_string(&guild.unwrap()) {
        return response.body(Body::from(serialized)).unwrap();
    }

    return response
        .status(503)
        .body(serialize_fail_body("guild"))
        .unwrap();
}

pub fn handle_cache_channel(value: &str, state: State) -> Response<Body> {
    let response = Response::builder().header("Content-Type", "application/json");
    let id = match value.parse::<u64>() {
        Ok(id) => id,
        Err(_) => return response.status(400).body(bad_request_body()).unwrap(),
    };
    if id == 0 {
        return response.status(400).body(bad_request_body()).unwrap();
    }

    let channel_id = Id::<ChannelMarker>::new(id);
    let mut channel = None;
    for shard in &state.shards {
        if !shard.guilds.cache().channel(channel_id).is_none() {
            channel = Some(shard.guilds.cache().channel(channel_id).unwrap().clone());
        }
    }

    if channel.is_none() {
        return response
            .status(404)
            .body(not_found_body("channel"))
            .unwrap();
    }

    if let Ok(serialized) = to_string(&channel.unwrap()) {
        return response.body(Body::from(serialized)).unwrap();
    }

    return response
        .status(503)
        .body(serialize_fail_body("channel"))
        .unwrap();
}

pub fn handle_cache_user(value: &str, state: State) -> Response<Body> {
    let response = Response::builder().header("Content-Type", "application/json");
    let id = match value.parse::<u64>() {
        Ok(id) => id,
        Err(_) => return response.status(400).body(bad_request_body()).unwrap(),
    };
    if id == 0 {
        return response.status(400).body(bad_request_body()).unwrap();
    }

    let user_id = Id::<UserMarker>::new(id);
    let mut user = None;
    for shard in &state.shards {
        if !shard.guilds.cache().user(user_id).is_none() {
            user = Some(shard.guilds.cache().user(user_id).unwrap().clone());
        }
    }

    if user.is_none() {
        return response.status(404).body(not_found_body("user")).unwrap();
    }

    if let Ok(serialized) = to_string(&user.unwrap()) {
        return response.body(Body::from(serialized)).unwrap();
    }

    return response
        .status(503)
        .body(serialize_fail_body("user"))
        .unwrap();
}
