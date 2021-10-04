use crate::{
    crypto::{primitives::sign::PublicKey, KeyCard},
    link::rendezvous::{
        errors::{
            client::{AttemptError, ConnectFailed, ConnectionError},
            AddressUnknown, AlreadyPublished, CardUnknown, ClientError,
            ShardFull, ShardIdInvalid, ShardIncomplete,
        },
        ClientSettings, Request, Response, ShardId,
    },
    net::traits::TcpConnect,
};

use snafu::ResultExt;

use std::net::SocketAddr;
use std::vec::Vec;

pub struct Client {
    server: Box<dyn TcpConnect>,
    pub settings: ClientSettings,
}

impl Client {
    pub fn new<S>(server: S, settings: ClientSettings) -> Self
    where
        S: 'static + TcpConnect,
    {
        Client {
            server: Box::new(server),
            settings,
        }
    }

    pub async fn publish_card(
        &self,
        card: KeyCard,
        shard: Option<ShardId>,
    ) -> Result<(), ClientError> {
        match self.perform(&Request::PublishCard(card, shard)).await {
            Response::AcknowledgeCard => Ok(()),
            Response::AlreadyPublished(shard) => {
                AlreadyPublished { shard }.fail()
            }
            Response::ShardIdInvalid => ShardIdInvalid.fail(),
            Response::ShardFull => ShardFull.fail(),
            response => {
                panic!("unexpected response to `publish_card`: {:?}", response)
            }
        }
    }

    pub async fn advertise_port(&self, root: PublicKey, port: u16) {
        match self.perform(&Request::AdvertisePort(root, port)).await {
            Response::AcknowledgePort => (),
            response => panic!(
                "unexpected response to `advertise_port`: {:?}",
                response
            ),
        }
    }

    pub async fn get_shard(
        &self,
        shard: ShardId,
    ) -> Result<Vec<KeyCard>, ClientError> {
        match self.perform(&Request::GetShard(shard)).await {
            Response::Shard(shard) => Ok(shard),
            Response::ShardIdInvalid => ShardIdInvalid.fail(),
            Response::ShardIncomplete => ShardIncomplete.fail(),
            response => {
                panic!("unexpected response to `get_shard`: {:?}", response)
            }
        }
    }

    pub async fn get_card(
        &self,
        root: PublicKey,
    ) -> Result<KeyCard, ClientError> {
        match self.perform(&Request::GetCard(root)).await {
            Response::Card(card) => Ok(card),
            Response::CardUnknown => CardUnknown.fail(),
            response => {
                panic!("unexpected response to `get_card`: {:?}", response)
            }
        }
    }

    pub async fn get_address(
        &self,
        root: PublicKey,
    ) -> Result<SocketAddr, ClientError> {
        match self.perform(&Request::GetAddress(root)).await {
            Response::Address(address) => Ok(address),
            Response::AddressUnknown => AddressUnknown.fail(),
            response => {
                panic!("unexpected response to `get_address`: {:?}", response)
            }
        }
    }

    async fn perform(&self, request: &Request) -> Response {
        let mut sleep_agent = self.settings.sleep_schedule.agent();

        loop {
            if let Ok(response) = self.attempt(&request).await {
                return response;
            }

            sleep_agent.step().await;
        }
    }

    async fn attempt(
        &self,
        request: &Request,
    ) -> Result<Response, AttemptError> {
        let mut connection =
            self.server.connect().await.context(ConnectFailed)?;

        connection.send(&request).await.context(ConnectionError)?;

        let response: Response =
            connection.receive().await.context(ConnectionError)?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        crypto::KeyChain,
        link::rendezvous::{Server, ServerSettings},
    };

    use std::time::Duration;

    use tokio::time;

    async fn setup_server(
        address: &'static str,
        shard_sizes: Vec<usize>,
    ) -> Server {
        Server::new(address, ServerSettings { shard_sizes })
            .await
            .unwrap()
    }

    async fn setup_clients(
        address: &'static str,
        clients: usize,
    ) -> (Vec<KeyChain>, Vec<KeyCard>, Vec<PublicKey>, Vec<Client>) {
        let keychains =
            (0..clients).map(|_| KeyChain::random()).collect::<Vec<_>>();

        let keycards = keychains
            .iter()
            .map(|keychain| keychain.keycard())
            .collect::<Vec<_>>();

        let roots = keycards
            .iter()
            .map(|keycard| keycard.root())
            .collect::<Vec<_>>();

        let clients = (0..clients)
            .map(|_| Client::new(address, ClientSettings::default()))
            .collect::<Vec<_>>();

        (keychains, keycards, roots, clients)
    }

    async fn setup(
        address: &'static str,
        clients: usize,
        shard_sizes: Vec<usize>,
    ) -> (
        Server,
        Vec<KeyChain>,
        Vec<KeyCard>,
        Vec<PublicKey>,
        Vec<Client>,
    ) {
        let server = setup_server(address, shard_sizes).await;
        let (keychains, keycards, roots, clients) =
            setup_clients(address, clients).await;

        (server, keychains, keycards, roots, clients)
    }

    async fn keycard_fill(
        shard: ShardId,
        keycards: Vec<KeyCard>,
        roots: Vec<PublicKey>,
        clients: Vec<Client>,
    ) {
        for j in 0..clients.len() {
            for c in 0..clients.len() {
                match clients[c].get_shard(0).await.unwrap_err() {
                    ClientError::ShardIncomplete => (),
                    error => panic!(
                        "unexpected error upon querying shard: {}",
                        error
                    ),
                }

                for d in 0..j {
                    assert_eq!(
                        clients[c].get_card(roots[d].clone()).await.unwrap(),
                        keycards[d]
                    );
                }

                for d in j..clients.len() {
                    match clients[c]
                        .get_card(roots[d].clone())
                        .await
                        .unwrap_err()
                    {
                        ClientError::CardUnknown => (),
                        error => panic!(
                            "unexpected error upon querying card: {}",
                            error
                        ),
                    }
                }
            }

            clients[j]
                .publish_card(keycards[j].clone(), Some(shard))
                .await
                .unwrap();
        }

        for c in 0..clients.len() {
            let shard = clients[c].get_shard(shard).await.unwrap();

            assert!(shard.iter().all(|keycard| keycards.contains(keycard)));
            assert!(keycards.iter().all(|keycard| shard.contains(keycard)));

            for d in 0..clients.len() {
                assert_eq!(
                    clients[c].get_card(roots[d].clone()).await.unwrap(),
                    keycards[d]
                );
            }
        }
    }

    #[tokio::test]
    async fn single_shard_keycard_fill() {
        let (_server, _keychains, keycards, roots, clients) =
            setup("127.0.0.1:1234", 3, vec![3]).await;

        keycard_fill(0, keycards, roots, clients).await;
    }

    #[tokio::test]
    #[ignore]
    async fn single_shard_keycard_delayed_fill() {
        const ADDRESS: &str = "127.0.0.1:1235";
        const CLIENTS: usize = 3;
        const SHARD_SIZES: &[usize] = &[3];

        let (_keychains, keycards, roots, clients) =
            setup_clients(ADDRESS, CLIENTS).await;

        let fill = tokio::spawn(async move {
            keycard_fill(0, keycards, roots, clients).await;
        });

        time::sleep(Duration::from_secs(5)).await;

        let _server = setup_server(ADDRESS, SHARD_SIZES.to_vec()).await;

        fill.await.unwrap();
    }

    #[tokio::test]
    async fn multiple_shard_keycard_fill() {
        let (_server, _keychains, keycards, roots, clients) =
            setup("127.0.0.1:1236", 9, vec![3, 3, 3]).await;

        let mut keycards_alpha = keycards;
        let mut keycards_beta = keycards_alpha.split_off(3);
        let keycards_gamma = keycards_beta.split_off(3);

        let mut roots_alpha = roots;
        let mut roots_beta = roots_alpha.split_off(3);
        let roots_gamma = roots_beta.split_off(3);

        let mut clients_alpha = clients;
        let mut clients_beta = clients_alpha.split_off(3);
        let clients_gamma = clients_beta.split_off(3);

        let alpha = tokio::spawn(async move {
            keycard_fill(0, keycards_alpha, roots_alpha, clients_alpha).await;
        });

        let beta = tokio::spawn(async move {
            keycard_fill(1, keycards_beta, roots_beta, clients_beta).await;
        });

        let gamma = tokio::spawn(async move {
            keycard_fill(2, keycards_gamma, roots_gamma, clients_gamma).await;
        });

        alpha.await.unwrap();
        beta.await.unwrap();
        gamma.await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn multiple_shard_keycard_delayed_fill() {
        const ADDRESS: &str = "127.0.0.1:1237";
        const CLIENTS: usize = 9;
        const SHARD_SIZES: &[usize] = &[3, 3, 3];

        let (_keychains, keycards, roots, clients) =
            setup_clients(ADDRESS, CLIENTS).await;

        let mut keycards_alpha = keycards;
        let mut keycards_beta = keycards_alpha.split_off(3);
        let keycards_gamma = keycards_beta.split_off(3);

        let mut roots_alpha = roots;
        let mut roots_beta = roots_alpha.split_off(3);
        let roots_gamma = roots_beta.split_off(3);

        let mut clients_alpha = clients;
        let mut clients_beta = clients_alpha.split_off(3);
        let clients_gamma = clients_beta.split_off(3);

        let alpha = tokio::spawn(async move {
            keycard_fill(0, keycards_alpha, roots_alpha, clients_alpha).await;
        });

        let beta = tokio::spawn(async move {
            keycard_fill(1, keycards_beta, roots_beta, clients_beta).await;
        });

        let gamma = tokio::spawn(async move {
            keycard_fill(2, keycards_gamma, roots_gamma, clients_gamma).await;
        });

        time::sleep(Duration::from_secs(5)).await;

        let _server = setup_server(ADDRESS, SHARD_SIZES.to_vec()).await;

        alpha.await.unwrap();
        beta.await.unwrap();
        gamma.await.unwrap();
    }

    #[tokio::test]
    async fn shard_overflow() {
        let (_server, _keychains, keycards, _roots, clients) =
            setup("127.0.0.1:1238", 4, vec![3]).await;

        for j in 0..3 {
            clients[j]
                .publish_card(keycards[j].clone(), Some(0))
                .await
                .unwrap();
        }

        match clients[3]
            .publish_card(keycards[3].clone(), Some(0))
            .await
            .unwrap_err()
        {
            ClientError::ShardFull => (),
            error => {
                panic!("unexpected error upon overflowing shard: {}", error)
            }
        }
    }

    #[tokio::test]
    async fn invalid_shard() {
        let (_server, _keychains, keycards, _roots, clients) =
            setup("127.0.0.1:1239", 1, vec![3]).await;

        match clients[0]
            .publish_card(keycards[0].clone(), Some(1))
            .await
            .unwrap_err()
        {
            ClientError::ShardIdInvalid => (),
            error => panic!(
                "unexpected error upon publishing to invalid shard: {}",
                error
            ),
        }
    }
}
