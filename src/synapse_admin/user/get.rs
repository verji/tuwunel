use std::sync::Arc;

use axum::{Json, extract::{Path, State}};
use futures::StreamExt;
use ruma::UserId;
use serde::Serialize;
use tuwunel_service::Services;

use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v2/users/:user_id`
pub(crate) async fn get_account(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Path(user_id_str): Path<String>,
) -> Result<Json<GetAccountResponse>, SynapseError> {
	let user_id = UserId::parse(&user_id_str)
		.map_err(|_| SynapseError::not_found("Invalid user ID"))?;

	if !services.users.exists(&user_id).await {
		return Err(SynapseError::not_found("User not found"));
	}

	let displayname = services
		.users
		.displayname(&user_id)
		.await
		.ok();

	let avatar_url = services
		.users
		.avatar_url(&user_id)
		.await
		.ok()
		.map(|u| u.to_string());

	let is_deactivated = services
		.users
		.is_deactivated(&user_id)
		.await
		.unwrap_or(false);

	let is_admin = services.admin.user_is_admin(&user_id).await;

	// Determine last_seen_ts from device metadata
	let last_seen_ts: Option<u64> = services
		.users
		.all_devices_metadata(&user_id)
		.filter_map(|device| async move {
			device
				.last_seen_ts
				.map(|ts| u64::from(ts.get()))
		})
		.fold(None, |acc, ts| async move {
			Some(match acc {
				| Some(max) if max > ts => max,
				| _ => ts,
			})
		})
		.await;

	// Check if the user belongs to an appservice
	let appservice_id = {
		let registrations = services.appservice.read().await;
		registrations
			.iter()
			.find(|(_, info)| info.is_user_match(&user_id))
			.map(|(id, _)| id.clone())
	};

	Ok(Json(GetAccountResponse {
		name: user_id.to_string(),
		displayname,
		avatar_url,
		admin: i32::from(is_admin),
		deactivated: i32::from(is_deactivated),
		erased: false,
		shadow_banned: 0,
		locked: false,
		appservice_id,
		user_type: None,
		// Known data gaps — not stored in tuwunel
		creation_ts: 0,
		consent_server_notice_sent: None,
		consent_version: None,
		consent_ts: None,
		external_ids: Vec::new(),
		threepids: Vec::new(),
		is_guest: 0,
		last_seen_ts,
	}))
}

#[derive(Serialize)]
pub(crate) struct GetAccountResponse {
	pub(crate) name: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) displayname: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) avatar_url: Option<String>,
	pub(crate) admin: i32,
	pub(crate) deactivated: i32,
	pub(crate) erased: bool,
	pub(crate) shadow_banned: i32,
	pub(crate) locked: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) appservice_id: Option<String>,
	pub(crate) user_type: Option<String>,
	/// Not persisted in tuwunel — always 0
	pub(crate) creation_ts: u64,
	pub(crate) consent_server_notice_sent: Option<String>,
	pub(crate) consent_version: Option<String>,
	pub(crate) consent_ts: Option<u64>,
	pub(crate) external_ids: Vec<ExternalId>,
	pub(crate) threepids: Vec<ThreePid>,
	/// Not persisted in tuwunel — always 0
	pub(crate) is_guest: i32,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) last_seen_ts: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct ExternalId {
	pub(crate) auth_provider: String,
	pub(crate) external_id: String,
}

#[derive(Serialize)]
pub(crate) struct ThreePid {
	pub(crate) medium: String,
	pub(crate) address: String,
	pub(crate) added_at: u64,
	pub(crate) validated_at: u64,
}
