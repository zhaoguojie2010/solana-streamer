use crate::{
    common::AnyResult,
    streaming::{
        common::SubscriptionHandle,
        grpc::TransactionPretty,
        yellowstone_grpc::{TransactionFilter, YellowstoneGrpc},
    },
};
use futures::{SinkExt, StreamExt};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, SubscribeRequest, SubscribeRequestPing,
    SubscribeUpdateTransactionInfo,
};

#[derive(Debug)]
pub enum SystemEvent {
    NewTransfer(TransferInfo),
    Error(String),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransferInfo {
    pub slot: u64,
    pub signature: String,
    pub tx: Option<SubscribeUpdateTransactionInfo>,
}
impl YellowstoneGrpc {
    /// Subscribe to raw transactions referencing the system program.
    pub async fn subscribe_system<F>(
        &self,
        mut callback: F,
        account_include: Option<Vec<String>>,
        account_exclude: Option<Vec<String>>,
    ) -> AnyResult<()>
    where
        F: FnMut(SystemEvent) + Send + 'static,
    {
        let mut handle = self.subscription_handle.lock().await;
        anyhow::ensure!(
            handle.as_ref().is_none_or(SubscriptionHandle::is_finished),
            "already subscribed"
        );
        let filters = vec![TransactionFilter {
            account_include: account_include.unwrap_or_default(),
            account_exclude: account_exclude.unwrap_or_default(),
            account_required: vec!["11111111111111111111111111111111".into()],
        }];
        let transactions = self.subscription_manager.get_subscribe_request_filter(filters, None);
        let (mut sink, mut stream, _) = self
            .subscription_manager
            .subscribe_with_request(transactions, None, None, None)
            .await?;
        let (shutdown, mut closed) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut closed => return Ok(()),
                    message = stream.next() => {
                        match message {
                            Some(Ok(message)) => match message.update_oneof {
                                Some(UpdateOneof::Transaction(update)) => {
                                    let tx = TransactionPretty::try_from((update, None))?;
                                    callback(SystemEvent::NewTransfer(TransferInfo { slot: tx.slot, signature: tx.signature.to_string(), tx: Some(tx.grpc_tx) }));
                                }
                                Some(UpdateOneof::Ping(_)) => sink.send(SubscribeRequest { ping: Some(SubscribeRequestPing { id: 1 }), ..SubscribeRequest::default() }).await?,
                                _ => {}
                            },
                            Some(Err(error)) => { callback(SystemEvent::Error(error.to_string())); return Err(error.into()); }
                            None => { callback(SystemEvent::Error("gRPC stream ended".into())); anyhow::bail!("gRPC stream ended"); }
                        }
                    }
                }
            }
        });
        *handle = Some(SubscriptionHandle::new(task, shutdown));
        Ok(())
    }
}
