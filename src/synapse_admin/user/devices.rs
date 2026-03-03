use std::sync::Arc;

use axum::{Json, extract::{Path, State}};
use futures::StreamExt;
use ruma::UserId;
use serde::Serialize;
use tuwunel_service::Services;

use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v2/users/:user_id/devices`
pub(crate) async fn get_devices(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Path(user_id_str): Path<String>,
) -> Result<Json<GetDevicesResponse>, SynapseError> {
	let user_id = UserId::parse(&user_id_str)
		.map_err(|_| SynapseError::not_found("Invalid user ID"))?;

	if !services.users.exists(&user_id).await {
		return Err(SynapseError::not_found("User not found"));
	}

	let devices: Vec<DeviceEntry> = services
		.users
		.all_devices_metadata(&user_id)
		.map(|device| DeviceEntry {
			device_id: device.device_id.to_string(),
			display_name: device.display_name.map(|s| s.to_string()),
			last_seen_ip: device.last_seen_ip.map(|s| s.to_string()),
			last_seen_ts: device.last_seen_ts.map(|ts| u64::from(ts.get())),
			user_id: user_id_str.clone(),
		})
		.collect()
		.await;

	let total = devices.len();

	Ok(Json(GetDevicesResponse { devices, total }))
}

#[derive(Serialize)]
pub(crate) struct GetDevicesResponse {
	pub(crate) devices: Vec<DeviceEntry>,
	pub(crate) total: usize,
}

#[derive(Serialize)]
pub(crate) struct DeviceEntry {
	pub(crate) device_id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) display_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) last_seen_ip: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) last_seen_ts: Option<u64>,
	pub(crate) user_id: String,
}
