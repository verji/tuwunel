use std::sync::Arc;

use axum::{Json, extract::{Query, State}};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tuwunel_service::Services;

use crate::{auth::AdminUser, error::SynapseError};

/// GET `/_synapse/admin/v2/users`
pub(crate) async fn list_accounts(
	State(services): State<Arc<Services>>,
	_admin: AdminUser,
	Query(params): Query<ListParams>,
) -> Result<Json<ListAccountsResponse>, SynapseError> {
	let from: usize = params.from.unwrap_or(0);
	let limit: usize = params.limit.unwrap_or(100);

	// Collect all local users
	let mut users: Vec<UserEntry> = Vec::new();
	let mut stream = std::pin::pin!(services.users.list_local_users());

	while let Some(user_id) = stream.next().await {
		// Apply name filter: substring match on user_id or displayname
		if let Some(name_filter) = params.name.as_deref() {
			let name_lower: String = name_filter.to_lowercase();
			let uid_matches = user_id.as_str().to_lowercase().contains(&name_lower);
			let dn_matches = services
				.users
				.displayname(user_id)
				.await
				.ok()
				.is_some_and(|dn| dn.to_lowercase().contains(&name_lower));

			if !uid_matches && !dn_matches {
				continue;
			}
		}

		let is_deactivated = services
			.users
			.is_deactivated(user_id)
			.await
			.unwrap_or(false);

		// Apply deactivation filter
		if !params.deactivated.unwrap_or(false) && is_deactivated {
			continue;
		}

		let displayname = services.users.displayname(user_id).await.ok();
		let avatar_url = services
			.users
			.avatar_url(user_id)
			.await
			.ok()
			.map(|u| u.to_string());
		let is_admin = services.admin.user_is_admin(user_id).await;

		// Determine last_seen_ts
		let last_seen_ts: Option<u64> = services
			.users
			.all_devices_metadata(user_id)
			.filter_map(|device| async move {
				device.last_seen_ts.map(|ts| u64::from(ts.get()))
			})
			.fold(None, |acc, ts| async move {
				Some(match acc {
					| Some(max) if max > ts => max,
					| _ => ts,
				})
			})
			.await;

		users.push(UserEntry {
			name: user_id.to_string(),
			displayname,
			avatar_url,
			admin: i32::from(is_admin),
			deactivated: i32::from(is_deactivated),
			erased: false,
			shadow_banned: 0,
			is_guest: 0,
			creation_ts: 0,
			user_type: None,
			locked: false,
			last_seen_ts,
		});
	}

	// Sort
	let order_by = params.order_by.as_deref().unwrap_or("name");
	let descending = params.dir.as_deref() == Some("b");

	users.sort_by(|a, b| {
		let cmp = match order_by {
			| "displayname" => a
				.displayname
				.as_deref()
				.unwrap_or("")
				.cmp(b.displayname.as_deref().unwrap_or("")),
			| "admin" => a.admin.cmp(&b.admin),
			| "deactivated" => a.deactivated.cmp(&b.deactivated),
			| "creation_ts" => a.creation_ts.cmp(&b.creation_ts),
			| "last_seen_ts" => a.last_seen_ts.cmp(&b.last_seen_ts),
			| _ => a.name.cmp(&b.name), // "name" or default
		};
		if descending { cmp.reverse() } else { cmp }
	});

	let total = users.len();

	// Pagination
	let next_token = if from.saturating_add(limit) < total {
		Some(from.saturating_add(limit))
	} else {
		None
	};

	let page = users
		.into_iter()
		.skip(from)
		.take(limit)
		.collect();

	Ok(Json(ListAccountsResponse {
		users: page,
		next_token,
		total,
	}))
}

#[derive(Deserialize)]
pub(crate) struct ListParams {
	pub(crate) from: Option<usize>,
	pub(crate) limit: Option<usize>,
	pub(crate) name: Option<String>,
	#[allow(dead_code)]
	pub(crate) guests: Option<bool>,
	pub(crate) deactivated: Option<bool>,
	pub(crate) order_by: Option<String>,
	pub(crate) dir: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ListAccountsResponse {
	pub(crate) users: Vec<UserEntry>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) next_token: Option<usize>,
	pub(crate) total: usize,
}

#[derive(Serialize)]
pub(crate) struct UserEntry {
	pub(crate) name: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) displayname: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) avatar_url: Option<String>,
	pub(crate) admin: i32,
	pub(crate) deactivated: i32,
	pub(crate) erased: bool,
	pub(crate) shadow_banned: i32,
	pub(crate) is_guest: i32,
	pub(crate) creation_ts: u64,
	pub(crate) user_type: Option<String>,
	pub(crate) locked: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) last_seen_ts: Option<u64>,
}
