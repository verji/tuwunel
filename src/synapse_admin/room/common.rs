use futures::StreamExt;
use ruma::{
	RoomId,
	events::{
		StateEventType,
		room::history_visibility::RoomHistoryVisibilityEventContent,
	},
};
use serde::Serialize;
use tuwunel_core::matrix::event::Event;
use tuwunel_service::Services;

use crate::error::SynapseError;

/// Room metadata returned by both ListRooms and GetRoomDetails.
///
/// Detail-only fields are `None` when built with `include_details = false`
/// and skipped during serialisation.
#[derive(Serialize)]
pub(crate) struct RoomEntry {
	pub(crate) room_id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) canonical_alias: Option<String>,
	pub(crate) joined_members: u64,
	pub(crate) joined_local_members: usize,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) version: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) creator: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) encryption: Option<String>,
	pub(crate) federatable: bool,
	pub(crate) public: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) join_rules: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) guest_access: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) history_visibility: Option<String>,
	pub(crate) state_events: usize,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) room_type: Option<String>,

	// Detail-only fields
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) topic: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) avatar: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) joined_local_devices: Option<usize>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) forgotten: Option<bool>,
}

/// Build a `RoomEntry` for the given room.
///
/// When `include_details` is `true` the detail-only fields (topic, avatar,
/// joined_local_devices, forgotten) are populated; otherwise they are `None`
/// and omitted from the JSON output.
pub(crate) async fn build_room_entry(
	services: &Services,
	room_id: &RoomId,
	include_details: bool,
) -> Result<RoomEntry, SynapseError> {
	let name = services
		.state_accessor
		.get_name(room_id)
		.await
		.ok();

	let canonical_alias = services
		.state_accessor
		.get_canonical_alias(room_id)
		.await
		.ok()
		.map(|a| a.to_string());

	let joined_members = services
		.state_cache
		.room_joined_count(room_id)
		.await
		.unwrap_or(0);

	let joined_local_members = services
		.state_cache
		.local_users_in_room(room_id)
		.count()
		.await;

	let version = services
		.state
		.get_room_version(room_id)
		.await
		.ok()
		.map(|v| v.to_string());

	let creator = services
		.state_accessor
		.room_state_get(room_id, &StateEventType::RoomCreate, "")
		.await
		.ok()
		.map(|pdu| pdu.sender().to_string());

	let encryption = services
		.state_accessor
		.get_room_encryption(room_id)
		.await
		.ok()
		.map(|a| a.to_string());

	let federatable = services
		.state_accessor
		.get_create(room_id)
		.await
		.ok()
		.and_then(|c| c.federate().ok())
		.unwrap_or(true);

	let public = services.metadata.is_public(room_id).await;

	let join_rules = Some(format!(
		"{:?}",
		services
			.state_accessor
			.get_join_rules(room_id)
			.await
	));

	let guest_access = if services
		.state_accessor
		.guest_can_join(room_id)
		.await
	{
		Some("can_join".to_owned())
	} else {
		Some("forbidden".to_owned())
	};

	let history_visibility: Option<String> = services
		.state_accessor
		.room_state_get_content::<RoomHistoryVisibilityEventContent>(
			room_id,
			&StateEventType::RoomHistoryVisibility,
			"",
		)
		.await
		.ok()
		.map(|c| format!("{:?}", c.history_visibility));

	let state_events = services
		.state_accessor
		.room_state_full_pdus(room_id)
		.count()
		.await;

	let room_type = services
		.state_accessor
		.get_room_type(room_id)
		.await
		.ok()
		.map(|t| t.to_string());

	// Detail-only fields
	let (topic, avatar, joined_local_devices, forgotten) = if include_details {
		let topic = services
			.state_accessor
			.get_room_topic(room_id)
			.await
			.ok();

		let avatar = services
			.state_accessor
			.get_avatar(room_id)
			.await
			.ok()
			.and_then(|c| c.url.map(|u| u.to_string()));

		let device_count: usize = {
			let mut count = 0usize;
			let mut members =
				std::pin::pin!(services.state_cache.local_users_in_room(room_id));
			while let Some(user_id) = members.next().await {
				count += services.users.all_device_ids(user_id).count().await;
			}
			count
		};

		(topic, avatar, Some(device_count), Some(false))
	} else {
		(None, None, None, None)
	};

	Ok(RoomEntry {
		room_id: room_id.to_string(),
		name,
		canonical_alias,
		joined_members,
		joined_local_members,
		version,
		creator,
		encryption,
		federatable,
		public,
		join_rules,
		guest_access,
		history_visibility,
		state_events,
		room_type,
		topic,
		avatar,
		joined_local_devices,
		forgotten,
	})
}
