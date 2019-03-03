use dotenv::dotenv;
use env_logger;
use futures::Future;
use log;
use std::env;
use tgbot::access::{AccessRule, InMemoryAccessPolicy, Principal};
use tgbot::dispatcher::middleware::AccessMiddleware;
use tgbot::dispatcher::{Dispatcher, HandlerFuture, HandlerResult, MessageHandler};
use tgbot::methods::SendMessage;
use tgbot::types::Message;
use tgbot::Api;

struct Handler;

impl MessageHandler for Handler {
    fn handle(&mut self, api: &Api, message: &Message) -> HandlerFuture {
        log::info!("got a message: {:?}\n", message);
        if let Some(text) = message.get_text() {
            let chat_id = message.get_chat_id();
            let method = SendMessage::new(chat_id, text.data.clone());
            return HandlerFuture::new(api.execute(&method).then(|x| {
                log::info!("sendMessage result: {:?}\n", x);
                Ok(HandlerResult::Continue)
            }));
        }
        HandlerResult::Continue.into()
    }
}

fn main() {
    dotenv().ok();
    env_logger::init();

    let token = env::var("TGBOT_TOKEN").expect("TGBOT_TOKEN is not set");
    let proxy = env::var("TGBOT_PROXY").ok();
    let allowed_username =
        env::var("TGBOT_ALLOWED_USERNAME").expect("TGBOT_ALLOWED_USERNAME is not set");
    let api = match proxy {
        Some(proxy) => Api::with_proxy(token, &proxy),
        None => Api::create(token),
    }
    .expect("Failed to create API");

    // Deny from all except for allowed_username
    let rule = AccessRule::allow(Principal::username(allowed_username));
    let policy = InMemoryAccessPolicy::default().push_rule(rule);

    Dispatcher::new(api.clone())
        .add_middleware(AccessMiddleware::new(policy))
        .add_message_handler(Handler)
        .start_polling();
}
