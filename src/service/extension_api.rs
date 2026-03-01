use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use ruma::{OwnedUserId, RoomId, ServerName, UserId, events::StateEventType};
use tuwunel_core::{
	extension::ExtensionApi,
	matrix::pdu::PduEvent,
};

use crate::Services;

pub(crate) struct ExtensionApiImpl {
	services: Arc<Services>,
}

impl ExtensionApiImpl {
	pub(crate) fn new(services: Arc<Services>) -> Self { Self { services } }
}

#[async_trait]
impl ExtensionApi for ExtensionApiImpl {
	async fn room_state_get(
		&self,
		room_id: &RoomId,
		event_type: &StateEventType,
		state_key: &str,
	) -> Option<PduEvent> {
		self.services
			.state_accessor
			.room_state_get(room_id, event_type, state_key)
			.await
			.ok()
	}

	async fn user_is_admin(&self, user_id: &UserId) -> bool {
		self.services.admin.user_is_admin(user_id).await
	}

	async fn get_room_members(&self, room_id: &RoomId) -> Vec<OwnedUserId> {
		self.services
			.state_cache
			.room_members(room_id)
			.map(ToOwned::to_owned)
			.collect()
			.await
	}

	fn server_name(&self) -> &ServerName { self.services.globals.server_name() }

	fn user_is_local(&self, user_id: &UserId) -> bool {
		self.services.globals.user_is_local(user_id)
	}
}
