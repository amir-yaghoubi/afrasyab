use afrasyab_app::worker;
use afrasyab_core::{AppState, Config, TelegramMode};
use afrasyab_oauth::router;
use afrasyab_storage::connect;
use afrasyab_telegram::commands::register_bot_commands;
use afrasyab_telegram::handler::{handle_callback, handle_message};
use anyhow::Context;
use std::sync::Arc;
use teloxide::dispatching::UpdateHandler;
use teloxide::error_handlers::IgnoringErrorHandler;
use teloxide::prelude::*;
use teloxide::requests::Requester;
use teloxide::types::Update;
use teloxide::update_listeners::webhooks::{self, Options as WebhookOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    afrasyab_core::log_runtime_versions().await;

    let config = Config::from_env()?;
    let pool = connect(&config.database_url)
        .await
        .context("connect sqlite")?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("run migrations")?;

    let state = AppState::new(config.clone(), pool).await?;
    let http_bind = config.http_bind.clone();

    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::run_pool(worker_state).await;
    });

    let bot = Bot::new(&config.telegram_bot_token);

    if let Err(e) = register_bot_commands(&bot, config.super_admin_telegram_id).await {
        tracing::warn!(error = %e, "failed to register telegram bot commands");
    }

    let handler = build_handler();

    match config.telegram_mode {
        TelegramMode::Polling => run_polling(bot, handler, state, &http_bind).await?,
        TelegramMode::Webhook => run_webhook(bot, handler, state, &config, &http_bind).await?,
    }

    Ok(())
}

fn build_handler() -> UpdateHandler<teloxide::RequestError> {
    dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback))
}

fn parse_http_bind(bind: &str) -> anyhow::Result<std::net::SocketAddr> {
    bind.parse()
        .with_context(|| format!("invalid HTTP_BIND={bind:?}"))
}

async fn run_polling(
    bot: Bot,
    handler: UpdateHandler<teloxide::RequestError>,
    state: std::sync::Arc<AppState>,
    http_bind: &str,
) -> anyhow::Result<()> {
    if let Err(err) = bot.delete_webhook().await {
        tracing::warn!(%err, "delete_webhook before polling failed");
    }

    let oauth_state = state.clone();
    let bind = http_bind.to_string();
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&bind)
            .await
            .expect("bind http listener");
        tracing::info!(%bind, "http server listening (oauth + health)");
        axum::serve(listener, router(oauth_state))
            .await
            .expect("http server");
    });

    tracing::info!("telegram dispatcher starting (polling)");
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn run_webhook(
    bot: Bot,
    handler: UpdateHandler<teloxide::RequestError>,
    state: std::sync::Arc<AppState>,
    config: &Config,
    http_bind: &str,
) -> anyhow::Result<()> {
    let addr = parse_http_bind(http_bind)?;
    let webhook_url_str = format!("{}/telegram/webhook", config.public_base_url);
    let webhook_url: url::Url = webhook_url_str
        .parse()
        .with_context(|| format!("invalid webhook URL from PUBLIC_BASE_URL: {webhook_url_str}"))?;

    let secret = config
        .telegram_webhook_secret
        .clone()
        .expect("webhook secret validated in config");

    let options = WebhookOptions::new(addr, webhook_url.clone()).secret_token(secret);

    tracing::info!(%addr, %webhook_url, "registering telegram webhook");
    let (listener, stop_future, webhook_router) = webhooks::axum_to_router(bot.clone(), options)
        .await
        .context("setup telegram webhook")?;

    let app = router(state.clone()).merge(webhook_router);

    let listener_error_handler = Arc::new(IgnoringErrorHandler);
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build();

    let dispatch_handle = tokio::spawn(async move {
        if let Err(err) = dispatcher
            .try_dispatch_with_listener(listener, listener_error_handler)
            .await
        {
            tracing::error!(%err, "telegram dispatcher stopped with error");
        }
    });

    let listener = tokio::net::TcpListener::bind(http_bind)
        .await
        .context("bind http listener")?;
    tracing::info!(%http_bind, "http server listening (oauth + health + webhook)");

    let shutdown = async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
        stop_future.await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .context("http server")?;

    dispatch_handle.abort();
    Ok(())
}
