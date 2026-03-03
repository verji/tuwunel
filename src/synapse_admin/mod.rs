mod auth;
mod error;
mod room;
mod user;

use std::sync::Arc;

use axum::{Router, routing::get};
use tuwunel_service::Services;

/// Build a `Router<()>` with all Synapse Admin API routes.
///
/// State (`Arc<Services>`) is baked in via `.with_state()`, so the returned
/// router can be merged into the main router after its own `.with_state()`
/// call.
pub fn routes(services: Arc<Services>) -> Router<()> {
	Router::new()
		.route(
			"/_synapse/admin/v2/users",
			get(user::list::list_accounts),
		)
		.route(
			"/_synapse/admin/v2/users/{user_id}",
			get(user::get::get_account).put(user::put::put_account),
		)
		.route(
			"/_synapse/admin/v2/users/{user_id}/devices",
			get(user::devices::get_devices),
		)
		// Room endpoints (v1)
		.route(
			"/_synapse/admin/v1/rooms",
			get(room::list::list_rooms),
		)
		.route(
			"/_synapse/admin/v1/rooms/{room_id}",
			get(room::details::get_room_details),
		)
		.route(
			"/_synapse/admin/v1/rooms/{room_id}/members",
			get(room::members::get_room_members),
		)
		.route(
			"/_synapse/admin/v1/rooms/{room_id}/state",
			get(room::state::get_room_state),
		)
		.with_state(services)
}
