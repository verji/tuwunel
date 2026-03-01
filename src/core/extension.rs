use std::{net::IpAddr, sync::Arc};

use async_trait::async_trait;
use ruma::{
	OwnedDeviceId, OwnedRoomId, OwnedServerName, OwnedUserId, RoomId, ServerName, UserId,
	events::StateEventType,
};

use crate::{Result, matrix::pdu::{PduBuilder, PduEvent}};

/// Describes where an event originated from.
#[derive(Clone, Debug)]
pub enum EventOrigin {
	/// Event sent by a local user via a client API.
	Local {
		device_id: Option<OwnedDeviceId>,
		client_ip: Option<IpAddr>,
		is_appservice: bool,
	},

	/// Event received from a remote server via federation.
	Federation { origin_server: OwnedServerName },

	/// Event generated internally (admin commands, migrations, etc.).
	Internal,
}

/// Context passed to extension hooks for each event.
#[derive(Clone, Debug)]
pub struct HookContext {
	pub sender: OwnedUserId,
	pub room_id: OwnedRoomId,
	pub origin: EventOrigin,
}

impl HookContext {
	#[must_use]
	pub fn device_id(&self) -> Option<&OwnedDeviceId> {
		match &self.origin {
			| EventOrigin::Local { device_id, .. } => device_id.as_ref(),
			| _ => None,
		}
	}

	#[must_use]
	pub fn client_ip(&self) -> Option<IpAddr> {
		match &self.origin {
			| EventOrigin::Local { client_ip, .. } => *client_ip,
			| _ => None,
		}
	}

	#[must_use]
	pub fn is_local(&self) -> bool { matches!(self.origin, EventOrigin::Local { .. }) }

	#[must_use]
	pub fn is_federated(&self) -> bool { matches!(self.origin, EventOrigin::Federation { .. }) }
}

/// Decision returned by a before-hook to allow or block an event.
#[derive(Clone, Debug)]
pub enum EventDecision {
	Allow,
	Block(String),
}

/// Trait implemented by extensions to intercept events.
#[async_trait]
pub trait EventInterceptor: Send + Sync {
	fn name(&self) -> &str;

	async fn before_local_event(
		&self,
		ctx: &HookContext,
		builder: &mut PduBuilder,
	) -> Result<EventDecision>;

	async fn after_event(&self, ctx: &HookContext, event: &PduEvent) -> Result;
}

/// Read-only API exposed to extensions for querying server state.
#[async_trait]
pub trait ExtensionApi: Send + Sync {
	async fn room_state_get(
		&self,
		room_id: &RoomId,
		event_type: &StateEventType,
		state_key: &str,
	) -> Option<PduEvent>;

	async fn user_is_admin(&self, user_id: &UserId) -> bool;

	async fn get_room_members(&self, room_id: &RoomId) -> Vec<OwnedUserId>;

	fn server_name(&self) -> &ServerName;

	fn user_is_local(&self, user_id: &UserId) -> bool;
}

/// Registration entry for an extension. Collected at link time via `inventory`.
pub struct ExtensionEntry {
	pub name: &'static str,
	pub create:
		fn(&serde_json::Value, Arc<dyn ExtensionApi>) -> Result<Box<dyn EventInterceptor>>,
}

inventory::collect!(ExtensionEntry);

tokio::task_local! {
	pub static HOOK_CTX: HookContext;
}
