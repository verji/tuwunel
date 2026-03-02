use axum::extract::State;
use axum_client_ip::InsecureClientIp;
use futures::FutureExt;
use ruma::{
	RoomId,
	api::client::membership::{join_room_by_id, join_room_by_id_or_alias},
};
use tuwunel_core::{
	Result,
	extension::{EventOrigin, HookContext, HOOK_CTX},
	warn,
};

use super::banned_room_check;
use crate::Ruma;

/// # `POST /_matrix/client/r0/rooms/{roomId}/join`
///
/// Tries to join the sender user into a room.
///
/// - If the server knowns about this room: creates the join event and does auth
///   rules locally
/// - If the server does not know about the room: asks other servers over
///   federation
#[tracing::instrument(skip_all, fields(%client), name = "join")]
pub(crate) async fn join_room_by_id_route(
	State(services): State<crate::State>,
	InsecureClientIp(client): InsecureClientIp,
	body: Ruma<join_room_by_id::v3::Request>,
) -> Result<join_room_by_id::v3::Response> {
	let sender_user = body.sender_user();

	let room_id: &RoomId = &body.room_id;

	banned_room_check(&services, sender_user, room_id, None, client).await?;

	let state_lock = services.state.mutex.lock(room_id).await;

	let hook_ctx = HookContext {
		sender: sender_user.to_owned(),
		room_id: room_id.to_owned(),
		origin: EventOrigin::Local {
			device_id: body.sender_device.as_deref().map(ToOwned::to_owned),
			client_ip: Some(client),
			is_appservice: body.appservice_info.is_some(),
		},
	};

	HOOK_CTX
		.scope(hook_ctx, async {
			let mut errors = 0_usize;
			while let Err(e) = services
				.membership
				.join(
					sender_user,
					room_id,
					None,
					body.reason.clone(),
					&[],
					body.appservice_info.is_some(),
					&state_lock,
				)
				.boxed()
				.await
			{
				errors = errors.saturating_add(1);
				if errors >= services.config.max_join_attempts_per_join_request {
					warn!(
						"Several servers failed. Giving up for this request. Try again for \
						 different server selection."
					);
					return Err(e);
				}
			}

			Ok::<(), tuwunel_core::Error>(())
		})
		.await?;

	drop(state_lock);

	Ok(join_room_by_id::v3::Response { room_id: room_id.to_owned() })
}

/// # `POST /_matrix/client/r0/join/{roomIdOrAlias}`
///
/// Tries to join the sender user into a room.
///
/// - If the server knowns about this room: creates the join event and does auth
///   rules locally
/// - If the server does not know about the room: use the server name query
///   param if specified. if not specified, asks other servers over federation
///   via room alias server name and room ID server name
#[tracing::instrument(skip_all, fields(%client), name = "join")]
pub(crate) async fn join_room_by_id_or_alias_route(
	State(services): State<crate::State>,
	InsecureClientIp(client): InsecureClientIp,
	body: Ruma<join_room_by_id_or_alias::v3::Request>,
) -> Result<join_room_by_id_or_alias::v3::Response> {
	let sender_user = body.sender_user();
	let appservice_info = &body.appservice_info;

	let (room_id, servers) = services
		.alias
		.maybe_resolve_with_servers(&body.room_id_or_alias, Some(&body.via))
		.await?;

	banned_room_check(&services, sender_user, &room_id, Some(&body.room_id_or_alias), client)
		.await?;

	let state_lock = services.state.mutex.lock(&room_id).await;

	let hook_ctx = HookContext {
		sender: sender_user.to_owned(),
		room_id: room_id.clone(),
		origin: EventOrigin::Local {
			device_id: body.sender_device.as_deref().map(ToOwned::to_owned),
			client_ip: Some(client),
			is_appservice: appservice_info.is_some(),
		},
	};

	HOOK_CTX
		.scope(hook_ctx, async {
			let mut errors = 0_usize;
			while let Err(e) = services
				.membership
				.join(
					sender_user,
					&room_id,
					Some(&body.room_id_or_alias),
					body.reason.clone(),
					&servers,
					appservice_info.is_some(),
					&state_lock,
				)
				.boxed()
				.await
			{
				errors = errors.saturating_add(1);
				if errors >= services.config.max_join_attempts_per_join_request {
					warn!(
						"Several servers failed. Giving up for this request. Try again for \
						 different server selection."
					);
					return Err(e);
				}
			}

			Ok::<(), tuwunel_core::Error>(())
		})
		.await?;

	drop(state_lock);

	Ok(join_room_by_id_or_alias::v3::Response { room_id: room_id.clone() })
}
