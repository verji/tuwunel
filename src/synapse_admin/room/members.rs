use std::sync::Arc;

use axum::{Json, extract::{Path, State}};
use futures::StreamExt;
use ruma::RoomId;
use serde::Serialize;
use tuwunel_service::Services;

use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v1/rooms/{room_id}/members`
pub(crate) async fn get_room_members(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Path(room_id_str): Path<String>,
) -> Result<Json<MembersResponse>, SynapseError> {
	let room_id = RoomId::parse(&room_id_str)
		.map_err(|_| SynapseError::not_found("Invalid room ID"))?;

	if !services.metadata.exists(&room_id).await {
		return Err(SynapseError::not_found("Room not found"));
	}

	let members: Vec<String> = services
		.state_cache
		.room_members(&room_id)
		.map(|user_id| user_id.to_string())
		.collect()
		.await;

	let total = members.len();

	Ok(Json(MembersResponse { members, total }))
}

#[derive(Serialize)]
pub(crate) struct MembersResponse {
	pub(crate) members: Vec<String>,
	pub(crate) total: usize,
}
