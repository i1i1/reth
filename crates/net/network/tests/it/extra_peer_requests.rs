//! Tests for eth related requests

use std::sync::Arc;

use alloy_consensus::{Header, TxEip2930};
use alloy_primitives::{Bytes, PrimitiveSignature as Signature, TxKind, U256};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use rand::Rng;
use reth_eth_wire::{ExtraPeerRequests, HeadersDirection};
use reth_network::{
    eth_requests::{EthRequestHandler, HandleExtraPeerRequest},
    test_utils::{NetworkEventStream, Testnet},
    BlockDownloaderProvider, EthNetworkPrimitives, NetworkEventListenerProvider, NetworkPrimitives,
    PeerRequest,
};
use reth_network_api::{NetworkInfo, PeerId, Peers};
use reth_network_p2p::{
    bodies::client::BodiesClient,
    error::RequestResult,
    headers::client::{HeadersClient, HeadersRequest},
};
use reth_primitives::{Block, Transaction, TransactionSigned};
use reth_provider::{test_utils::MockEthProvider, BlockReader};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Debug, PartialEq, Eq, Clone, RlpEncodable, RlpDecodable, Deserialize, Serialize)]
struct Ping;

#[derive(Debug, PartialEq, Eq, Clone, RlpEncodable, RlpDecodable, Deserialize, Serialize)]
struct Pong;

impl ExtraPeerRequests for Ping {
    type Response = Pong;
    fn req_id(p: &Self) -> u8 {
        0x20
    }
    fn resp_id(p: &Pong) -> u8 {
        0x21
    }
}

impl<C, N> HandleExtraPeerRequest<Ping> for EthRequestHandler<C, N>
where
    N: NetworkPrimitives,
    C: BlockReader,
{
    fn handle_extra_peer_request(
        &self,
        _: PeerId,
        _: Ping,
        response: oneshot::Sender<RequestResult<Pong>>,
    ) {
        tracing::error!("Responding");
        let _ = response.send(Ok(Pong));
        tracing::error!("Responding out");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OurNetworkPrimitives;

impl NetworkPrimitives for OurNetworkPrimitives {
    type BlockHeader = <EthNetworkPrimitives as NetworkPrimitives>::BlockHeader;
    type BlockBody = <EthNetworkPrimitives as NetworkPrimitives>::BlockBody;
    type Block = <EthNetworkPrimitives as NetworkPrimitives>::Block;
    type BroadcastedTransaction =
        <EthNetworkPrimitives as NetworkPrimitives>::BroadcastedTransaction;
    type PooledTransaction = <EthNetworkPrimitives as NetworkPrimitives>::PooledTransaction;
    type Receipt = <EthNetworkPrimitives as NetworkPrimitives>::Receipt;

    // Basically only override extra peer requests with Ping
    type ExtraPeerRequests = Ping;
}

#[tokio::test(flavor = "multi_thread")]
async fn ping() {
    reth_tracing::init_test_tracing();
    let mut rng = rand::thread_rng();
    let mock_provider = Arc::new(MockEthProvider::default());

    let mut net =
        Testnet::<_, _, OurNetworkPrimitives>::create_with(2, mock_provider.clone()).await;

    // install request handlers
    net.for_each_mut(|peer| peer.install_request_handler());

    let handle0 = net.peers()[0].handle();
    let mut events0 = NetworkEventStream::new(handle0.event_listener());

    let handle1 = net.peers()[1].handle();

    let _handle = net.spawn();

    let fetch0 = handle0.fetch_client().await.unwrap();

    handle0.add_peer(*handle1.peer_id(), handle1.local_addr());
    let connected = events0.next_session_established().await.unwrap();
    assert_eq!(connected, *handle1.peer_id());

    tracing::error!("Here");
    let (tx, rx) = tokio::sync::oneshot::channel();
    handle0.send_eth_message(
        *handle1.peer_id(),
        reth_network::message::PeerMessage::EthRequest(PeerRequest::Extra {
            request: Ping,
            response: tx,
        }),
    );
    tracing::error!("Here");
    let Pong = rx.await.unwrap().unwrap();
    tracing::error!("Here");
}
