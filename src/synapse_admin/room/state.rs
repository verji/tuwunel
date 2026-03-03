use std::sync::Arc;

use axum::{Json, extract::{Path, State}};
use futures::StreamExt;
use ruma::RoomId;
use serde::Serialize;
use tuwunel_core::matrix::event::Event;
use tuwunel_service::Services;

use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v1/rooms/{room_id}/state`
pub(crate) async fn get_room_state(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Path(room_id_str): Path<String>,
) -> Result<Json<StateResponse>, SynapseError> {
	let room_id = RoomId::parse(&room_id_str)
		.map_err(|_| SynapseError::not_found("Invalid room ID"))?;

	if !services.metadata.exists(&room_id).await {
		return Err(SynapseError::not_found("Room not found"));
	}

	let state: Vec<serde_json::Value> = services
		.state_accessor
		.room_state_full(&room_id)
		.filter_map(|result| async move {
			result.ok().map(|(_, event)| event.to_value())
		})
		.collect()
		.await;

	Ok(Json(StateResponse { state }))
}

#[derive(Serialize)]
pub(crate) struct StateResponse {
	pub(crate) state: Vec<serde_json::Value>,
}
