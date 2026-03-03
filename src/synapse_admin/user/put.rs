use std::sync::Arc;

use axum::{Json, extract::{Path, State}, response::IntoResponse};
use http::StatusCode;
use ruma::{MxcUri, UserId};
use serde::Deserialize;
use tuwunel_service::Services;

use crate::{auth::AdminUser, error::SynapseError};

/// PUT `/_synapse/admin/v2/users/:user_id`
///
/// Creates or modifies a user account. Returns 201 for creation, 200 for
/// modification.
pub(crate) async fn put_account(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Path(user_id_str): Path<String>,
	Json(body): Json<PutAccountRequest>,
) -> Result<impl IntoResponse, SynapseError> {
	let user_id = UserId::parse(&user_id_str)
		.map_err(|_| SynapseError::not_found("Invalid user ID"))?;

	let already_exists = services.users.exists(&user_id).await;

	if !already_exists {
		// Create the user
		services
			.users
			.create(&user_id, body.password.as_deref(), None)
			.await
			.map_err(|e| SynapseError::unknown(format!("Failed to create user: {e}")))?;
	}

	// Update password if provided (and user already existed)
	if already_exists {
		if let Some(ref password) = body.password {
			services
				.users
				.set_password(&user_id, Some(password))
				.await
				.map_err(|e| SynapseError::unknown(format!("Failed to set password: {e}")))?;
		}
	}

	// Update displayname
	if let Some(ref displayname) = body.displayname {
		services
			.users
			.set_displayname(&user_id, Some(displayname));
	}

	// Update avatar_url
	if let Some(ref avatar_url) = body.avatar_url {
		let mxc = <&MxcUri>::from(avatar_url.as_str());
		services
			.users
			.set_avatar_url(&user_id, Some(mxc));
	}

	// Handle deactivation
	if body.deactivated == Some(true)
		&& !services
			.users
			.is_deactivated(&user_id)
			.await
			.unwrap_or(true)
	{
		services
			.users
			.deactivate_account(&user_id)
			.await
			.map_err(|e| SynapseError::unknown(format!("Failed to deactivate: {e}")))?;
	}

	let status = if already_exists {
		StatusCode::OK
	} else {
		StatusCode::CREATED
	};

	Ok((status, Json(serde_json::json!({}))))
}

#[derive(Deserialize)]
pub(crate) struct PutAccountRequest {
	pub(crate) password: Option<String>,
	pub(crate) displayname: Option<String>,
	pub(crate) avatar_url: Option<String>,
	#[allow(dead_code)]
	pub(crate) admin: Option<bool>,
	pub(crate) deactivated: Option<bool>,
	#[allow(dead_code)]
	pub(crate) threepids: Option<Vec<ThreePidInput>>,
}

#[derive(Deserialize)]
pub(crate) struct ThreePidInput {
	#[allow(dead_code)]
	pub(crate) medium: String,
	#[allow(dead_code)]
	pub(crate) address: String,
}
