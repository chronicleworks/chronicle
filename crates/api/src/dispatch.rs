use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tracing::{error, instrument, trace};
use uuid::Uuid;

use common::identity::AuthId;
use common::ledger::SubmissionStage;
use common::prov::NamespaceId;
use common::prov::operations::ChronicleOperation;

use crate::ApiError;
use crate::commands::{ApiCommand, ApiResponse, DepthChargeCommand, ImportCommand};

pub type ApiSendWithReply = ((ApiCommand, AuthId), Sender<Result<ApiResponse, ApiError>>);

#[derive(Debug, Clone)]
/// A clonable api handle
pub struct ApiDispatch {
    pub(crate) tx: Sender<ApiSendWithReply>,
    pub notify_commit: tokio::sync::broadcast::Sender<SubmissionStage>,
}

impl ApiDispatch {
    #[instrument]
    pub async fn dispatch(
        &self,
        command: ApiCommand,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?command, "Dispatch command to api");
        self.tx.clone().send(((command, identity), reply_tx)).await?;

        let reply = reply_rx.recv().await;

        if let Some(Err(ref error)) = reply {
            error!(?error, "Api dispatch");
        }

        reply.ok_or(ApiError::ApiShutdownRx {})?
    }

    #[instrument]
    pub async fn handle_import_command(
        &self,
        identity: AuthId,
        operations: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        self.import_operations(identity, operations).await
    }

    #[instrument]
    async fn import_operations(
        &self,
        identity: AuthId,
        operations: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        self.dispatch(ApiCommand::Import(ImportCommand { operations }), identity.clone())
            .await
    }

    #[instrument]
    pub async fn handle_depth_charge(
        &self,
        namespace: &str,
        uuid: &Uuid,
    ) -> Result<ApiResponse, ApiError> {
        self.dispatch_depth_charge(
            AuthId::Chronicle,
            NamespaceId::from_external_id(namespace, *uuid),
        )
            .await
    }

    #[instrument]
    async fn dispatch_depth_charge(
        &self,
        identity: AuthId,
        namespace: NamespaceId,
    ) -> Result<ApiResponse, ApiError> {
        self.dispatch(ApiCommand::DepthCharge(DepthChargeCommand { namespace }), identity.clone())
            .await
    }
}
