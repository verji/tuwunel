use std::sync::Arc;

use axum::{Json, extract::{Query, State}};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tuwunel_service::Services;

use super::common::{RoomEntry, build_room_entry};
use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v1/rooms`
pub(crate) async fn list_rooms(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Query(params): Query<ListRoomsParams>,
) -> Result<Json<ListRoomsResponse>, SynapseError> {
	let from: usize = params.from.unwrap_or(0);
	let limit: usize = params.limit.unwrap_or(100);

	// Collect all rooms
	let mut rooms: Vec<RoomEntry> = Vec::new();
	let mut stream = std::pin::pin!(services.metadata.iter_ids());

	while let Some(room_id) = stream.next().await {
		let entry = build_room_entry(&services, room_id, false).await?;

		// Apply search_term filter: substring match on name or canonical_alias
		if let Some(ref search) = params.search_term {
			let search_lower = search.to_lowercase();
			let name_matches = entry
				.name
				.as_deref()
				.is_some_and(|n| n.to_lowercase().contains(&search_lower));
			let alias_matches = entry
				.canonical_alias
				.as_deref()
				.is_some_and(|a| a.to_lowercase().contains(&search_lower));

			if !name_matches && !alias_matches {
				continue;
			}
		}

		rooms.push(entry);
	}

	// Sort
	let order_by = params.order_by.as_deref().unwrap_or("name");
	let descending = params.dir.as_deref() == Some("b");

	rooms.sort_by(|a, b| {
		let cmp = match order_by {
			| "canonical_alias" => a
				.canonical_alias
				.as_deref()
				.unwrap_or("")
				.cmp(b.canonical_alias.as_deref().unwrap_or("")),
			| "joined_members" => a.joined_members.cmp(&b.joined_members),
			| "joined_local_members" => {
				a.joined_local_members.cmp(&b.joined_local_members)
			},
			| "version" => a
				.version
				.as_deref()
				.unwrap_or("")
				.cmp(b.version.as_deref().unwrap_or("")),
			| "creator" => a
				.creator
				.as_deref()
				.unwrap_or("")
				.cmp(b.creator.as_deref().unwrap_or("")),
			| "encryption" => a
				.encryption
				.as_deref()
				.unwrap_or("")
				.cmp(b.encryption.as_deref().unwrap_or("")),
			| "federatable" => a.federatable.cmp(&b.federatable),
			| "public" => a.public.cmp(&b.public),
			| "join_rules" => a
				.join_rules
				.as_deref()
				.unwrap_or("")
				.cmp(b.join_rules.as_deref().unwrap_or("")),
			| "guest_access" => a
				.guest_access
				.as_deref()
				.unwrap_or("")
				.cmp(b.guest_access.as_deref().unwrap_or("")),
			| "history_visibility" => a
				.history_visibility
				.as_deref()
				.unwrap_or("")
				.cmp(b.history_visibility.as_deref().unwrap_or("")),
			| "state_events" => a.state_events.cmp(&b.state_events),
			| "room_type" => a
				.room_type
				.as_deref()
				.unwrap_or("")
				.cmp(b.room_type.as_deref().unwrap_or("")),
			| _ => a
				.name
				.as_deref()
				.unwrap_or("")
				.cmp(b.name.as_deref().unwrap_or("")), // "name" or default
		};
		if descending { cmp.reverse() } else { cmp }
	});

	let total_rooms = rooms.len();

	// Pagination
	let next_batch = if from.saturating_add(limit) < total_rooms {
		Some(from.saturating_add(limit))
	} else {
		None
	};

	let page: Vec<RoomEntry> = rooms
		.into_iter()
		.skip(from)
		.take(limit)
		.collect();

	Ok(Json(ListRoomsResponse {
		rooms: page,
		offset: from,
		total_rooms,
		next_batch,
	}))
}

#[derive(Deserialize)]
pub(crate) struct ListRoomsParams {
	pub(crate) from: Option<usize>,
	pub(crate) limit: Option<usize>,
	pub(crate) order_by: Option<String>,
	pub(crate) dir: Option<String>,
	pub(crate) search_term: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ListRoomsResponse {
	pub(crate) rooms: Vec<RoomEntry>,
	pub(crate) offset: usize,
	pub(crate) total_rooms: usize,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) next_batch: Option<usize>,
}
