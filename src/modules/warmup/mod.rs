use alloy::{
    network::Ethereum,
    providers::{Provider, ProviderBuilder, RootProvider},
};
use alloy_chains::NamedChain;
use database::{
    entity::impls::{
        account::{AccountAction, AccountConditions},
        prelude::*,
    },
    repositories::RepoImpls,
    use_cases::accounts,
};

use common::{
    config::Config,
    onchain::client::{Client as EvmClient, StrictNonceManager},
    utils::random::random_in_range,
};
use error::WarmupError;
use lending::deposit;
use std::{str::FromStr, sync::Arc, time::Duration};
use swap::swap;
use tokio::task::JoinSet;
use url::Url;

use crate::Result;

pub mod error;
mod lending;
mod swap;

pub async fn run_warmup(repo: Arc<RepoImpls>, config: Arc<Config>) -> Result<()> {
    let spawn_task = |handles: &mut JoinSet<_>,
                      provider: RootProvider,
                      account: AccountModel,
                      repo: Arc<_>,
                      config: Arc<_>,
                      delay: u64| {
        handles.spawn(async move {
            let id = account.id;
            tokio::time::sleep(Duration::from_secs(delay)).await;
            let res = process_account(provider, repo, config, account).await;

            (id, res)
        })
    };

    // The inner state of the root provider.
    // pub(crate) inner: Arc<RootProviderInner<N>>,
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Ethereum>()
        .with_chain(NamedChain::MonadTestnet)
        .on_http(Url::from_str(&config.rpc_url)?);

    let accounts = accounts::search(
        repo.clone(),
        AccountConditions { goal_reached: Some(false), ..Default::default() },
    )
    .await?;

    let mut handles = JoinSet::new();

    for (i, account) in accounts.into_iter().enumerate() {
        let delay = random_in_range(config.thread_delay) * i as u64;
        spawn_task(&mut handles, provider.clone(), account, repo.clone(), config.clone(), delay);
    }

    while let Some(res) = handles.join_next().await {
        let (id, result) = res.unwrap();

        if let Err(e) = result {
            match e {
                crate::Error::Warmup(warmup_error) => {
                    // the wallet is either empty or has no more actions left
                    if let WarmupError::EmptyWallet(a) = warmup_error {
                        tracing::warn!("Wallet {a} has no non-zero balance tokens")
                    }
                    accounts::deactivate_account_by_id(repo.clone(), id).await?;
                }
                _ => {
                    tracing::error!("Thread stopped with error: {e}, restarting a thread");

                    let account = accounts::search_account_by_id(repo.clone(), id).await?;

                    spawn_task(
                        &mut handles,
                        provider.clone(),
                        account,
                        repo.clone(),
                        config.clone(),
                        0,
                    );
                }
            }
        }
    }

    Ok(())
}

/// Continuously processes an account by executing its available actions.
///
/// # Errors
///
/// Propagates errors from underlying operations. A returned `WarmupError::NoActionsLeft`
/// means that the account has no more available actions.
async fn process_account<P>(
    provider: P,
    repo: Arc<RepoImpls>,
    config: Arc<Config>,
    account: AccountModel,
) -> Result<()>
where
    P: Provider<Ethereum>,
{
    let signer = account.signer();
    let evm_client =
        EvmClient::<_, StrictNonceManager>::new(signer, NamedChain::MonadTestnet.into(), provider);

    loop {
        let account = accounts::search_account_by_id(repo.clone(), account.id).await?;

        let action = account
            .random_available_action()
            .ok_or_else(|| WarmupError::NoActionsLeft(account.address()))?;

        match action {
            AccountAction::Swap(dex) => {
                // true -> update_swap_count
                if swap(dex, &account, &evm_client, config.clone()).await? {
                    accounts::update_swap_count(repo.clone(), dex, account).await?;
                }
            }
            AccountAction::Lending(lending) => {
                if deposit(lending, &evm_client, config.clone()).await? {
                    accounts::update_deposit_count(repo.clone(), lending, account).await?;
                }
            }
        }
    }
}
