use std::sync::Arc;

use axum::{Json, extract::{Path, State}};
use ruma::RoomId;
use tuwunel_service::Services;

use super::common::{RoomEntry, build_room_entry};
use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v1/rooms/{room_id}`
pub(crate) async fn get_room_details(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Path(room_id_str): Path<String>,
) -> Result<Json<RoomEntry>, SynapseError> {
	let room_id = RoomId::parse(&room_id_str)
		.map_err(|_| SynapseError::not_found("Invalid room ID"))?;

	if !services.metadata.exists(&room_id).await {
		return Err(SynapseError::not_found("Room not found"));
	}

	let entry = build_room_entry(&services, &room_id, true).await?;
	Ok(Json(entry))
}
